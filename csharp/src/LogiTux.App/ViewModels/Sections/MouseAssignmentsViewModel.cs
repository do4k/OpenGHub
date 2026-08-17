namespace LogiTux.App.ViewModels.Sections;

/// <summary>
/// The mouse's Assignments page. A place to stub the G HUB-spec surface:
/// the Commands/Keys/Actions/Macros/System tab rail. Real remapping needs
/// the device-side REPROG_CONTROLS_V4 feature and a uinput emission layer,
/// neither of which the receiver-connected unit in front of us exposes —
/// so the page carries the enumeration instead.
/// </summary>
public sealed class MouseAssignmentsViewModel
{
    // G HUB's assignment sub-tabs, in the same rail order.
    public IReadOnlyList<string> Items { get; } = ["Commands", "Keys", "Actions", "Macros", "System"];

    public string Note { get; } =
        "Reassign a button to another mouse button or a keyboard key.\n" +
        "This unit's receiver handles remapping itself, so LogiTux can't" +
        " read or drive it directly — the tabs above are placeholders for" +
        " the software-level layer (uinput) once it lands.";
}