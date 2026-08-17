using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LogiTux.Core.Hid;

namespace LogiTux.App.ViewModels;

public partial class MainWindowViewModel : ObservableObject, IAsyncDisposable
{
    private readonly HidBackend _backend;
    private readonly DispatcherTimer _batteryTimer;
    private List<DeviceCardViewModel> _devices = [];

    public MainWindowViewModel(HidBackend? backend = null)
    {
        _backend = backend ?? new HidBackend();
        _batteryTimer = new DispatcherTimer(TimeSpan.FromSeconds(30), DispatcherPriority.Background, OnBatteryTick);
        _ = RefreshAsync();
    }

    [ObservableProperty]
    private IReadOnlyList<DeviceCardViewModel> devices = Array.Empty<DeviceCardViewModel>();

    [ObservableProperty]
    private bool isRefreshing;

    [ObservableProperty]
    private DeviceCardViewModel? selected;

    /// <summary>True while a device page is open; the dashboard shows otherwise.</summary>
    public bool IsDetail => Selected is not null;

    /// <summary>True when no devices are connected; shows the empty-state hint.</summary>
    public bool IsEmpty => Devices.Count == 0;

    partial void OnSelectedChanged(DeviceCardViewModel? value)
    {
        OnPropertyChanged(nameof(IsDetail));
    }

    partial void OnDevicesChanged(IReadOnlyList<DeviceCardViewModel> value)
    {
        OnPropertyChanged(nameof(IsEmpty));
    }

    [RelayCommand]
    private void Back() => Selected = null;

    [RelayCommand]
    private async Task RefreshAsync()
    {
        if (IsRefreshing)
            return;
        IsRefreshing = true;
        try
        {
            foreach (DeviceCardViewModel old in _devices)
                await old.DisposeAsync().ConfigureAwait(false);
            _devices = (await DeviceManager.DiscoverAsync(_backend).ConfigureAwait(false)).ToList();
            foreach (DeviceCardViewModel card in _devices)
                card.GoToDetail = () => Selected = card;
            Devices = _devices;
            _batteryTimer.Start();
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"LogiTux: device discovery failed: {e}");
            Devices = [];
        }
        finally
        {
            IsRefreshing = false;
        }
    }

    private void OnBatteryTick(object? sender, EventArgs e)
    {
        foreach (DeviceCardViewModel card in _devices)
        {
            switch (card)
            {
                case MouseViewModel mouse:
                    _ = mouse.RefreshBatteryAsync();
                    break;
                case HeadsetViewModel headset:
                    _ = headset.RefreshBatteryAsync();
                    break;
            }
        }
    }

    public async ValueTask DisposeAsync()
    {
        _batteryTimer.Stop();
        foreach (DeviceCardViewModel card in _devices)
            await card.DisposeAsync().ConfigureAwait(false);
    }
}