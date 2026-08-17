using CommunityToolkit.Mvvm.ComponentModel;
using SectionsVMs = LogiTux.App.ViewModels.Sections;

namespace LogiTux.App.ViewModels;

/// <summary>
/// Base for a device's dedicated page: the G HUB-style section rail and
/// the selected section's content.
/// </summary>
public abstract partial class DevicePageViewModel : DeviceCardViewModel
{
    public IReadOnlyList<SectionsVMs.SectionViewModel> Sections { get; private set; } = [];

    [ObservableProperty]
    private SectionsVMs.SectionViewModel? selectedSection;

    /// <summary>True when the page has any sections (false for error cards).</summary>
    public bool HasSections => Sections.Count > 0;

    /// <summary>
    /// Installs the section list, selects the first one and wires each
    /// rail entry's select request to the shared selection state.
    /// </summary>
    protected void InstallSections(List<SectionsVMs.SectionViewModel> sections)
    {
        Sections = sections;
        SelectedSection = sections.FirstOrDefault();
        OnPropertyChanged(nameof(HasSections));
        foreach (SectionsVMs.SectionViewModel section in sections)
            section.SelectRequested = () => SelectedSection = section;
    }
}