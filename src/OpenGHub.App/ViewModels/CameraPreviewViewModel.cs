using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using OpenGHub.Core.Devices.Camera;
using SkiaSharp;

namespace OpenGHub.App.ViewModels;

/// <summary>
/// Drives a WebcamPreview into a WriteableBitmap at ~15 fps: starts the
/// V4L2 stream, poll-ticks for new frames on the UI thread and lands them
/// on the card's Image control. JPEG frames are decoded with SkiaSharp;
/// YUYV frames arrive already BGRA from Core. The tick interval mirrors
/// cmd/openghub/ui.go's previewInterval.
/// </summary>
public sealed partial class CameraPreviewViewModel : ObservableObject, IDisposable
{
    private static readonly TimeSpan TickInterval = TimeSpan.FromMilliseconds(66);

    private readonly WebcamPreview _preview;
    private DispatcherTimer? _timer;
    private ulong _lastSeq;
    private readonly WriteableBitmap[] _surfaces = new WriteableBitmap[2];
    private int _paintIndex;

    /// <summary>True while a frame source is up; hides the preview slot otherwise.</summary>
    [ObservableProperty]
    public partial bool IsActive { get; set; }

    /// <summary>The latest rendered frame, replaced at tick rate.</summary>
    [ObservableProperty]
    public partial WriteableBitmap? Surface { get; set; }

    public CameraPreviewViewModel(WebcamPreview preview)
    {
        _preview = preview;
    }

    /// <summary>
    /// Brings the stream up; a camera that refuses (format negotiation
    /// failure, node gone) simply keeps the preview slot hidden. Also
    /// creates the tick timer here rather than in the constructor: this
    /// view model is built during background discovery, and a
    /// DispatcherTimer can only tick on the thread that created it.
    /// </summary>
    public void Start()
    {
        try
        {
            _preview.Start();
            if (_timer is null)
            {
                _timer = new DispatcherTimer(TickInterval, DispatcherPriority.Normal, OnTick);
                var size = new PixelSize(WebcamPreview.PreviewWidth, WebcamPreview.PreviewHeight);
                for (int i = 0; i < _surfaces.Length; i++)
                    _surfaces[i] = new WriteableBitmap(size, new Vector(96, 96), PixelFormat.Bgra8888, AlphaFormat.Premul);
                Surface = _surfaces[0];
            }
            _timer.Start();
            IsActive = true;
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"OpenGHub: preview unavailable: {e.Message}");
            IsActive = false;
        }
    }

    public void Stop()
    {
        _timer?.Stop();
        _preview.Stop();
        IsActive = false;
    }

    private void OnTick(object? sender, EventArgs e)
    {
        if (!_preview.TryGetFrame(out PreviewFrame frame) || frame.Sequence == _lastSeq)
            return;
        _lastSeq = frame.Sequence;
        Render(frame);
    }

    private void Render(PreviewFrame frame)
    {
        ValidateSurfaces(frame.Width, frame.Height);

        byte[] pixels;
        if (frame.IsJpeg)
        {
            using SKBitmap? decoded = SKBitmap.Decode(frame.Pixels);
            if (decoded is null)
                return;
            pixels = new byte[decoded.Width * decoded.Height * 4];
            Marshal.Copy(decoded.GetPixels(), pixels, 0, pixels.Length);
        }
        else
        {
            pixels = frame.Pixels;
        }

        _paintIndex ^= 1;
        WriteableBitmap target = _surfaces[_paintIndex];
        using ILockedFramebuffer fb = target.Lock();
        Marshal.Copy(pixels, 0, fb.Address, pixels.Length);
        // Alternate targets so the bound Source reference changes each frame
        // and the Image actually repaints the freshly copied pixels.
        Surface = target;
    }

    private void ValidateSurfaces(int width, int height)
    {
        if (_surfaces[0].PixelSize.Width != width || _surfaces[0].PixelSize.Height != height)
        {
            var size = new PixelSize(width, height);
            for (int i = 0; i < _surfaces.Length; i++)
                _surfaces[i] = new WriteableBitmap(size, new Vector(96, 96), PixelFormat.Bgra8888, AlphaFormat.Premul);
        }
    }

    public void Dispose()
    {
        if (_timer is not null)
            _timer.Tick -= OnTick;
        Stop();
    }
}