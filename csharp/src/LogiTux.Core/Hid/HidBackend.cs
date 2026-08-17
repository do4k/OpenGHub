namespace LogiTux.Core.Hid;

/// <summary>
/// Linux hidraw backend: device discovery via /sys/class/hidraw and I/O via
/// /dev/hidrawN, using only the BCL (no libhidapi, no P/Invoke beyond the
/// stream layer). Ported from internal/hid/hidraw_linux.go.
/// </summary>
public sealed class HidBackend
{
    private const string ClassPath = "/sys/class/hidraw";

    /// <summary>Enumerates all HID devices matching <paramref name="vendorId"/>;
    /// a <paramref name="productId"/> of 0 matches any product.</summary>
    public IReadOnlyList<DeviceInfo> Enumerate(ushort vendorId, ushort productId = 0)
    {
        var results = new List<DeviceInfo>();
        if (!Directory.Exists(ClassPath))
            return results;

        foreach (string entry in Directory.EnumerateDirectories(ClassPath))
        {
            string devDir = Path.Combine(entry, "device");
            if (!TryReadUevent(devDir, out HidUevent u))
                continue;
            if (u.VendorId != vendorId || (productId != 0 && u.ProductId != productId))
                continue;

            // Fall back to HID_UNIQ when there's no USB iSerialNumber (common on
            // receivers and on per-peripheral virtual hidraw nodes).
            string? serial = TryReadSerial(devDir) ?? u.Uniq;
            results.Add(new DeviceInfo(
                Path.Combine("/dev", Path.GetFileName(entry)),
                u.VendorId,
                u.ProductId,
                serial));
        }

        results.Sort(static (a, b) => string.CompareOrdinal(a.Path, b.Path));
        return results;
    }

    /// <summary>Opens the device for read/write. The returned stream is fully
    /// async-capable; blocking reads on char devices work naturally.</summary>
    public Stream Open(DeviceInfo info) =>
        new FileStream(info.Path, FileMode.Open, FileAccess.ReadWrite,
            FileShare.ReadWrite, 20, FileOptions.Asynchronous);

    private readonly record struct HidUevent(ushort VendorId, ushort ProductId, string? Uniq);

    /// <summary>Parses vendor/product IDs from HID_ID ("&lt;bus&gt;:&lt;vendor&gt;:&lt;product&gt;")
    /// and captures HID_UNIQ, mirroring readUevent in hidraw_linux.go.</summary>
    private static bool TryReadUevent(string devDir, out HidUevent u)
    {
        u = default;
        string path = Path.Combine(devDir, "uevent");
        if (!File.Exists(path))
            return false;

        ushort? vendor = null, product = null;
        string? uniq = null;
        foreach (string line in File.ReadLines(path))
        {
            if (line.StartsWith("HID_ID=", StringComparison.Ordinal))
            {
                string[] parts = line["HID_ID=".Length..].Split(':');
                if (parts.Length == 3 &&
                    ushort.TryParse(parts[1], System.Globalization.NumberStyles.HexNumber, null, out ushort v) &&
                    ushort.TryParse(parts[2], System.Globalization.NumberStyles.HexNumber, null, out ushort p))
                {
                    vendor = v;
                    product = p;
                }
            }
            else if (line.StartsWith("HID_UNIQ=", StringComparison.Ordinal))
            {
                uniq = line["HID_UNIQ=".Length..];
            }
        }

        if (vendor is null || product is null)
            return false;
        u = new HidUevent(vendor.Value, product.Value, uniq);
        return true;
    }

    /// <summary>Walks up the sysfs hierarchy from the HID device directory to the
    /// owning USB device directory (identified by an "idVendor" file) and reads
    /// its "serial" attribute. Stops at the first such directory whether or not
    /// serial exists there, mirroring readSerial in hidraw_linux.go.</summary>
    private static string? TryReadSerial(string devDir)
    {
        string? real = null;
        try
        {
            real = Directory.ResolveLinkTarget(devDir, true)?.FullName;
        }
        catch (IOException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }

        string dir = real ?? devDir;
        for (int i = 0; i < 8 && dir != "/" && dir != "."; i++)
        {
            if (File.Exists(Path.Combine(dir, "idVendor")))
            {
                try
                {
                    string? serial = File.ReadAllText(Path.Combine(dir, "serial"));
                    return string.IsNullOrWhiteSpace(serial) ? null : serial.Trim();
                }
                catch (IOException)
                {
                    return null;
                }
            }
            dir = Path.GetDirectoryName(dir) ?? "/";
        }
        return null;
    }
}