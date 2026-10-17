using OpenGHub.Core.Hidpp;

namespace OpenGHub.Core.Devices.Headset;

/// <summary>
/// EQUALIZER (0x8310) support for the PRO X Wireless headset. Ported from
/// internal/device/prox/eq.go.
/// </summary>
public sealed partial class ProxHeadset
{
    // EQUALIZER (0x8310) function numbers.
    private const byte EqFuncGetInfo = 0x00;
    private const byte EqFuncGetBandFrequencies = 0x10;
    private const byte EqFuncGetLevels = 0x20;
    private const byte EqFuncSetLevels = 0x30;
    private const byte EqFuncGetNoiseReduction = 0x40;
    private const byte EqFuncSetNoiseReduction = 0x50;

    // The get/set-levels calls take a leading selector byte. Get uses a
    // gain *location* (0 reads the custom EQ stored in EEPROM, 1 the active
    // EQ in RAM); set uses a gain *persistence* (0 RAM only, 1 RAM + EEPROM,
    // 2 EEPROM only). Values verified against real hardware.
    private const byte EqGainLocationEeprom = 0x00;
    private const byte EqPersistNonVolatileWrite = 0x02;

    /// <summary>
    /// Reads the device's fixed band layout (frequencies) and dB range.
    /// Band levels themselves are read live by <see cref="GetEqLevelsAsync"/>,
    /// not cached here, since they can change on the device side (e.g. a
    /// physical control, or another app) between calls. Ported from
    /// internal/device/prox/eq.go.
    /// </summary>
    internal static async Task<(int[] Frequencies, int MinDb, int MaxDb)> DiscoverEqualizerAsync(HidppConn conn, byte featureIndex)
    {
        var info = await conn.CallAsync(HidppConn.DeviceIndexDirect, featureIndex, EqFuncGetInfo, Array.Empty<byte>()).ConfigureAwait(false);
        if (info.Length < 5)
            throw new InvalidDataException("prox: short equalizer info response");

        int count = info[0];
        int dbRange = (sbyte)info[1];
        int minDb = (sbyte)info[3];
        int maxDb = (sbyte)info[4];
        if (minDb == 0)
            minDb = -dbRange;
        if (maxDb == 0)
            maxDb = dbRange;

        var frequencies = new int[count];
        for (var group = 0; group * 7 < count; group++)
        {
            var resp = await conn.CallAsync(HidppConn.DeviceIndexDirect, featureIndex, EqFuncGetBandFrequencies, [(byte)(group * 7)]).ConfigureAwait(false);
            for (var b = 0; b < 7; b++)
            {
                var bandIndex = group * 7 + b;
                if (bandIndex >= count)
                    break;
                var offset = 1 + 2 * b;
                if (offset + 2 > resp.Length)
                    throw new InvalidDataException("prox: short equalizer band frequency response");
                frequencies[bandIndex] = (resp[offset] << 8) | resp[offset + 1];
            }
        }

        return (frequencies, minDb, maxDb);
    }

    /// <summary>
    /// Whether the device exposes the equalizer feature. Ported from
    /// internal/device/prox/eq.go.
    /// </summary>
    public bool HasEqualizer => _eqFeatureIndex != 0;

    /// <summary>
    /// The device's fixed band frequencies, in Hz, one per band in order.
    /// Ported from internal/device/prox/eq.go.
    /// </summary>
    public IReadOnlyList<int> EqFrequencies => _eqFrequenciesHz;

    /// <summary>
    /// The device's dB range for equalizer band levels. Ported from
    /// internal/device/prox/eq.go.
    /// </summary>
    public (int Min, int Max) EqRange => (_eqMinDb, _eqMaxDb);

    /// <summary>
    /// Reads the headset's current equalizer band levels, in dB, from the
    /// custom EQ stored in EEPROM. Ported from internal/device/prox/eq.go.
    /// </summary>
    public async Task<IReadOnlyList<int>> GetEqLevelsAsync()
    {
        EnsureEqualizerFeature();
        var resp = await _conn.CallAsync(HidppConn.DeviceIndexDirect, _eqFeatureIndex, EqFuncGetLevels, [EqGainLocationEeprom]).ConfigureAwait(false);
        var count = _eqFrequenciesHz.Length;
        if (resp.Length < count)
            throw new InvalidDataException("prox: short equalizer levels response");

        var levels = new int[count];
        for (var i = 0; i < count; i++)
            levels[i] = (sbyte)resp[i];
        return levels;
    }

    /// <summary>
    /// Sets the equalizer band levels, in dB, persisting them to EEPROM.
    /// Each level is clamped to the device's dB range. Ported from
    /// internal/device/prox/eq.go.
    /// </summary>
    public async Task SetEqLevelsAsync(IReadOnlyList<int> levelsDb)
    {
        EnsureEqualizerFeature();
        if (levelsDb.Count != _eqFrequenciesHz.Length)
            throw new InvalidDataException($"prox: expected {_eqFrequenciesHz.Length} equalizer levels, got {levelsDb.Count}");

        var parameters = new byte[1 + levelsDb.Count];
        parameters[0] = EqPersistNonVolatileWrite;
        for (var i = 0; i < levelsDb.Count; i++)
            parameters[1 + i] = (byte)Math.Clamp(levelsDb[i], _eqMinDb, _eqMaxDb);
        await _conn.CallAsync(HidppConn.DeviceIndexDirect, _eqFeatureIndex, EqFuncSetLevels, parameters).ConfigureAwait(false);
    }

    /// <summary>
    /// Whether the headset's hardware microphone noise reduction is enabled.
    /// Functions 4/5 of the same EQUALIZER feature; units whose firmware
    /// predates them answer with a HID++ error, which callers treat as
    /// "not supported". Ported from internal/device/prox/eq.go.
    /// </summary>
    public async Task<bool> GetNoiseReductionAsync()
    {
        EnsureEqualizerFeature();
        var resp = await _conn.CallAsync(HidppConn.DeviceIndexDirect, _eqFeatureIndex, EqFuncGetNoiseReduction, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 1)
            throw new InvalidDataException("prox: short noise reduction response");
        return resp[0] != 0;
    }

    /// <summary>
    /// Enables or disables the headset's hardware microphone noise
    /// reduction. Ported from internal/device/prox/eq.go.
    /// </summary>
    public async Task SetNoiseReductionAsync(bool enabled)
    {
        EnsureEqualizerFeature();
        var value = (byte)(enabled ? 1 : 0);
        await _conn.CallAsync(HidppConn.DeviceIndexDirect, _eqFeatureIndex, EqFuncSetNoiseReduction, [value]).ConfigureAwait(false);
    }

    /// <summary>
    /// Throws when the device has no equalizer feature.
    /// </summary>
    private void EnsureEqualizerFeature()
    {
        if (_eqFeatureIndex == 0)
            throw new InvalidOperationException("prox: device has no equalizer feature");
    }
}