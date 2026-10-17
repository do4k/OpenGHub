using CommunityToolkit.Mvvm.ComponentModel;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// A named on/off setting (Auto Focus, Mic Noise Reduction, Power, ...)
/// that writes immediately on change. Reads back state at load time.
/// </summary>
public sealed partial class ToggleSettingViewModel : ObservableObject
{
    private readonly Func<bool, Task> _write;
    private bool _suppressWrite;

    public string Name { get; }

    [ObservableProperty]
    private bool isOn;

    public ToggleSettingViewModel(string name, bool initial, Func<bool, Task> write)
    {
        Name = name;
        _write = write;
        _suppressWrite = true;
        IsOn = initial;
        _suppressWrite = false;
    }

    partial void OnIsOnChanged(bool value)
    {
        if (_suppressWrite)
            return;
        _ = _write(value);
    }
}