namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// A placeholder section for G HUB-spec features Open G Hub doesn't drive
/// yet: the page shows the note and the spec's sub-tabs/pages as disabled
/// chips so the full G HUB surface is visible and accounted for.
/// </summary>
public sealed class StubSectionViewModel
{
    public string Note { get; }

    public IReadOnlyList<string> Items { get; }

    public StubSectionViewModel(string note, IReadOnlyList<string> items)
    {
        Note = note;
        Items = items;
    }
}