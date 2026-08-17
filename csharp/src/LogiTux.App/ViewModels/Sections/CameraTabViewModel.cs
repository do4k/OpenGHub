namespace LogiTux.App.ViewModels.Sections;

/// <summary>
/// A webcam page section holding a filtered set of camera control
/// sliders: the Camera tab gets the optics (zoom, pan, tilt, focus,
/// exposure), the Image tab the adjustments (brightness, contrast,
/// saturation, sharpness) — matching cmd/logitux/ui.go's split.
/// </summary>
public sealed class CameraTabViewModel
{
    public IReadOnlyList<CameraControlVm> Controls { get; }

    public CameraTabViewModel(IReadOnlyList<CameraControlVm> controls)
    {
        Controls = controls;
    }
}