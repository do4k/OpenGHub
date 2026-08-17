using LogiTux.Core.Devices.Litra;

namespace LogiTux.App.ViewModels.Sections;

/// <summary>
/// Litra page: Lighting presets — one-tap temperature/brightness
/// combinations, mirroring G HUB's preset pills for the glow. Since the
/// light can't report state back, each preset writes its full target
/// (power, brightness, temperature) itself.
/// </summary>
public sealed class PresetsViewModel
{
    private readonly LitraLight _light;

    public IReadOnlyList<PresetViewModel> Presets { get; }

    public PresetsViewModel(LitraLight light)
    {
        _light = light;
        Presets =
        [
            new PresetViewModel("Reading", () => ApplyAsync(2700, 50)),
            new PresetViewModel("Working", () => ApplyAsync(4000, 80)),
            new PresetViewModel("Daylight", () => ApplyAsync(6500, 100)),
            new PresetViewModel("Off", () => _light.SetPowerAsync(false)),
        ];
    }

    private async Task ApplyAsync(int kelvin, int percent)
    {
        await _light.SetPowerAsync(true);
        await _light.SetBrightnessAsync(percent);
        await _light.SetTemperatureAsync(kelvin);
    }
}