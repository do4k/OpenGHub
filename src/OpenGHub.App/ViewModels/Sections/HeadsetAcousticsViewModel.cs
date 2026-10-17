using CommunityToolkit.Mvvm.ComponentModel;
using OpenGHub.Core.Config;
using OpenGHub.Core.Devices.Headset;
using OpenGHub.Core.Hidpp;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// Headset Acoustics section: the sidetone (mic monitoring) slider and,
/// when the unit's firmware exposes it, hardware mic noise reduction.
/// Ported from cmd/openghub/ui.go (soundPanel).
/// </summary>
public sealed partial class HeadsetAcousticsViewModel : ObservableObject
{
    private readonly ProxHeadset _headset;
    private readonly ConfigStore? _store;
    private readonly string _serial;

    public RangeSettingViewModel? Sidetone { get; private set; }

    public bool HasSidetone => Sidetone is not null;

    public ToggleSettingViewModel? NoiseReduction { get; private set; }

    public bool HasNoiseReduction => NoiseReduction is not null;

    public HeadsetAcousticsViewModel(ProxHeadset headset, ConfigStore? store, string serial)
    {
        _headset = headset;
        _store = store;
        _serial = serial;
    }

    /// <summary>Reads live sidetone and noise-reduction state.</summary>
    public async Task LoadAsync()
    {
        var saved = new DeviceState();
        _store?.TryGet(_serial, out saved);

        int initial = saved?.Sidetone ?? 0;
        try
        {
            initial = await _headset.GetSidetoneAsync().ConfigureAwait(false);
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
        }
        Sidetone = new RangeSettingViewModel(
            "Sidetone", value => $"{value}%", initial, 0, 100, 1,
            value =>
            {
                _store?.Set(_serial, MutateState(s => s.Sidetone = value));
                return _headset.SetSidetoneAsync(value);
            });
        OnPropertyChanged(nameof(Sidetone));
        OnPropertyChanged(nameof(HasSidetone));

        try
        {
            bool nr = await _headset.GetNoiseReductionAsync().ConfigureAwait(false);
            NoiseReduction = new ToggleSettingViewModel("Mic Noise Reduction", nr, _headset.SetNoiseReductionAsync);
            OnPropertyChanged(nameof(NoiseReduction));
            OnPropertyChanged(nameof(HasNoiseReduction));
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
            // Firmware predating the functions answers with a timeout;
            // the toggle simply doesn't appear.
        }
    }

    private DeviceState MutateState(Action<DeviceState> mutate)
    {
        if (_store is null || !_store.TryGet(_serial, out DeviceState state))
            state = new DeviceState();
        mutate(state);
        return state;
    }
}