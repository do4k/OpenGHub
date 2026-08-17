using Avalonia.Svg.Skia;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace LogiTux.App.ViewModels;

/// <summary>
/// Base for a device card shown on the dashboard. Card instances are shared
/// with their dedicated device page: tiles show Name/Summary/Art, the page
/// shows the full control surface.
/// </summary>
public abstract partial class DeviceCardViewModel : ObservableObject, IAsyncDisposable
{
    [ObservableProperty]
    private string status = "";

    [ObservableProperty]
    private bool isBusy;

    [ObservableProperty]
    private string summary = "";

    public string Name { get; init; } = "";

    public string Subtitle { get; init; } = "";

    /// <summary>Uppercase marketing name, the way G HUB and the Go UI show it.</summary>
    public string DisplayName => Name.ToUpperInvariant();

    /// <summary>Product render shown on the tile; null hides it.</summary>
    public SvgImage? Art { get; init; }

    /// <summary>Invoked by <see cref="OpenCommand"/> (wired by the window VM)
    /// to navigate from the dashboard tile to this device's page.</summary>
    public Action? GoToDetail { get; set; }

    [RelayCommand]
    private void Open() => GoToDetail?.Invoke();

    /// <summary>Debounces slider-driven writes so dragging doesn't spam the device.</summary>
    protected CancellationTokenSource? DebounceCts { get; private set; }

    protected async Task DebounceWriteAsync(Func<Task> write, int delayMs = 250)
    {
        DebounceCts?.Cancel();
        var cts = new CancellationTokenSource();
        DebounceCts = cts;
        try
        {
            await Task.Delay(delayMs, cts.Token);
            if (!cts.IsCancellationRequested)
                await write();
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception e)
        {
            Status = $"write failed: {e.Message}";
        }
    }

    public virtual ValueTask DisposeAsync() => ValueTask.CompletedTask;
}