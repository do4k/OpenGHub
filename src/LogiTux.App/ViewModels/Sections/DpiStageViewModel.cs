using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace LogiTux.App.ViewModels.Sections;

/// <summary>
/// One DPI preset button on the Sensitivity page: selecting it writes the
/// DPI to the device and is highlighted while it is the active stage.
/// </summary>
public sealed partial class DpiStageViewModel : ObservableObject
{
    private readonly Action<int> _pick;

    public int Value { get; }

    [ObservableProperty]
    private bool isActive;

    public DpiStageViewModel(int value, Action<int> pick)
    {
        Value = value;
        _pick = pick;
    }

    [RelayCommand]
    private void Pick() => _pick(Value);
}