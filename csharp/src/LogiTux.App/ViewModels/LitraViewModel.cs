using CommunityToolkit.Mvvm.ComponentModel;
using LogiTux.Core.Devices.Camera;
using LogiTux.Core.Devices.Litra;
using LogiTux.Core.Hid;
using SectionsVMs = LogiTux.App.ViewModels.Sections;

namespace LogiTux.App.ViewModels;

/// <summary>
/// Card and page for a Litra Glow/Beam: write-only protocol, so state
/// shown here is what we last sent. The Lighting section IS this VM
/// (power, brightness, temperature), the Presets section holds the
/// one-tap presets.
/// </summary>
public sealed partial class LitraViewModel : DevicePageViewModel
{
    private readonly LitraLight _light;
    private readonly Stream _stream;

    [ObservableProperty]
    private bool isOn;

    [ObservableProperty]
    private int brightness = 60;

    [ObservableProperty]
    private int temperature = 3200;

    public int MinTemperature => LitraLight.MinTemperatureK;
    public int MaxTemperature => LitraLight.MaxTemperatureK;

    private LitraViewModel(LitraLight light, Stream stream, string name, string subtitle)
    {
        _light = light;
        _stream = stream;
        Name = name;
        Subtitle = subtitle;
    }

    public static LitraViewModel Create(HidBackend backend, DeviceInfo info)
    {
        Stream stream = backend.Open(info);
        var light = new LitraLight(stream);
        var vm = new LitraViewModel(
            light,
            stream,
            LitraLight.FriendlyName(info.ProductId),
            $"serial {info.Serial ?? "n/a"}")
        {
            Art = info.ProductId == LitraLight.ProductGlow
                ? ArtCatalog.Load("litra-glow.svg")
                : ArtCatalog.Load("litra-beam.svg"),
        };
        vm.UpdateSummary();
        vm.InstallSections(
        [
            new SectionsVMs.SectionViewModel("LIGHTING", "☀", vm),
            new SectionsVMs.SectionViewModel("PRESETS", "▦", new SectionsVMs.PresetsViewModel(light)),
        ]);
        return vm;
    }

    private void UpdateSummary() => Summary = $"{IsOn} · {Brightness}% · {Temperature} K";

    partial void OnIsOnChanged(bool value)
    {
        UpdateSummary();
        _ = SetPowerAsync(value).ConfigureAwait(false);
    }

    partial void OnBrightnessChanged(int value)
    {
        UpdateSummary();
        _ = DebounceWriteAsync(() => _light.SetBrightnessAsync(value), 200);
    }

    partial void OnTemperatureChanged(int value)
    {
        UpdateSummary();
        _ = DebounceWriteAsync(() => _light.SetTemperatureAsync(value), 200);
    }

    private async Task SetPowerAsync(bool on)
    {
        try
        {
            await _light.SetPowerAsync(on);
        }
        catch (Exception e)
        {
            Status = $"power write failed: {e.Message}";
        }
    }

    public override async ValueTask DisposeAsync()
    {
        await _light.SetPowerAsync(false).ConfigureAwait(false);
        await _stream.DisposeAsync().ConfigureAwait(false);
    }
}