using CommunityToolkit.Mvvm.ComponentModel;
using LogiTux.Core.Devices.Camera;

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
/// Card for a Logitech UVC webcam: one slider per control the
/// driver reports.
/// </summary>
public sealed class CameraCardViewModel : DeviceCardViewModel
{
    private readonly V4l2Camera _camera;

    public IReadOnlyList<CameraControlVm> Controls { get; }

    private CameraCardViewModel(V4l2Camera camera, List<CameraControlVm> controls)
    {
        _camera = camera;
        Controls = controls;
        Name = camera.Name;
        Subtitle = $"serial {camera.Serial} · {camera.DevicePath}";
    }

    public static CameraCardViewModel? OpenAsync(string node)
    {
        V4l2Camera? camera = V4l2Camera.Open(node);
        if (camera is null)
            return null; // e.g. a metadata node, not the capture interface

        var controls = new List<CameraControlVm>(camera.Controls.Count);
        foreach (CameraControl control in camera.Controls)
        {
            var vm = new CameraControlVm(camera, control);
            vm.TryLoadValue();
            controls.Add(vm);
        }

        return new CameraCardViewModel(camera, controls) { Status = "ready" };
    }

    public override ValueTask DisposeAsync()
    {
        _camera.Dispose();
        return ValueTask.CompletedTask;
    }
}
