using Avalonia.Svg.Skia;
using CommunityToolkit.Mvvm.ComponentModel;

namespace OpenGHub.App.ViewModels;

/// <summary>
/// Shown when a device can't be opened or probed (e.g. mouse asleep);
/// keeps the dashboard alive and shows why the card is empty.
/// </summary>
public sealed partial class ErrorCardViewModel : DeviceCardViewModel
{
    public ErrorCardViewModel(string name, string subtitle, string message, SvgImage? art = null)
    {
        Name = name;
        Subtitle = subtitle;
        Status = message;
        Art = art;
        Summary = message;
    }
}
