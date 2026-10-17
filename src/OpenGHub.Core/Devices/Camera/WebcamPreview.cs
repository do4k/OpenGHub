using OpenGHub.Core.V4l2;

namespace OpenGHub.Core.Devices.Camera;

/// <summary>
/// A live viewfinder on top of a V4l2Device: negotiates 640x480 (MJPEG,
/// falling back to whatever the driver gives us), streams through a
/// four-buffer mmap ring and publishes every decoded frame for the UI to
/// poll. Ported from internal/device/webcam/webcam.go (startPreview,
/// capture, decodeFrame, StopPreview).
/// </summary>
public sealed class WebcamPreview : IDisposable
{
    /// <summary>Requested preview frame size.</summary>
    public const int PreviewWidth = 640;
    public const int PreviewHeight = 480;

    private const int PreviewBufferCount = 4;

    private readonly V4l2Device _dev;
    private readonly string _name;
    private readonly object _lock = new();

    private bool _running;
    private Task? _captureTask;
    private List<MappedBuffer> _buffers = [];
    private byte[] _framePixels = [];
    private int _frameWidth;
    private int _frameHeight;
    private bool _frameIsJpeg;
    private ulong _frameSeq;

    public WebcamPreview(V4l2Device device, string name)
    {
        _dev = device;
        _name = name;
    }

    /// <summary>
    /// Starts streaming; a second call while running is a no-op. Throws
    /// IOException/InvalidOperationException describing why the stream
    /// could not be brought up (e.g. no usable pixel format).
    /// </summary>
    public void Start(uint pixelFormat = V4l2Device.PixFmtMJPEG)
    {
        lock (_lock)
        {
            if (_running)
                return;

            V4l2Format format;
            try
            {
                format = _dev.SetFormat(PreviewWidth, PreviewHeight, pixelFormat);
            }
            catch (IOException)
            {
                // Some drivers (notably PipeWire's shim) reject SetFormat
                // but honour GetFormat; accept what we actually got.
                format = _dev.GetFormat();
            }

            switch (format.PixelFormat)
            {
                case V4l2Device.PixFmtMJPEG:
                case V4l2Device.PixFmtYUYV:
                    break;
                default:
                    throw new InvalidOperationException(
                        $"webcam: {_name} has no usable preview pixel format (negotiated 0x{format.PixelFormat:x8})");
            }

            try
            {
                int granted = _dev.RequestBuffers(PreviewBufferCount);
                if (granted != PreviewBufferCount)
                    throw new IOException($"webcam: {_name} granted only {granted}/{PreviewBufferCount} buffers");
                for (int i = 0; i < granted; i++)
                {
                    _buffers.Add(_dev.MapBuffer(i));
                    _dev.QueueBuffer(i);
                }
                _dev.StreamOn();
            }
            catch (IOException)
            {
                ReleaseBuffersLocked();
                throw;
            }

            _running = true;
            _captureTask = Task.Run(() => CaptureLoop(format));
        }
    }

    /// <summary>
    /// Stops streaming and frees the ring; concurrent Start/Stop are safe,
    /// and a stop while never started is a no-op.
    /// </summary>
    public void Stop()
    {
        Task? task;
        lock (_lock)
        {
            if (!_running)
                return;
            _running = false;
            try
            {
                _dev.StreamOff();
            }
            catch (IOException)
            {
                // Device already off or lost; the loop will also exit on its
                // own DQBUF failure.
            }
            task = _captureTask;
        }

        if (!task.Wait(TimeSpan.FromSeconds(2)))
            return; // DQBUF stayed blocked; the task dies on the next access

        lock (_lock)
        {
            ReleaseBuffersLocked();
            _framePixels = [];
            _frameWidth = _frameHeight = 0;
            _frameIsJpeg = false;
            _frameSeq = 0;
        }
    }

    /// <summary>
    /// Copies the newest frame, if any arrived; returns false until the
    /// first frame is published. The returned pixels are a snapshot.
    /// </summary>
    public bool TryGetFrame(out PreviewFrame frame)
    {
        lock (_lock)
        {
            if (_frameSeq == 0)
            {
                frame = null!;
                return false;
            }
            var pixels = new byte[_framePixels.Length];
            _framePixels.CopyTo(pixels, 0);
            frame = new PreviewFrame(pixels, _frameWidth, _frameHeight, _frameIsJpeg, _frameSeq);
            return true;
        }
    }

    public void Dispose() => Stop();

    private void CaptureLoop(V4l2Format format)
    {
        uint pixelFormat = format.PixelFormat;
        int width = format.Width;
        int height = format.Height;

        while (true)
        {
            int index;
            int bytesUsed;
            try
            {
                (index, bytesUsed) = _dev.DequeueBuffer();
            }
            catch (IOException)
            {
                return; // stream stopped or device lost
            }

            byte[] pixels = [];
            bool isJpeg = false;
            bool decoded = false;
            if (index >= 0 && index < _buffers.Count)
            {
                MappedBuffer buffer = _buffers[index]!;
                var raw = new byte[bytesUsed];
                buffer.Read(raw, bytesUsed);
                if (pixelFormat == V4l2Device.PixFmtMJPEG)
                {
                    pixels = raw;
                    isJpeg = true;
                    decoded = true;
                }
                else
                {
                    pixels = DecodeYuyvToBgra(raw, width, height);
                    decoded = true;
                }
            }

            if (decoded)
            {
                lock (_lock)
                {
                    _framePixels = pixels;
                    _frameWidth = width;
                    _frameHeight = height;
                    _frameIsJpeg = isJpeg;
                    _frameSeq++;
                }
            }

            try
            {
                _dev.QueueBuffer(index);
            }
            catch (IOException)
            {
                return;
            }
        }
    }

    private void ReleaseBuffersLocked()
    {
        foreach (MappedBuffer buffer in _buffers)
            buffer.Dispose();
        _buffers = [];
        try
        {
            _dev.RequestBuffers(0);
        }
        catch (IOException)
        {
            // Ring is gone with the fd anyway.
        }
    }

    /// <summary>
    /// Converts a YUYV (4:2:2 pack) frame to BGRA, two pixels per four
    /// bytes, with ITU-R BT.601 luma-to-RGB; mirrors the Go decodeFrame
    /// path's output layout.
    /// </summary>
    private static byte[] DecodeYuyvToBgra(byte[] yuyv, int width, int height)
    {
        var bgra = new byte[width * height * 4];
        int src = 0;
        int dst = 0;
        for (int i = 0; i < width * height; i += 2)
        {
            byte y0 = yuyv[src++];
            byte u = yuyv[src++];
            byte y1 = yuyv[src++];
            byte v = yuyv[src++];
            WritePixel(bgra, dst, y0, u, v);
            dst += 4;
            WritePixel(bgra, dst, y1, u, v);
            dst += 4;
        }
        return bgra;
    }

    private static void WritePixel(byte[] bgra, int offset, byte y, byte u, byte v)
    {
        int c = y - 16;
        int d = u - 128;
        int e = v - 128;
        int r = (298 * c + 409 * e + 128) >> 8;
        int g = (298 * c - 100 * d - 208 * e + 128) >> 8;
        int b = (298 * c + 516 * d + 128) >> 8;
        bgra[offset] = (byte)Math.Clamp(b, 0, 255);
        bgra[offset + 1] = (byte)Math.Clamp(g, 0, 255);
        bgra[offset + 2] = (byte)Math.Clamp(r, 0, 255);
        bgra[offset + 3] = 255;
    }
}