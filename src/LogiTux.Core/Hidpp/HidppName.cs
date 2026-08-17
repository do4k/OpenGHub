using System.Runtime.InteropServices;
using System.Text;

namespace LogiTux.Core.Hidpp;

/// <summary>
/// DEVICE_TYPE_AND_NAME (0x0005): the device's own marketing name,
/// readable in fixed-size chunks. Ported from name.go.
/// </summary>
public static class HidppName
{
    public const ushort FeatureDeviceTypeAndName = 0x0005;

    private const byte NameFuncGetCount = 0x00; // -> [nameLength]
    private const byte NameFuncGetName = 0x10;  // [charIndex] -> up to 16 name bytes

    /// <summary>
    /// Guards against a device reporting an implausible name length and
    /// this issuing dozens of calls for it.
    /// </summary>
    private const int MaxDeviceNameLen = 64;

    /// <summary>
    /// Reads the device's marketing name (e.g. "PRO X SUPERLIGHT"),
    /// paging through the name until the reported length is collected. Throws if
    /// the device doesn't expose the feature; callers treat the name as a
    /// nice-to-have and fall back to a product-ID-derived name.
    /// </summary>
    public static async Task<string> GetDeviceNameAsync(HidppConn conn, byte deviceIndex)
    {
        byte? idx = await HidppRoot.GetFeatureIndexAsync(conn, deviceIndex, FeatureDeviceTypeAndName).ConfigureAwait(false)
            ?? throw new InvalidOperationException("hidpp: device has no DEVICE_TYPE_AND_NAME feature");

        var resp = await conn.CallAsync(deviceIndex, idx.Value, NameFuncGetCount, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 1)
            throw new InvalidDataException("hidpp: short device name length response");
        int count = resp[0];
        if (count == 0 || count > MaxDeviceNameLen)
            throw new InvalidDataException($"hidpp: implausible device name length {count}");

        var name = new List<byte>(count);
        while (name.Count < count)
        {
            resp = await conn.CallAsync(deviceIndex, idx.Value, NameFuncGetName, [(byte)name.Count]).ConfigureAwait(false);
            if (resp.Length == 0)
                throw new InvalidDataException("hidpp: empty device name chunk");
            int remaining = count - name.Count;
            name.AddRange(resp.AsSpan(0, Math.Min(resp.Length, remaining)).ToArray());
        }

        // Some firmwares pad the advertised length with NULs; trim them.
        while (name.Count > 0 && name[^1] == 0)
            name.RemoveAt(name.Count - 1);
        if (name.Count == 0)
            throw new InvalidDataException("hidpp: device reported an empty name");
        return Encoding.UTF8.GetString(CollectionsMarshal.AsSpan(name));
    }
}
