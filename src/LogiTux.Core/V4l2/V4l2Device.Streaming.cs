using System.Runtime.InteropServices;

namespace LogiTux.Core.V4l2;

/// <summary>
/// The capture half of V4l2Device: format negotiation, the mmap buffer
/// ring and streaming, ported from internal/v4l2/v4l2.go.
/// </summary>
public sealed partial class V4l2Device
{
    /// <summary>
    /// FourCC pixel formats for capture negotiation.
    /// </summary>
    public const uint PixFmtMJPEG = 0x47504a4d; // 'M','J','P','G'
    public const uint PixFmtYUYV = 0x56595559; // 'Y','U','Y','V'

    private const uint BufTypeVideoCapture = 1;
    private const uint MemoryMMAP = 1;

    // Struct sizes fixed by the kernel ABI (64-bit); keep in step with the
    // Go port's constants.
    private const int SizeofFormat = 208;
    private const int SizeofRequestbuffers = 20;
    private const int SizeofBuffer = 88;

    private static readonly ulong VidiocGFmt = Vidioc(IocRead, 4, SizeofFormat);
    private static readonly ulong VidiocSFmt = Vidioc(IocRead | IocWrite, 5, SizeofFormat);
    private static readonly ulong VidiocReqbufs = Vidioc(IocRead | IocWrite, 8, SizeofRequestbuffers);
    private static readonly ulong VidiocQuerybuf = Vidioc(IocRead | IocWrite, 9, SizeofBuffer);
    private static readonly ulong VidiocQBuf = Vidioc(IocRead | IocWrite, 15, SizeofBuffer);
    private static readonly ulong VidiocDQBuf = Vidioc(IocRead | IocWrite, 17, SizeofBuffer);
    private static readonly ulong VidiocStreamOn = Vidioc(IocWrite, 18, 4);
    private static readonly ulong VidiocStreamOff = Vidioc(IocWrite, 19, 4);

    private static uint Fourcc(byte a, byte b, byte c, byte d) => (uint)(a | b << 8 | c << 16 | d << 24);

    private static V4l2Format ParseFormat(byte[] buf) => new(
        BitConverter.ToInt32(buf, 8),
        BitConverter.ToInt32(buf, 12),
        BitConverter.ToUInt32(buf, 16),
        BitConverter.ToInt32(buf, 24),
        BitConverter.ToInt32(buf, 28));

    /// <summary>
    /// Asks the driver to switch the device to the given frame size and
    /// pixel format; returns what was actually negotiated (the driver may
    /// substitute its own format). v4l2_format layout: type u32 at 0,
    /// pix.width at 8, pix.height at 12, pix.pixelformat at 16, and read
    /// back pix.bytesperline at 24, pix.sizeimage at 28.
    /// </summary>
    public V4l2Format SetFormat(int width, int height, uint pixelFormat)
    {
        var buf = new byte[SizeofFormat];
        BitConverter.GetBytes(BufTypeVideoCapture).CopyTo(buf, 0);
        BitConverter.GetBytes(width).CopyTo(buf, 8);
        BitConverter.GetBytes(height).CopyTo(buf, 12);
        BitConverter.GetBytes(pixelFormat).CopyTo(buf, 16);
        Ioctl(VidiocSFmt, buf);
        return ParseFormat(buf);
    }

    /// <summary>
    /// Reads the currently negotiated capture format; some drivers
    /// (e.g. PipeWire's shim) reject SetFormat but honour GetFormat.
    /// </summary>
    public V4l2Format GetFormat()
    {
        var buf = new byte[SizeofFormat];
        Ioctl(VidiocGFmt, buf);
        return ParseFormat(buf);
    }

    /// <summary>
    /// Allocates (or frees, with count 0) the kernel-side mmap buffer ring.
    /// Returns how many buffers were actually granted. v4l2_requestbuffers
    /// layout: count u32 at 0, type u32 at 4, memory u32 at 8.
    /// </summary>
    public int RequestBuffers(int count)
    {
        var buf = new byte[SizeofRequestbuffers];
        BitConverter.GetBytes(count).CopyTo(buf, 0);
        BitConverter.GetBytes(BufTypeVideoCapture).CopyTo(buf, 4);
        BitConverter.GetBytes(MemoryMMAP).CopyTo(buf, 8);
        Ioctl(VidiocReqbufs, buf);
        return BitConverter.ToInt32(buf, 0);
    }

    /// <summary>
    /// mmaps buffer index's region into the process (its kernel offset and
    /// length come from QUERYBUF: m.offset at 64, length at 72).
    /// </summary>
    public MappedBuffer MapBuffer(int index)
    {
        var buf = new byte[SizeofBuffer];
        BitConverter.GetBytes(BufTypeVideoCapture).CopyTo(buf, 4);
        BitConverter.GetBytes(index).CopyTo(buf, 0);
        Ioctl(VidiocQuerybuf, buf);
        return MappedBuffer.Map(_fd, BitConverter.ToInt32(buf, 64), BitConverter.ToInt32(buf, 72), _path);
    }

    /// <summary>
    /// Hands buffer index back to the driver for the next frame
    /// (v4l2_buffer layout: index u32 at 0, type u32 at 4, memory u32 at 60).
    /// </summary>
    public void QueueBuffer(int index)
    {
        var buf = new byte[SizeofBuffer];
        BitConverter.GetBytes(BufTypeVideoCapture).CopyTo(buf, 4);
        BitConverter.GetBytes(MemoryMMAP).CopyTo(buf, 60);
        BitConverter.GetBytes(index).CopyTo(buf, 0);
        Ioctl(VidiocQBuf, buf);
    }

    /// <summary>
    /// Blocks until the driver delivers a filled buffer; returns its index
    /// and the frame's byte count (bytesused at 8).
    /// </summary>
    public (int Index, int BytesUsed) DequeueBuffer()
    {
        var buf = new byte[SizeofBuffer];
        BitConverter.GetBytes(BufTypeVideoCapture).CopyTo(buf, 4);
        Ioctl(VidiocDQBuf, buf);
        return (BitConverter.ToInt32(buf, 0), BitConverter.ToInt32(buf, 8));
    }

    /// <summary>
    /// Starts the capture pipeline; queued buffers start filling and
    /// DequeueBuffer begins returning frames.
    /// </summary>
    public void StreamOn()
    {
        var buf = new byte[4];
        BitConverter.GetBytes(BufTypeVideoCapture).CopyTo(buf, 0);
        Ioctl(VidiocStreamOn, buf);
    }

    /// <summary>
    /// Stops the capture pipeline; a blocked DequeueBuffer returns an
    /// error and the driver empties the ring.
    /// </summary>
    public void StreamOff()
    {
        var buf = new byte[4];
        BitConverter.GetBytes(BufTypeVideoCapture).CopyTo(buf, 0);
        Ioctl(VidiocStreamOff, buf);
    }
}