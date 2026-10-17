using CommunityToolkit.Mvvm.ComponentModel;
using OpenGHub.Core.Config;
using OpenGHub.Core.Devices.Mouse;
using OpenGHub.Core.Hid;
using OpenGHub.Core.Hidpp;
using SectionsVMs = OpenGHub.App.ViewModels.Sections;

namespace OpenGHub.App.ViewModels;

/// <summary>
/// Card and page for a G Pro Wireless family mouse: Sensitivity (DPI
/// stages + report rate), Assignments (stub surface) and LIGHTSYNC RGB
/// sections backed by GProMouse, plus battery. Hardware is asleep-gated,
/// so every read can fail with a timeout — surfaced via Status.
/// </summary>
public sealed partial class MouseViewModel : DevicePageViewModel
{
    private readonly GProMouse _mouse;
    private readonly ConfigStore? _store;
    private readonly SectionsVMs.MouseSensitivityViewModel _sensitivity;

    [ObservableProperty]
    private int dpi;

    [ObservableProperty]
    private int batteryPercent;

    [ObservableProperty]
    private bool isCharging;

    [ObservableProperty]
    private bool hasBattery;

    private MouseViewModel(GProMouse mouse, string name, string subtitle, ConfigStore? store)
    {
        _mouse = mouse;
        _store = store;
        Name = name;
        Subtitle = subtitle;
        _sensitivity = new SectionsVMs.MouseSensitivityViewModel(mouse, store, mouse.Serial, value => Dpi = value);
    }

    public static async Task<MouseViewModel?> CreateAsync(HidBackend backend, DeviceInfo info, ConfigStore? store = null)
    {
        GProMouse? mouse = await GProMouse.OpenAsync(backend, info).ConfigureAwait(false);
        if (mouse is null)
            return null; // duplicate receiver sibling — the canonical node builds the card

        var vm = new MouseViewModel(mouse, mouse.Name, $"serial {mouse.Serial}", store)
        {
            Art = ArtCatalog.Load("gpro.svg"),
        };
        await vm.LoadAsync().ConfigureAwait(false);
        return vm;
    }

    /// <summary>One-line tile status: battery plus DPI when we have them.</summary>
    private void UpdateSummary()
    {
        Summary = HasBattery
            ? $"{BatteryPercent}% battery · {Dpi} DPI"
            : $"{Dpi} DPI";
    }

    private async Task LoadAsync()
    {
        InstallSections([]);
        await _sensitivity.LoadAsync().ConfigureAwait(false);
        await RefreshBatteryAsync().ConfigureAwait(false);

        var sections = new List<SectionsVMs.SectionViewModel>
        {
            new("SENSITIVITY", "⊕", _sensitivity),
            new("ASSIGNMENTS", "✥", new SectionsVMs.MouseAssignmentsViewModel()),
        };
        if (_mouse.RgbSupported)
            sections.Add(new SectionsVMs.SectionViewModel("LIGHTSYNC", "◉", new SectionsVMs.MouseLightingViewModel(_mouse, _store, _mouse.Serial)));
        InstallSections(sections);
        Status = "ready";
    }

    partial void OnDpiChanged(int value) => UpdateSummary();

    public async Task RefreshBatteryAsync()
    {
        try
        {
            (int percent, bool charging) = await _mouse.GetBatteryAsync().ConfigureAwait(false);
            BatteryPercent = percent;
            IsCharging = charging;
            HasBattery = true;
            _sensitivity.UpdateBattery(percent, charging);
        }
        catch (InvalidOperationException)
        {
            HasBattery = false;
            _sensitivity.UpdateBattery(0, false);
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
        {
            Status = $"battery read: {e.Message}";
        }
        UpdateSummary();
    }

    public override ValueTask DisposeAsync() => _mouse.DisposeAsync();
}