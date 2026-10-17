using System.Text.Json.Serialization;

namespace OpenGHub.Core.Config;

/// <summary>
/// Last-known state of a single device, keyed by serial number in
/// ConfigStore. Litra lights can't report their current brightness, color
/// temperature, or power state back over USB, so Open G Hub remembers the last
/// values it sent and uses them to seed the GUI and to compute relative
/// adjustments (e.g. "brightness up 10").
/// Ported from internal/config/config.go.
/// </summary>
public sealed class DeviceState
{
    /// <summary>
    /// Gets or sets the remembered power state.
    /// </summary>
    public bool Power { get; set; }

    /// <summary>
    /// Gets or sets the remembered brightness.
    /// </summary>
    public int Brightness { get; set; }

    /// <summary>
    /// Gets or sets the remembered color temperature in Kelvin.
    /// </summary>
    public int Temperature { get; set; }

    /// <summary>
    /// Gets or sets the mouse's current DPI.
    /// </summary>
    [JsonPropertyName("dpi")]
    public int Dpi { get; set; }

    /// <summary>
    /// Gets or sets the mouse's report rate in Hz.
    /// </summary>
    [JsonPropertyName("report_rate")]
    public int ReportRate { get; set; }

    /// <summary>
    /// Gets or sets the user's DPI presets ("stages") for a mouse, the
    /// quick-pick values shown on the Sensitivity page. The hardware
    /// exposes only a single current DPI (device.DPIControl); the stage
    /// list is an Open G Hub convenience, persisted here. Null means "use the
    /// defaults derived from the device's DPI range".
    /// </summary>
    [JsonPropertyName("dpi_stages")]
    public List<int>? DpiStages { get; set; }

    /// <summary>
    /// Gets or sets the red component of the LED color.
    /// </summary>
    public byte Red { get; set; }

    /// <summary>
    /// Gets or sets the green component of the LED color.
    /// </summary>
    public byte Green { get; set; }

    /// <summary>
    /// Gets or sets the blue component of the LED color.
    /// </summary>
    public byte Blue { get; set; }

    /// <summary>
    /// Gets or sets a map of device-defined control ids to the uinput
    /// target code they're remapped to. Absent entries mean "default
    /// (unremapped)".
    /// </summary>
    [JsonPropertyName("button_remaps")]
    public Dictionary<ushort, ushort>? ButtonRemaps { get; set; }

    /// <summary>
    /// Gets or sets the headset's sidetone level.
    /// </summary>
    public int Sidetone { get; set; }

    /// <summary>
    /// Gets or sets one dB value per band, in the EqualizerControl band
    /// order.
    /// </summary>
    [JsonPropertyName("equalizer_levels")]
    public List<int>? EqualizerLevels { get; set; }
}