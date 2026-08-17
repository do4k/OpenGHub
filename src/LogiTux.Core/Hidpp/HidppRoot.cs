namespace LogiTux.Core.Hidpp;

/// <summary>
/// Root feature (0x0000) helpers. The Root feature is always at feature
/// index 0, so these calls never need a prior lookup. Ported from root.go.
/// </summary>
public static class HidppRoot
{
    private const byte RootFuncGetFeature = 0x00;
    private const byte RootFuncGetProtocolVersion = 0x10;

    /// <summary>
    /// Asks the Root feature which feature index implements
    /// <paramref name="featureId"/> on <paramref name="deviceIndex"/>. Returns null
    /// if the device doesn't support the feature at all (a legitimate, expected
    /// outcome — not an error).
    /// </summary>
    public static async Task<byte?> GetFeatureIndexAsync(HidppConn conn, byte deviceIndex, ushort featureId)
    {
        var resp = await conn.CallAsync(deviceIndex, HidppConn.RootFeatureIndex,
            RootFuncGetFeature, [(byte)(featureId >> 8), (byte)featureId]).ConfigureAwait(false);
        return resp.Length >= 1 && resp[0] != 0 ? resp[0] : null;
    }

    /// <summary>
    /// Confirms <paramref name="deviceIndex"/> is present and speaks
    /// HID++ 2.0, returning its protocol version. Also the standard way to probe
    /// which device indices behind a receiver currently have a paired device.
    /// </summary>
    public static async Task<(byte Major, byte Minor)> PingAsync(HidppConn conn, byte deviceIndex)
    {
        var resp = await conn.CallAsync(deviceIndex, HidppConn.RootFeatureIndex,
            RootFuncGetProtocolVersion, [0x00, 0x00, 0xaa]).ConfigureAwait(false);
        if (resp.Length < 2)
            throw new InvalidDataException($"hidpp: short ping response from device 0x{deviceIndex:x2}");
        return (resp[0], resp[1]);
    }
}
