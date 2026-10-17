using OpenGHub.Core.Hidpp;

namespace OpenGHub.Core.Devices.Mouse;

public sealed partial class GProMouse
{
    // COLOR_LED_EFFECTS (0x8070) function numbers. Ported from
    // internal/device/gpro/rgb.go.
    private const byte LedFuncGetInfo = 0x00;
    private const byte LedFuncGetZoneInfo = 0x10;
    private const byte LedFuncGetZoneEffectInfo = 0x20;
    private const byte LedFuncSetZoneEffect = 0x30;
    private const byte LedFuncSetControl = 0x80;

    private const ushort LedZoneLocationLogo = 0x02;
    private const ushort LedEffectIdStatic = 0x01;

    /// <summary>
    /// RGBSupported implements device.RGBSupport: Superlight models have no
    /// RGB lighting at all, so the GUI hides color controls for them.
    /// Ported from internal/device/gpro/rgb.go.
    /// </summary>
    public bool RgbSupported => _ledFeatureIndex != 0;

    /// <summary>
    /// Locates the mouse's Logo LED zone and its "Static" effect slot,
    /// resolved once during discovery: SetZoneEffect addresses an effect
    /// by its position in that zone's own effect list, not by the effect's ID.
    /// Walks GetInfo -&gt; GetZoneInfo -&gt; GetZoneEffectInfo to find the
    /// Logo zone and its Static effect.
    /// Ported from internal/device/gpro/rgb.go (discoverLogoZone).
    /// </summary>
    internal static async Task<(byte ZoneIndex, byte StaticEffectIndex)> DiscoverLogoZoneAsync(HidppConn conn, byte deviceIndex, byte featureIndex)
    {
        var info = await conn.CallAsync(deviceIndex, featureIndex, LedFuncGetInfo, Array.Empty<byte>()).ConfigureAwait(false);
        if (info.Length < 1)
            throw new InvalidDataException("gpro: short LED GetInfo response");
        int zoneCount = info[0];

        for (int zone = 0; zone < zoneCount; zone++)
        {
            var zi = await conn.CallAsync(deviceIndex, featureIndex, LedFuncGetZoneInfo, [(byte)zone, 0xff, 0x00]).ConfigureAwait(false);
            if (zi.Length < 4)
                throw new InvalidDataException("gpro: short LED GetZoneInfo response");
            ushort location = (ushort)((zi[1] << 8) | zi[2]);
            if (location != LedZoneLocationLogo)
                continue;
            int effectCount = zi[3];

            for (int e = 0; e < effectCount; e++)
            {
                var ei = await conn.CallAsync(deviceIndex, featureIndex, LedFuncGetZoneEffectInfo, [(byte)zone, (byte)e, 0x00]).ConfigureAwait(false);
                if (ei.Length < 4)
                    throw new InvalidDataException("gpro: short LED GetZoneEffectInfo response");
                ushort effectId = (ushort)((ei[2] << 8) | ei[3]);
                if (effectId == LedEffectIdStatic)
                    return ((byte)zone, (byte)e);
            }
            throw new InvalidOperationException("gpro: Logo LED zone has no Static effect");
        }
        throw new InvalidOperationException("gpro: no Logo LED zone found");
    }

    /// <summary>
    /// Switches the LED zones to host control. Without this, the device's
    /// onboard profile keeps driving the LED and a written color may not
    /// become visible. Called before every write rather than once at open
    /// time so it's unaffected by the device resetting control mode across
    /// a sleep/wake cycle.
    /// Ported from internal/device/gpro/rgb.go (claimLEDControl).
    /// </summary>
    internal static async Task ClaimLedControlAsync(HidppConn conn, byte deviceIndex, byte featureIndex)
    {
        await conn.CallAsync(deviceIndex, featureIndex, LedFuncSetControl, [0x01]).ConfigureAwait(false);
    }

    /// <summary>
    /// Sets the Logo LED to a static color.
    /// Ported from internal/device/gpro/rgb.go (SetColor).
    /// </summary>
    public async Task SetColorAsync(byte r, byte g, byte b)
    {
        if (_ledFeatureIndex == 0)
            throw new InvalidOperationException($"gpro: {_name} has no RGB lighting");
        await ClaimLedControlAsync(_conn, _deviceIndex, _ledFeatureIndex).ConfigureAwait(false);

        // zoneIndex, effectListIndex, then a 10-byte effect parameter block
        // (color R,G,B, ramp, and 6 zero-padding bytes for the Static effect).
        byte[] payload =
        [
            _ledZoneIndex,
            _ledStaticEffectIndex,
            r,
            g,
            b,
            0x00, // ramp: Default
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        await _conn.CallAsync(_deviceIndex, _ledFeatureIndex, LedFuncSetZoneEffect, payload).ConfigureAwait(false);
    }
}