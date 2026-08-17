using CommunityToolkit.Mvvm.ComponentModel;
using LogiTux.Core.Devices.Camera;
using SectionsVMs = LogiTux.App.ViewModels.Sections;

namespace LogiTux.App.ViewModels;

/// <summary>
/// One V4L2 control on a camera card: a slider bound to the driver
/// value with debounced writes, mirroring the other cards' controls.
/// </summary>
public sealed partial class CameraControlVm : ObservableObject
{
    private readonly V4l2Camera _camera;
    private readonly string _name;
    private CancellationTokenSource? _debounceCts;

    public string Name { get; }
    public int Min { get; }
    public int Max { get; }
    public int Step { get; }
    public int Default { get; }

    [ObservableProperty]
    public partial int Value { get; set; }

    public CameraControlVm(V4l2Camera camera, CameraControl control)
    {
        _camera = camera;
        _name = control.Name;
        Name = control.Name;
        Min = control.Min;
        Max = control.Max;
        Step = control.Step;
        Default = control.Default;
    }

    /// <summary>
    /// Reads the current driver value; returns false if the read
    /// failed (control momentarily disabled, node gone), leaving Value at
    /// Min so the UI still shows a well-formed row.
    /// </summary>
    public bool TryLoadValue()
    {
        try
        {
            Value = _camera.GetControl(_name);
            return true;
        }
        catch (IOException)
        {
            return false;
        }
    }

    partial void OnValueChanged(int value)
    {
        _debounceCts?.Cancel();
        var cts = new CancellationTokenSource();
        _debounceCts = cts;
        _ = WriteAsync(cts, value);
    }

    private async Task WriteAsync(CancellationTokenSource cts, int value)
    {
        try
        {
            await Task.Delay(200, cts.Token);
            if (!cts.IsCancellationRequested)
                _camera.SetControl(_name, value);
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception)
        {
            // Control writes can hit busy/disabled conditions mid-drag; the
            // next slider movement retries. Not worth a status line here.
        }
    }
}

/// <summary>
/// Card and page for a Logitech UVC webcam, G HUB-style: the Camera tab
/// (zoom, pan, tilt, focus, exposure), the Video stub tab and the Image
/// tab (brightness, contrast, saturation, sharpness).
/// </summary>
public sealed class CameraCardViewModel : DevicePageViewModel
{
    private readonly V4l2Camera _camera;

    // Controls that belong on the Image tab; everything else camera-shaped
    // lands on the Camera tab. Mirrors cmd/logitux/ui.go's imageAdjustments.
    private static readonly HashSet<string> ImageAdjustments =
    [
        "Brightness", "Contrast", "Saturation", "Sharpness",
    ];

    private CameraCardViewModel(V4l2Camera camera, string name, string subtitle)
    {
        _camera = camera;
        Name = name;
        Subtitle = subtitle;
        Preview = new CameraPreviewViewModel(camera.CreatePreview());
    }

    /// <summary>
    /// A live viewfinder shown above the Camera/Image tabs while its
    /// page is open or the dashboard preview is visible.
    /// </summary>
    public CameraPreviewViewModel Preview { get; }

    public void StartPreview() => Preview.Start();

    public void StopPreview() => Preview.Stop();

    public static CameraCardViewModel? OpenAsync(string node)
    {
        V4l2Camera? camera = V4l2Camera.Open(node);
        if (camera is null)
            return null; // e.g. a metadata node, not the capture interface

        var all = new List<CameraControlVm>(camera.Controls.Count);
        foreach (CameraControl control in camera.Controls)
        {
            var vm = new CameraControlVm(camera, control);
            vm.TryLoadValue();
            all.Add(vm);
        }

        var card = new CameraCardViewModel(camera, camera.Name, $"serial {camera.Serial} · {camera.DevicePath}")
        {
            Status = "ready",
            Summary = $"{all.Count} controls",
            Art = ArtCatalog.Load("webcam.svg"),
        };
        card.InstallSections(
        [
            new SectionsVMs.SectionViewModel("CAMERA", "☐", new SectionsVMs.CameraTabViewModel(
                all.Where(c => !ImageAdjustments.Contains(c.Name)).ToList(), card.Preview)),
            new SectionsVMs.SectionViewModel("VIDEO", "▤", new SectionsVMs.StubSectionViewModel(
                "Frame rate, field of view, shutter speed, ISO and HDR are " +
                "hardware/encoder settings that Windows-only G HUB drives " +
                "through its own camera plugin. On Linux, V4L2 exposes the " +
                "capture and image controls on the other tabs; this tab " +
                "mirrors G HUB's spec surface.",
                ["FOV", "Shutter Speed", "ISO", "HDR"])),
            new SectionsVMs.SectionViewModel("IMAGE", "◧", new SectionsVMs.CameraTabViewModel(
                all.Where(c => ImageAdjustments.Contains(c.Name)).ToList(), card.Preview)),
        ]);
        return card;
    }

    public override ValueTask DisposeAsync()
    {
        StopPreview();
        _camera.Dispose();
        return ValueTask.CompletedTask;
    }
}