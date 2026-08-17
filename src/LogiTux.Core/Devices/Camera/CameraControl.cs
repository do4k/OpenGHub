namespace LogiTux.Core.Devices.Camera;

/// <summary>
/// One camera control: name, driver-reported range and default, and
/// its current value. Exposure auto is presented as 0/1 even though the
/// driver reports a v4l2_exposure_auto_type menu (1=manual, 3=auto).
/// </summary>
public sealed record CameraControl(string Name, int Min, int Max, int Step, int Default);