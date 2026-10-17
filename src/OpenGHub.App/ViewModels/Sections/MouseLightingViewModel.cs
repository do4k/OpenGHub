using Avalonia.Media;
using CommunityToolkit.Mvvm.ComponentModel;
using OpenGHub.Core.Config;
using OpenGHub.Core.Devices.Mouse;
using OpenGHub.Core.Hidpp;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// LIGHTSYNC section: a static logo color for RGB-equipped G Pro mice,
/// edited as three channels behind a live swatch. Hardware has no color
/// read-back, so the picker seeds from the saved state, defaulting to
/// white. Ported from cmd/openghub/ui.go (lightingPanel's RGB block).
/// </summary>
public sealed partial class MouseLightingViewModel : ObservableObject
{
    private readonly GProMouse _mouse;
    private readonly ConfigStore? _store;
    private readonly string _serial;
    private CancellationTokenSource? _debounceCts;

    [ObservableProperty]
    private int red;

    [ObservableProperty]
    private int green;

    [ObservableProperty]
    private int blue;

    /// <summary>Live composite of the channels, for the swatch.</summary>
    public Color RgbColor => Color.FromRgb((byte)Red, (byte)Green, (byte)Blue);

    /// <summary>Swatch brush mirroring <see cref="RgbColor"/>.</summary>
    public IBrush RgbBrush => new SolidColorBrush(RgbColor);

    public MouseLightingViewModel(GProMouse mouse, ConfigStore? store, string serial)
    {
        _mouse = mouse;
        _store = store;
        _serial = serial;
        if (store is not null && store.TryGet(serial, out DeviceState saved) && saved.Red + saved.Green + saved.Blue > 0)
        {
            Red = saved.Red;
            Green = saved.Green;
            Blue = saved.Blue;
        }
        else
        {
            Red = 0xff;
            Green = 0xff;
            Blue = 0xff;
        }
    }

    partial void OnRedChanged(int value) => ChannelChanged();
    partial void OnGreenChanged(int value) => ChannelChanged();
    partial void OnBlueChanged(int value) => ChannelChanged();

    private void ChannelChanged()
    {
        OnPropertyChanged(nameof(RgbColor));
        OnPropertyChanged(nameof(RgbBrush));
        _debounceCts?.Cancel();
        var cts = new CancellationTokenSource();
        _debounceCts = cts;
        _ = WriteAsync(cts);
    }

    private async Task WriteAsync(CancellationTokenSource cts)
    {
        try
        {
            await Task.Delay(200, cts.Token);
            if (cts.IsCancellationRequested)
                return;
            await _mouse.SetColorAsync((byte)Red, (byte)Green, (byte)Blue).ConfigureAwait(false);
            _store?.Set(_serial, MutateState());
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
        }
    }

    private DeviceState MutateState()
    {
        if (_store is null || !_store.TryGet(_serial, out DeviceState state))
            state = new DeviceState();
        state.Red = (byte)Red;
        state.Green = (byte)Green;
        state.Blue = (byte)Blue;
        return state;
    }
}