using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LogiTux.Core.Config;
using LogiTux.Core.Devices.Headset;
using LogiTux.Core.Hidpp;

namespace LogiTux.App.ViewModels.Sections;

/// <summary>
/// The headset's equalizer, G HUB-style: a bank of vertical sliders, one
/// per band, with the frequency above and the current dB below. The
/// hardware writes every band at once, so each slider's debounce flushes
/// the whole bank. Ported from cmd/logitux/ui.go (equalizerShowcase).
/// </summary>
public sealed partial class EqualizerViewModel : ObservableObject
{
    private readonly ProxHeadset _headset;
    private readonly ConfigStore? _store;
    private readonly string _serial;

    public IReadOnlyList<RangeSettingViewModel> Bands { get; private set; } = [];

    public bool HasEqualizer { get; private set; }

    [ObservableProperty]
    private string rangeText = "";

    public EqualizerViewModel(ProxHeadset headset, ConfigStore? store, string serial)
    {
        _headset = headset;
        _store = store;
        _serial = serial;
    }

    /// <summary>
    /// Seeds the band bank from the device where the read succeeds,
    /// falling back to the saved levels as the Go UI does.
    /// </summary>
    public async Task LoadAsync()
    {
        if (!_headset.HasEqualizer)
            return;

        var saved = new DeviceState();
        _store?.TryGet(_serial, out saved);
        IReadOnlyList<int> frequencies = _headset.EqFrequencies;
        (int min, int max) = _headset.EqRange;
        RangeText = $"{FormatFrequency(frequencies[0])} – {FormatFrequency(frequencies[^1])} · {min}..{max} dB";

        int[] levels = new int[frequencies.Count];
        Array.Fill(levels, 0);
        try
        {
            IReadOnlyList<int> live = await _headset.GetEqLevelsAsync().ConfigureAwait(false);
            if (live.Count == frequencies.Count)
                levels = [.. live];
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
            if (saved?.EqualizerLevels is { Count: > 0 } savedLevels && savedLevels.Count == frequencies.Count)
                levels = [.. savedLevels];
        }

        var bands = new List<RangeSettingViewModel>(frequencies.Count);
        for (int i = 0; i < frequencies.Count; i++)
        {
            int index = i;
            bands.Add(new RangeSettingViewModel(
                FormatFrequency(frequencies[i]),
                value => $"{value:+#;-#;0} dB",
                levels[i], min, max, 1,
                value => WriteLevelsAsync()));
        }
        Bands = bands;
        HasEqualizer = true;
        OnPropertyChanged(nameof(Bands));
        OnPropertyChanged(nameof(HasEqualizer));
    }

    [RelayCommand]
    private void Reset()
    {
        foreach (RangeSettingViewModel band in Bands)
            band.Value = 0;
        _ = WriteLevelsAsync();
    }

    private async Task WriteLevelsAsync()
    {
        try
        {
            await _headset.SetEqLevelsAsync(Bands.Select(band => band.Value).ToList()).ConfigureAwait(false);
            _store?.Set(_serial, MutateState(s => s.EqualizerLevels = Bands.Select(band => band.Value).ToList()));
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
        }
    }

    private DeviceState MutateState(Action<DeviceState> mutate)
    {
        if (_store is null || !_store.TryGet(_serial, out DeviceState state))
            state = new DeviceState();
        mutate(state);
        return state;
    }

    /// <summary>
    /// Renders a band frequency compactly, e.g. 125 -> "125 Hz",
    /// 8000 -> "8 kHz". Ported from formatFrequency in cmd/logitux/ui.go.
    /// </summary>
    public static string FormatFrequency(int hz)
    {
        if (hz >= 1000)
            return $"{hz / 1000.0:0.#} kHz";
        return $"{hz} Hz";
    }
}