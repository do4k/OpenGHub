using CommunityToolkit.Mvvm.ComponentModel;

namespace OpenGHub.App.ViewModels.Sections;

/// <summary>
/// Approximates Go's labeledSlider: a named slider row with a bold live
/// readout beside it. Writes are debounced so a drag doesn't spam the
/// device, mirroring the other cards.
/// </summary>
public sealed partial class RangeSettingViewModel : ObservableObject
{
    private readonly Func<int, string> _format;
    private readonly Func<int, Task> _write;
    private CancellationTokenSource? _debounceCts;

    public string Name { get; }
    public int Min { get; }
    public int Max { get; }
    public int Step { get; }

    [ObservableProperty]
    private int value;

    [ObservableProperty]
    private string valueText = "";

    public RangeSettingViewModel(string name, Func<int, string> format, int initial, int min, int max, int step, Func<int, Task> write)
    {
        Name = name;
        _format = format;
        Min = min;
        Max = max;
        Step = step;
        _write = write;
        Value = initial;
        ValueText = format(initial);
    }

    partial void OnValueChanged(int value)
    {
        ValueText = _format(value);
        _debounceCts?.Cancel();
        var cts = new CancellationTokenSource();
        _debounceCts = cts;
        _ = WriteAsync(cts, value);
    }

    private async Task WriteAsync(CancellationTokenSource cts, int value)
    {
        try
        {
            await Task.Delay(150, cts.Token);
            if (!cts.IsCancellationRequested)
                await _write(value);
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception)
        {
            // Slider writes can fail while a device is asleep; the next
            // drag retries. Not worth surfacing on every tick.
        }
    }
}