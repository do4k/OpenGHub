using CommunityToolkit.Mvvm.ComponentModel;
using LogiTux.Core.Devices.Mouse;
using LogiTux.Core.Hid;
using LogiTux.Core.Hidpp;

namespace LogiTux.App.ViewModels;

/// <summary>
/// Card for a G Pro Wireless family mouse: DPI + report rate controls
/// backed by GProMouse, plus battery. Hardware is asleep-gated, so every read
/// can fail with a timeout — surfaced via Status.
/// </summary>
public sealed partial class MouseViewModel : DeviceCardViewModel
{
    private readonly GProMouse _mouse;
    private bool _suppressRateWrite;

    private MouseViewModel(GProMouse mouse, string name, string subtitle)
    {
        _mouse = mouse;
        Name = name;
        Subtitle = subtitle;
    }

    [ObservableProperty]
    private int dpi;

    [ObservableProperty]
    private int dpiMin;

    [ObservableProperty]
    private int dpiMax;

    [ObservableProperty]
    private int dpiStep = 50;

    [ObservableProperty]
    private IReadOnlyList<int> reportRates = Array.Empty<int>();

    [ObservableProperty]
    private int selectedReportRate;

    [ObservableProperty]
    private int batteryPercent;

    [ObservableProperty]
    private bool isCharging;

    [ObservableProperty]
    private bool hasBattery;

    public static async Task<MouseViewModel?> CreateAsync(HidBackend backend, DeviceInfo info)
    {
        GProMouse? mouse = await GProMouse.OpenAsync(backend, info).ConfigureAwait(false);
        if (mouse is null)
            return null; // duplicate receiver sibling — the canonical node builds the card

        var vm = new MouseViewModel(mouse, mouse.Name, $"serial {mouse.Serial}")
        {
            Art = ArtCatalog.Load("gpro.svg"),
        };
        await vm.LoadAsync().ConfigureAwait(false);
        return vm;
    }

    /// <summary>One-line tile status: battery plus DPI/rate when we have them.</summary>
    private void UpdateSummary()
    {
        Summary = HasBattery
            ? $"{BatteryPercent}% battery · {Dpi} DPI"
            : $"{Dpi} DPI";
    }

    private async Task LoadAsync()
    {
        IsBusy = true;
        try
        {
            (int min, int max, int step) = _mouse.DpiRange;
            DpiMin = min;
            DpiMax = max;
            DpiStep = step;

            Dpi = await _mouse.GetDpiAsync().ConfigureAwait(false);
            ReportRates = _mouse.ReportRateOptions.ToList();
            if (ReportRates.Count > 0)
            {
                _suppressRateWrite = true;
                SelectedReportRate = await _mouse.GetReportRateAsync().ConfigureAwait(false);
                _suppressRateWrite = false;
            }

            await RefreshBatteryAsync().ConfigureAwait(false);
            Status = "ready";
        }
        catch (TimeoutException)
        {
            Status = "mouse is asleep — move it and press Refresh";
        }
        catch (Exception e) when (e is HidppErrorException or InvalidDataException or IOException)
        {
            Status = e.Message;
        }
        finally
        {
            IsBusy = false;
        }
    }

    partial void OnDpiChanged(int value) => _ = DebounceWriteAsync(() => _mouse.SetDpiAsync(value), 300);

    partial void OnSelectedReportRateChanged(int value)
    {
        if (_suppressRateWrite)
            return;
        _ = DebounceWriteAsync(() => _mouse.SetReportRateAsync(value), 100);
    }

    public async Task RefreshBatteryAsync()
    {
        try
        {
            (int percent, bool charging) = await _mouse.GetBatteryAsync().ConfigureAwait(false);
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
        UpdateSummary();
    }

    public override ValueTask DisposeAsync() => _mouse.DisposeAsync();
}
