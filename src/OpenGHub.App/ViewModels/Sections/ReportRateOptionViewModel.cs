using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// One report-rate option on the Sensitivity page (e.g. 1000 Hz), shown
/// as a G HUB-style segmented picker.
/// </summary>
public sealed partial class ReportRateOptionViewModel : ObservableObject
{
    private readonly Action<int> _pick;

    public int Value { get; }

    [ObservableProperty]
    private bool isActive;

    public ReportRateOptionViewModel(int value, Action<int> pick)
    {
        Value = value;
        _pick = pick;
    }

    [RelayCommand]
    private void Pick() => _pick(Value);
}