using CommunityToolkit.Mvvm.ComponentModel;
using OpenGHub.Core.Config;
using OpenGHub.Core.Devices.Mouse;
using OpenGHub.Core.Hidpp;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// The mouse's Sensitivity page, G HUB-style: DPI stage buttons (the
/// active stage highlighted) and report-rate options below, plus the
/// battery line. Stage sets and the current pick persist to the config
/// store. Ported from cmd/openghub/ui.go (sensitivityPanel + loadDPIStages).
/// </summary>
public sealed partial class MouseSensitivityViewModel : ObservableObject
{
    private readonly GProMouse _mouse;
    private readonly ConfigStore? _store;
    private readonly string _serial;
    private readonly Action<int>? _onDpiChanged;

    public IReadOnlyList<DpiStageViewModel> Stages { get; private set; } = [];

    public IReadOnlyList<ReportRateOptionViewModel>? ReportRateOptions { get; private set; }

    public bool HasReportRate => ReportRateOptions is { Count: > 0 };

    [ObservableProperty]
    private string batteryText = "";

    public MouseSensitivityViewModel(GProMouse mouse, ConfigStore? store, string serial, Action<int>? onDpiChanged)
    {
        _mouse = mouse;
        _store = store;
        _serial = serial;
        _onDpiChanged = onDpiChanged;
    }

    /// <summary>
    /// Loads the DPI range, stages and active stage from the live device
    /// where possible, then the report-rate options.
    /// </summary>
    public async Task LoadAsync()
    {
        try
        {
            (int min, int max, int step) = _mouse.DpiRange;
            if (_store is not null && _store.TryGet(_serial, out DeviceState saved))
            {
                await LoadStagesAsync(min, max, saved).ConfigureAwait(false);
                await LoadRatesAsync(saved).ConfigureAwait(false);
            }
            else
            {
                await LoadStagesAsync(min, max, null).ConfigureAwait(false);
                await LoadRatesAsync(null).ConfigureAwait(false);
            }
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
            // The section stays empty rather than bricking the page; the
            // page header line shows the device wasn't reachable.
        }
    }

    private async Task LoadStagesAsync(int min, int max, DeviceState? saved)
    {
        List<int> stages = SanitizeStages(saved?.DpiStages ?? DefaultDpiStages(min, max), min, max);

        int active = saved?.Dpi ?? 0;
        try
        {
            active = await _mouse.GetDpiAsync().ConfigureAwait(false);
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
        }
        if (active <= 0)
            active = stages[0];
        if (active >= min && active <= max && !stages.Contains(active))
            stages = SanitizeStages([.. stages, active], min, max);

        var vms = new List<DpiStageViewModel>(stages.Count);
        foreach (int stage in stages)
        {
            var vm = new DpiStageViewModel(stage, value => _ = PickStageAsync(value, stages));
            vm.IsActive = stage == active;
            vms.Add(vm);
        }
        Stages = vms;
        OnPropertyChanged(nameof(Stages));
    }

    private async Task LoadRatesAsync(DeviceState? saved)
    {
        List<int> rates = _mouse.ReportRateOptions.ToList();
        if (rates.Count == 0)
            return;

        var options = new List<ReportRateOptionViewModel>(rates.Count);
        foreach (int rate in rates)
        {
            var option = new ReportRateOptionViewModel(rate, value =>
            {
                _ = WriteRateAsync(value);
                RefreshActiveRate(value);
            });
            options.Add(option);
        }
        ReportRateOptions = options;
        OnPropertyChanged(nameof(ReportRateOptions));
        OnPropertyChanged(nameof(HasReportRate));

        RefreshActiveRate(saved?.ReportRate is > 0 ? saved!.ReportRate : rates[0]);
        try
        {
            RefreshActiveRate(await _mouse.GetReportRateAsync().ConfigureAwait(false));
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
        }
    }

    /// <summary>Refreshes the battery line shown under the stages.</summary>
    public void UpdateBattery(int percent, bool charging) =>
        BatteryText = $"{percent}% battery{(charging ? " (charging)" : "")}";

    private async Task PickStageAsync(int value, List<int> stages)
    {
        foreach (DpiStageViewModel vm in Stages)
            vm.IsActive = vm.Value == value;
        try
        {
            await _mouse.SetDpiAsync(value).ConfigureAwait(false);
            _onDpiChanged?.Invoke(value);
            Persist(s => { s.Dpi = value; s.DpiStages = stages; });
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
        }
    }

    private async Task WriteRateAsync(int value)
    {
        try
        {
            await _mouse.SetReportRateAsync(value).ConfigureAwait(false);
            Persist(s => s.ReportRate = value);
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or IOException or InvalidOperationException)
        {
        }
    }

    private void RefreshActiveRate(int value)
    {
        if (ReportRateOptions is null)
            return;
        foreach (ReportRateOptionViewModel option in ReportRateOptions)
            option.IsActive = option.Value == value;
    }

    /// <summary>
    /// Read-modify-write against the store, mirroring Go's saveState.
    /// No-op when no store is configured (Poc runs, tests).
    /// </summary>
    private void Persist(Action<DeviceState> mutate)
    {
        if (_store is null)
            return;
        if (!_store.TryGet(_serial, out DeviceState state))
            state = new DeviceState();
        mutate(state);
        _store.Set(_serial, state);
    }

    private static readonly int[] StagePresets = [400, 800, 1600, 3200, 6400, 12800, 25600];

    private static List<int> DefaultDpiStages(int min, int max)
    {
        List<int> stages = StagePresets.Where(v => v >= min && v <= max).ToList();
        return stages.Count >= 2 ? stages : [min, max];
    }

    /// <summary>
    /// Clamps values into [min, max], drops duplicates and sorts
    /// ascending, falling back to the defaults if nothing usable remains.
    /// Ported from cmd/openghub/ui.go (sanitizeStages).
    /// </summary>
    public static List<int> SanitizeStages(List<int> input, int min, int max)
    {
        var seen = new HashSet<int>();
        var output = new List<int>(input.Count);
        foreach (int v in input)
        {
            int clamped = Math.Clamp(v, min, max);
            if (seen.Add(clamped))
                output.Add(clamped);
        }
        output.Sort();
        return output.Count == 0 ? DefaultDpiStages(min, max) : output;
    }
}