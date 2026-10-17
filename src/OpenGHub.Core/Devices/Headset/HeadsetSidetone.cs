using OpenGHub.Core.Hidpp;

namespace OpenGHub.Core.Devices.Headset;

/// <summary>
/// SIDETONE (0x8300) support for the PRO X Wireless headset. Ported from
/// internal/device/prox/sidetone.go.
/// </summary>
public sealed partial class ProxHeadset
{
    // SIDETONE (0x8300) function numbers (both are HID++'s defaults: read 0,
    // write 0x10 — this feature doesn't override them).
    private const byte SidetoneFuncGet = 0x00;
    private const byte SidetoneFuncSet = 0x10;

    /// <summary>
    /// Reads the current sidetone (microphone monitoring) volume, as a
    /// percent. Ported from internal/device/prox/sidetone.go.
    /// </summary>
    public async Task<int> GetSidetoneAsync()
    {
        if (_sidetoneFeatureIndex == 0)
            throw new InvalidOperationException("prox: device has no sidetone feature");
        var resp = await _conn.CallAsync(HidppConn.DeviceIndexDirect, _sidetoneFeatureIndex, SidetoneFuncGet, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 1)
            throw new InvalidDataException("prox: short sidetone response");
        return resp[0];
    }

    /// <summary>
    /// Sets the sidetone (microphone monitoring) volume, as a percent
    /// clamped to 0..100. Ported from internal/device/prox/sidetone.go.
    /// </summary>
    public async Task SetSidetoneAsync(int percent)
    {
        if (_sidetoneFeatureIndex == 0)
            throw new InvalidOperationException("prox: device has no sidetone feature");
        var clamped = Math.Clamp(percent, 0, 100);
        await _conn.CallAsync(HidppConn.DeviceIndexDirect, _sidetoneFeatureIndex, SidetoneFuncSet, [(byte)clamped]).ConfigureAwait(false);
    }
}