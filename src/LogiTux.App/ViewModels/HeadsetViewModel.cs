using CommunityToolkit.Mvvm.ComponentModel;
using LogiTux.Core.Config;
using LogiTux.Core.Devices.Headset;
using LogiTux.Core.Hid;
using LogiTux.Core.Hidpp;
using SectionsVMs = LogiTux.App.ViewModels.Sections;

namespace LogiTux.App.ViewModels;

/// <summary>
/// Card and page for the PRO X Wireless headset: Acoustics (sidetone +
/// mic noise reduction), the equalizer bank, and the G HUB-spec stub
/// sections (Surround, Mic, Device Settings).
/// </summary>
public sealed partial class HeadsetViewModel : DevicePageViewModel
{
    private readonly ProxHeadset _headset;
    private readonly SectionsVMs.HeadsetAcousticsViewModel _acoustics;
    private readonly SectionsVMs.EqualizerViewModel _equalizer;

    [ObservableProperty]
    private int batteryPercent;

    [ObservableProperty]
    private bool isCharging;

    [ObservableProperty]
    private bool hasBattery;

    private HeadsetViewModel(ProxHeadset headset, string name, string subtitle, ConfigStore? store, string serial)
    {
        _headset = headset;
        Name = name;
        Subtitle = subtitle;
        _acoustics = new SectionsVMs.HeadsetAcousticsViewModel(headset, store, serial);
        _equalizer = new SectionsVMs.EqualizerViewModel(headset, store, serial);
    }

    public static async Task<HeadsetViewModel?> CreateAsync(HidBackend backend, DeviceInfo info, ConfigStore? store = null)
    {
        ProxHeadset? headset = await ProxHeadset.OpenAsync(backend, info).ConfigureAwait(false);
        if (headset is null)
            return null; // off/asleep right now

        var vm = new HeadsetViewModel(headset, headset.Name, $"serial {info.Serial ?? "n/a"}", store, info.Serial ?? $"prox-{info.ProductId:x4}")
        {
            Art = ArtCatalog.Load("prox.svg"),
        };
        await vm.LoadAsync().ConfigureAwait(false);
        return vm;
    }

    private async Task LoadAsync()
    {
        List<SectionsVMs.SectionViewModel> sections =
        [
            new("ACOUSTICS", "♫", _acoustics),
            new("EQ", "▤", _equalizer),
            new("SURROUND", "△", new SectionsVMs.StubSectionViewModel(
                "DTS Headphone:X 2.0 7.1 surround is licensed hardware DSP; " +
                "the PRO X Wireless doesn't expose it over HID++, so this " +
                "page mirrors G HUB's surface without any controls.",
                ["DTS Headphone:X 2.0"])),
            new("MIC", "●", new SectionsVMs.StubSectionViewModel(
                "Blue VO!CE microphone effects and samples run inside G HUB's " +
                "licensed DSP. LogiTux can drive the mic gain sample rate " +
                "once the audio stack lands; the G HUB spec surface is " +
                "shown here for reference.",
                ["Sample Rate", "Mic Gain", "Blue VO!CE"])),
            new("DEVICE SETTINGS", "⚙", new SectionsVMs.StubSectionViewModel(
                "Firmware updates, sleep timers and connection info are " +
                "out of scope for a hardware-only port. This page keeps " +
                "G HUB's navigation visible.",
                ["Firmware", "Sleep Timer", "Connection Info"])),
        ];
        InstallSections(sections);
        await Task.WhenAll(
            _acoustics.LoadAsync(),
            _equalizer.LoadAsync()).ConfigureAwait(false);
        await RefreshBatteryAsync().ConfigureAwait(false);
        Status = "ready";
    }

    public async Task RefreshBatteryAsync()
    {
        try
        {
            (int percent, bool charging) = await _headset.GetBatteryAsync().ConfigureAwait(false);
            BatteryPercent = percent;
            IsCharging = charging;
            HasBattery = true;
            Summary = $"{percent}% battery";
        }
        catch (InvalidOperationException)
        {
            HasBattery = false;
            Summary = "no battery feature";
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
        {
            Status = $"battery read: {e.Message}";
        }
    }

    public override ValueTask DisposeAsync() => _headset.DisposeAsync();
}