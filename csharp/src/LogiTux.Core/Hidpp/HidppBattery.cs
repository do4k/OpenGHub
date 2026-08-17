namespace LogiTux.Core.Hidpp;

/// <summary>
/// Battery feature IDs, tried in this priority order (most direct/precise first)
/// by <see cref="ResolveBatteryFeatureAsync"/>, since not every device implements
/// the same one. Ported from internal/hidpp/battery.go.
/// </summary>
public static class HidppBattery
{
    public const ushort FeatureUnifiedBattery = 0x1004;
    public const ushort FeatureBatteryStatus = 0x1000;  // legacy: percent + status, but 0% means "unknown"
    public const ushort FeatureBatteryVoltage = 0x1001; // voltage-based estimate
    public const ushort FeatureAdcMeasurement = 0x1f20; // voltage-based estimate, last resort

    /// <summary>Battery status byte values, shared by BATTERY_STATUS and UNIFIED_BATTERY.</summary>
    public const byte StatusDischarging = 0x00;
    public const byte StatusRecharging = 0x01;
    public const byte StatusAlmostFull = 0x02;
    public const byte StatusFull = 0x03;
    public const byte StatusSlowRecharge = 0x04;

    public const byte FuncGetStatusUnified = 0x10;
    public const byte FuncGetStatusLegacy = 0x00;
    public const byte FuncGetVoltage = 0x00;
    public const byte FuncGetAdc = 0x00;

    /// <summary>Identifies which battery feature a device resolved to, since each
    /// has a different function number and response layout.</summary>
    public enum BatteryKind
    {
        None,
        Unified,
        LegacyStatus,
        Voltage,
        Adc,
    }

    /// <summary>Tries each known battery feature in turn and returns whichever the
    /// device supports. Throws only on an actual transport failure; a device with
    /// no battery feature at all yields <see cref="BatteryKind.None"/>.</summary>
    public static async Task<(byte FeatureIndex, BatteryKind Kind)> ResolveBatteryFeatureAsync(HidppConn conn, byte deviceIndex)
    {
        (ushort Id, BatteryKind Kind)[] candidates =
        [
            (FeatureUnifiedBattery, BatteryKind.Unified),
            (FeatureBatteryStatus, BatteryKind.LegacyStatus),
            (FeatureBatteryVoltage, BatteryKind.Voltage),
            (FeatureAdcMeasurement, BatteryKind.Adc),
        ];

        foreach (var c in candidates)
        {
            byte? idx = await HidppRoot.GetFeatureIndexAsync(conn, deviceIndex, c.Id).ConfigureAwait(false);
            if (idx is not null)
                return (idx.Value, c.Kind);
        }
        return (0, BatteryKind.None);
    }

    /// <summary>Reads the current percent/charging state using a feature index and
    /// kind previously resolved by <see cref="ResolveBatteryFeatureAsync"/>.</summary>
    public static async Task<(int Percent, bool Charging)> ReadBatteryAsync(
        HidppConn conn, byte deviceIndex, byte featureIndex, BatteryKind kind)
    {
        switch (kind)
        {
            case BatteryKind.Unified:
                return await FromStatusCallAsync(conn, deviceIndex, featureIndex, FuncGetStatusUnified, zeroMeansUnknown: false).ConfigureAwait(false);
            case BatteryKind.LegacyStatus:
                return await FromStatusCallAsync(conn, deviceIndex, featureIndex, FuncGetStatusLegacy, zeroMeansUnknown: true).ConfigureAwait(false);
            case BatteryKind.Voltage:
                return await FromVoltageCallAsync(conn, deviceIndex, featureIndex, FuncGetVoltage, isAdc: false).ConfigureAwait(false);
            case BatteryKind.Adc:
                return await FromVoltageCallAsync(conn, deviceIndex, featureIndex, FuncGetAdc, isAdc: true).ConfigureAwait(false);
            default:
                throw new InvalidOperationException("hidpp: device has no supported battery feature");
        }
    }

    /// <summary>Handles UNIFIED_BATTERY and BATTERY_STATUS, whose responses both
    /// start with [percent, _, status]. <paramref name="zeroMeansUnknown"/> is only
    /// true for the legacy feature.</summary>
    private static async Task<(int, bool)> FromStatusCallAsync(
        HidppConn conn, byte deviceIndex, byte featureIndex, byte function, bool zeroMeansUnknown)
    {
        byte[] resp = await conn.CallAsync(deviceIndex, featureIndex, function, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 3)
            throw new InvalidDataException("hidpp: short battery status response");

        int percent = resp[0];
        if (percent == 0 && zeroMeansUnknown)
            throw new InvalidDataException("hidpp: battery percentage unavailable");

        return (percent, IsChargingStatus(resp[2]));
    }

    /// <summary>Handles BATTERY_VOLTAGE and ADC_MEASUREMENT, whose responses are
    /// both [voltage(2B BE, mV), flags(1B)]; ADC requires checking a "valid" bit
    /// first, and the two features use different flag bits for "charging".</summary>
    private static async Task<(int, bool)> FromVoltageCallAsync(
        HidppConn conn, byte deviceIndex, byte featureIndex, byte function, bool isAdc)
    {
        byte[] resp = await conn.CallAsync(deviceIndex, featureIndex, function, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 3)
            throw new InvalidDataException("hidpp: short battery voltage response");

        int millivolts = (resp[0] << 8) | resp[1];
        byte flags = resp[2];

        bool charging;
        if (isAdc)
        {
            if ((flags & 0x01) == 0)
                throw new InvalidDataException("hidpp: no valid battery reading available");
            charging = (flags & 0x02) != 0;
        }
        else
        {
            charging = (flags & 0x80) != 0;
        }

        return (EstimateBatteryPercent(millivolts), charging);
    }

    private static bool IsChargingStatus(byte status) =>
        status is StatusRecharging or StatusAlmostFull or StatusFull or StatusSlowRecharge;

    /// <summary>Maps single-cell Li-ion/Li-Po voltage (millivolts) to an estimated
    /// charge percentage, for devices that only report raw voltage. Approximate
    /// discharge-curve values, consistent with the mapping widely used for this
    /// purpose across Linux Logitech tooling.</summary>
    private static readonly (int Millivolts, int Percent)[] BatteryVoltageCurve =
    [
        (4186, 100), (4067, 90), (3989, 80), (3922, 70), (3859, 60), (3811, 50),
        (3778, 40), (3751, 30), (3717, 20), (3671, 10), (3646, 5), (3579, 2), (3500, 0),
    ];

    /// <summary>Estimates a charge percentage from a millivolt reading via linear
    /// interpolation over <see cref="BatteryVoltageCurve"/>.</summary>
    public static int EstimateBatteryPercent(int millivolts)
    {
        if (millivolts >= BatteryVoltageCurve[0].Millivolts)
            return BatteryVoltageCurve[0].Percent;
        if (millivolts <= BatteryVoltageCurve[^1].Millivolts)
            return BatteryVoltageCurve[^1].Percent;

        for (int i = 0; i < BatteryVoltageCurve.Length - 1; i++)
        {
            int hi = BatteryVoltageCurve[i].Millivolts;     // higher-voltage end
            int lo = BatteryVoltageCurve[i + 1].Millivolts; // lower-voltage end
            if (millivolts >= lo && millivolts <= hi)
            {
                int span = hi - lo;
                double frac = (double)(millivolts - lo) / span;
                return BatteryVoltageCurve[i + 1].Percent +
                    (int)(frac * (BatteryVoltageCurve[i].Percent - BatteryVoltageCurve[i + 1].Percent) + 0.5);
            }
        }
        return 0;
    }
}