using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// One entry in a device page's left-hand rail: a G HUB-style section
/// (Sensitivity, Lighting, Sound, ...) with a pickable content body.
/// </summary>
public sealed partial class SectionViewModel : ObservableObject
{
    /// <summary>Short uppercase label shown under the rail glyph.</summary>
    public string Title { get; }

    /// <summary>Unicode glyph for the rail icon.</summary>
    public string Glyph { get; }

    /// <summary>Body content; matched to a DataTemplate by runtime type.</summary>
    public object Content { get; }

    /// <summary>Invoked when the section is picked (wired by the owning page VM).</summary>
    public Action? SelectRequested { get; set; }

    [ObservableProperty]
    private bool isSelected;

    public SectionViewModel(string title, string glyph, object content)
    {
        Title = title;
        Glyph = glyph;
        Content = content;
    }

    [RelayCommand]
    private void Select() => SelectRequested?.Invoke();
}