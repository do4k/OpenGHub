namespace LogiTux.App.ViewModels.Sections;

/// <summary>
/// A webcam page section holding a filtered set of camera control
/// sliders: the Camera tab gets the optics (zoom, pan, tilt, focus,
/// exposure), the Image tab the adjustments (brightness, contrast,
/// saturation, sharpness) — matching cmd/logitux/ui.go's split. Both
/// tabs share the card's live preview.
/// </summary>
public sealed class CameraTabViewModel
{
    /// <summary>The live viewfinder for this card, shared by both tabs.</summary>
    public CameraPreviewViewModel? Preview { get; }

    public IReadOnlyList<CameraControlVm> Controls { get; }

    public CameraTabViewModel(IReadOnlyList<CameraControlVm> controls, CameraPreviewViewModel? preview = null)
    {
        Controls = controls;
        Preview = preview;
    }
}