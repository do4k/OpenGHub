using CommunityToolkit.Mvvm.Input;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// One quick preset button on the Litra page (e.g. "Daylight").
/// </summary>
public sealed partial class PresetViewModel
{
    public string Label { get; }

    private readonly Func<Task> _apply;

    public PresetViewModel(string label, Func<Task> apply)
    {
        Label = label;
        _apply = apply;
    }

    [RelayCommand]
    private async Task Apply()
    {
        try
        {
            await _apply();
        }
        catch (Exception)
        {
            // Write-only light protocol; a failed write means the cable
            // or the light itself went away — ignore, next click retries.
        }
    }
}