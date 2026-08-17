using Avalonia.Svg.Skia;
using CommunityToolkit.Mvvm.ComponentModel;

namespace LogiTux.App.ViewModels;

/// <summary>Base for a device card shown on the dashboard.</summary>
public abstract partial class DeviceCardViewModel : ObservableObject, IAsyncDisposable
{
    [ObservableProperty]
    private string status = "";

    [ObservableProperty]
    private bool isBusy;

    public string Name { get; init; } = "";

    public string Subtitle { get; init; } = "";

    /// <summary>Product render shown at the top of the card; null hides it.</summary>
    public SvgImage? Art { get; init; }

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