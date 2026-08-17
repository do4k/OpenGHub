namespace LogiTux.Core.Devices.Litra;

/// <summary>
/// Logitech Litra Glow / Litra Beam key lights. These speak a simple
/// vendor-defined USB HID protocol (not HID++): each command is a fixed
/// 20-byte output report starting with report ID 0x11 and a 0xff 0x04 feature
/// prefix, followed by a one-byte command and up to two payload bytes. There is
/// no way to read state back, so callers track last-known state themselves.
/// Ported from internal/device/litra/litra.go.
/// </summary>
public sealed class LitraLight
{
    public const ushort VendorId = 0x046d;
    public const ushort ProductGlow = 0xc900;
    public const ushort ProductBeam = 0xc901;

    // Raw brightness values the device accepts; exposed as 0-100 percent and
    // scaled at the boundary.
    private const byte MinBrightnessRaw = 0x14;
    private const byte MaxBrightnessRaw = 0xfa;

    public const int MinTemperatureK = 2700;
    public const int MaxTemperatureK = 6500;

    private readonly Stream _stream;

    public LitraLight(Stream stream) => _stream = stream;

    public static string FriendlyName(ushort productId) => productId switch
    {
        ProductGlow => "Litra Glow",
        ProductBeam => "Litra Beam",
        _ => $"Litra ({productId:x4})",
    };

    public async Task SetPowerAsync(bool on)
    {
        byte powerByte = on ? (byte)0x01 : (byte)0x00;
        await SendAsync(0x1c, powerByte).ConfigureAwait(false);
    }

    /// <summary><paramref name="percent"/> is clamped to 0-100 and scaled to the
    /// device's raw brightness range.</summary>
    public async Task SetBrightnessAsync(int percent)
    {
        percent = Math.Clamp(percent, 0, 100);
        float span = MaxBrightnessRaw - MinBrightnessRaw;
        byte raw = (byte)(MinBrightnessRaw + MathF.Round(percent / 100f * span));
        await SendAsync(0x4c, 0x00, raw).ConfigureAwait(false);
    }

    /// <summary><paramref name="kelvin"/> is clamped to the device's supported
    /// range before being sent.</summary>
    public async Task SetTemperatureAsync(int kelvin)
    {
        kelvin = Math.Clamp(kelvin, MinTemperatureK, MaxTemperatureK);
        await SendAsync(0x9c, (byte)(kelvin >> 8), (byte)kelvin).ConfigureAwait(false);
    }

    /// <summary>Builds and writes a 20-byte Litra report: report ID 0x11, the
    /// 0xff 0x04 lighting-feature prefix, a one-byte command, then payload bytes
    /// zero-padded to the device's fixed report length.</summary>
    private async Task SendAsync(byte command, params byte[] payload)
    {
        var report = new byte[20];
        report[0] = 0x11;
        report[1] = 0xff;
        report[2] = 0x04;
        report[3] = command;
        payload.CopyTo(report, 4);
        await _stream.WriteAsync(report).ConfigureAwait(false);
    }
}