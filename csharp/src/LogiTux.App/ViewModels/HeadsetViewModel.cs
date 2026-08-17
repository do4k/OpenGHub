using CommunityToolkit.Mvvm.ComponentModel;
using LogiTux.Core.Devices.Headset;
using LogiTux.Core.Hid;
using LogiTux.Core.Hidpp;

namespace LogiTux.App.ViewModels;

/// <summary>
/// Card for the PRO X Wireless headset: battery only for now (sidetone
/// and EQ come with the prox port).
/// </summary>
public sealed partial class HeadsetViewModel : DeviceCardViewModel
{
    private readonly ProxHeadset _headset;

    private HeadsetViewModel(ProxHeadset headset, string name, string subtitle)
    {
        _headset = headset;
        Name = name;
        Subtitle = subtitle;
    }

    [ObservableProperty]
    private int batteryPercent;

    [ObservableProperty]
    private bool isCharging;

    [ObservableProperty]
    private bool hasBattery;

    public static async Task<HeadsetViewModel?> CreateAsync(HidBackend backend, DeviceInfo info)
    {
        ProxHeadset? headset = await ProxHeadset.OpenAsync(backend, info).ConfigureAwait(false);
        if (headset is null)
            return null; // off/asleep right now

        var vm = new HeadsetViewModel(headset, headset.Name, $"serial {info.Serial ?? "n/a"}")
        {
            Art = ArtCatalog.Load("prox.svg"),
        };
        await vm.RefreshBatteryAsync().ConfigureAwait(false);
        return vm;
    }

    public async Task RefreshBatteryAsync()
    {
        try
        {
            (int percent, bool charging) = await _headset.GetBatteryAsync().ConfigureAwait(false);
            BatteryPercent = percent;
            IsCharging = charging;
            HasBattery = true;
        }
        catch (InvalidOperationException)
        {
            HasBattery = false;
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
        {
            Status = $"battery read: {e.Message}";
        }
    }

    public override ValueTask DisposeAsync() => _headset.DisposeAsync();
}
