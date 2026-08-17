using LogiTux.Core.Hid;
using LogiTux.Core.Hidpp;

namespace LogiTux.Core.Devices.Mouse;

/// <summary>
/// G Pro Wireless / PRO X Superlight mouse family over HID++ 2.0. Every
/// capability is a feature discovered at runtime via the Root feature.
/// Ported from internal/device/gpro (gpro.go, dpi.go, dpi_extended.go,
/// reportrate.go, reportrate_extended.go).
/// </summary>
public sealed class GProMouse : IAsyncDisposable
{
    public const ushort VendorId = 0x046d;

    // productReceiver is the Logitech Lightspeed USB receiver the mice are
    // typically paired to.
    public const ushort ProductReceiver = 0xc539;
    // Direct-connection product IDs: wired (Bluetooth), wired USB cable,
    // and the newer Superlight units.
    public const ushort ProductWired = 0x4079;
    public const ushort ProductWiredUsb = 0xc088;
    public const ushort ProductSuperlight = 0xc094;
    public const ushort ProductSuperlight2 = 0xc09b;

    private const ushort FeatureAdjustableDpi = 0x2201;
    private const ushort FeatureExtendedDpi = 0x2202;      // successor to 0x2201 on newer mice
    private const ushort FeatureReportRate = 0x8060;
    private const ushort FeatureExtendedReportRate = 0x8061; // successor to 0x8060 on newer mice

    private static readonly Dictionary<ushort, string> ProductNames = new()
    {
        [ProductWired] = "G Pro Wireless",
        [ProductWiredUsb] = "G Pro Wireless",
        [ProductSuperlight] = "PRO X Superlight",
        [ProductSuperlight2] = "PRO X Superlight 2",
    };

    // Probe timeout bounds each Ping while hunting for which hidraw interface and
    // device index the mouse is reachable on; operation timeout applies after.
    private static readonly TimeSpan ProbeTimeout = TimeSpan.FromMilliseconds(300);
    private static readonly TimeSpan OperationTimeout = TimeSpan.FromSeconds(2);

    private readonly HidppConn _conn;
    private readonly byte _deviceIndex;
    private readonly string _name;

    private readonly byte _dpiFeatureIndex;
    private readonly byte _extDpiFeatureIndex;
    private readonly ExtDpiInfo _extDpi;

    private readonly byte _batteryFeatureIndex;
    private readonly HidppBattery.BatteryKind _batteryKind;

    private readonly byte _reportRateFeatureIndex;
    private readonly byte _extRateFeatureIndex;
    private readonly int[] _reportRateOptions; // Hz, fastest (highest Hz) first

    private GProMouse(
        HidppConn conn, byte deviceIndex, string serial, string name, ushort productId,
        byte dpiIdx, byte extDpiIdx, ExtDpiInfo extDpi,
        byte batteryIdx, HidppBattery.BatteryKind batteryKind,
        byte rateIdx, byte extRateIdx, int[] rateOptions)
    {
        _conn = conn;
        _deviceIndex = deviceIndex;
        _name = name;
        Serial = serial;
        ProductId = productId;
        _dpiFeatureIndex = dpiIdx;
        _extDpiFeatureIndex = extDpiIdx;
        _extDpi = extDpi;
        _batteryFeatureIndex = batteryIdx;
        _batteryKind = batteryKind;
        _reportRateFeatureIndex = rateIdx;
        _extRateFeatureIndex = extRateIdx;
        _reportRateOptions = rateOptions;
    }

    public string Name => _name;
    public string Serial { get; }
    public ushort ProductId { get; }

    public static bool IsMouseProduct(ushort productId) => ProductNames.ContainsKey(productId) || productId == ProductReceiver;

    /// <summary>
    /// Opens a mouse for <paramref name="info"/>. Returns null when this node is a
    /// duplicate sibling of a receiver's canonical node (another call in the same
    /// pass does the real work, mirroring openViaReceiver's grouping).
    /// </summary>
    public static async Task<GProMouse?> OpenAsync(HidBackend backend, DeviceInfo info)
    {
        HidppConn? conn = null;
        try
        {
            byte deviceIndex;
            string serial = info.Serial ?? "";

            if (info.ProductId == ProductReceiver)
            {
                // The receiver exposes one hidraw node per USB interface, all with
                // the same product ID and serial; only one actually carries HID++
                // long reports, and the mouse's device index (1-6) is assigned at
                // pairing time — both must be discovered by probing.
                var siblings = backend.Enumerate(VendorId, ProductReceiver)
                    .Where(s => s.Serial == info.Serial)
                    .OrderBy(s => s.Path, StringComparer.Ordinal)
                    .ToList();
                if (siblings.Count == 0 || siblings[0].Path != info.Path)
                    return null; // duplicate sibling; the canonical node does the work

                byte? found = null;
                foreach (var candidate in siblings)
                {
                    conn = new HidppConn(backend.Open(candidate));
                    conn.SetTimeout(ProbeTimeout);
                    for (byte idx = 1; idx <= 6; idx++)
                    {
                        try
                        {
                            await HidppRoot.PingAsync(conn, idx).ConfigureAwait(false);
                            found = idx;
                            break;
                        }
                        catch (TimeoutException)
                        {
                        }
                        catch (HidppErrorException)
                        {
                        }
                        catch (IOException)
                        {
                            break; // write failed — this node is dead, try the next
                        }
                    }
                    if (found is not null)
                        break;
                    await conn.DisposeAsync().ConfigureAwait(false);
                    conn = null;
                }
                if (found is null)
                    throw new InvalidOperationException(
                        "gpro: no paired device responded on receiver (is the mouse asleep? try moving it)");
                deviceIndex = found.Value;
                serial = ResolveMouseSerial(backend, serial);
            }
            else
            {
                // Directly connected (wired/Bluetooth), addressed at device index 0xFF.
                conn = new HidppConn(backend.Open(info));
                conn.SetTimeout(ProbeTimeout);
                try
                {
                    await HidppRoot.PingAsync(conn, HidppConn.DeviceIndexDirect).ConfigureAwait(false);
                }
                catch (TimeoutException)
                {
                    throw new InvalidOperationException("gpro: ping direct-connected mouse: no response");
                }
                deviceIndex = HidppConn.DeviceIndexDirect;
            }

            conn!.SetTimeout(OperationTimeout);
            return await BuildAsync(conn, deviceIndex, serial, info.ProductId).ConfigureAwait(false);
        }
        catch
        {
            if (conn is not null)
                await conn.DisposeAsync().ConfigureAwait(false);
            throw;
        }
    }

    /// <summary>
    /// Picks the best available identifier for a receiver-paired mouse:
    /// a virtual per-peripheral hidraw node (product 0x4079) often carries a real
    /// hardware ID even when the receiver has no USB serial descriptor.
    /// </summary>
    private static string ResolveMouseSerial(HidBackend backend, string receiverSerial)
    {
        foreach (var wi in backend.Enumerate(VendorId, ProductWired))
            if (!string.IsNullOrEmpty(wi.Serial))
                return wi.Serial!;
        return receiverSerial;
    }

    /// <summary>
    /// Resolves every feature the plugin uses, mirroring buildMouse:
    /// DPI and report rate are expected (classic or extended), battery optional.
    /// </summary>
    private static async Task<GProMouse> BuildAsync(HidppConn conn, byte deviceIndex, string serial, ushort productId)
    {
        // DPI: classic 0x2201 on the G Pro Wireless generation, or its successor
        // 0x2202 on newer mice, which implement 0x2202 *instead of* 0x2201.
        byte? classicDpiIdx = await HidppRoot.GetFeatureIndexAsync(conn, deviceIndex, FeatureAdjustableDpi).ConfigureAwait(false);
        byte dpiIdx = classicDpiIdx ?? 0;
        byte extDpiIdx = 0;
        ExtDpiInfo extDpi = default;
        if (classicDpiIdx is null)
        {
            extDpiIdx = await RequireFeatureAsync(conn, deviceIndex, FeatureExtendedDpi, "Extended Adjustable DPI").ConfigureAwait(false);
            extDpi = await DiscoverExtendedDpiAsync(conn, deviceIndex, extDpiIdx).ConfigureAwait(false);
        }

        (byte idx, HidppBattery.BatteryKind kind) = await HidppBattery.ResolveBatteryFeatureAsync(conn, deviceIndex).ConfigureAwait(false);

        // Report rate: same classic-or-extended split; 0x8061 covers the
        // 2000-8000 Hz rates 0x8060's millisecond-interval encoding can't express.
        byte? classicRateIdx = await HidppRoot.GetFeatureIndexAsync(conn, deviceIndex, FeatureReportRate).ConfigureAwait(false);
        byte rateIdx = classicRateIdx ?? 0;
        byte extRateIdx = 0;
        int[] rateOptions;
        if (classicRateIdx is not null)
        {
            rateOptions = await GetReportRateOptionsAsync(conn, deviceIndex, rateIdx).ConfigureAwait(false);
        }
        else
        {
            extRateIdx = await RequireFeatureAsync(conn, deviceIndex, FeatureExtendedReportRate, "Extended Report Rate").ConfigureAwait(false);
            rateOptions = await GetExtendedReportRateOptionsAsync(conn, deviceIndex, extRateIdx).ConfigureAwait(false);
        }

        // Prefer the device's self-reported marketing name.
        string name = ProductNames.GetValueOrDefault(productId) ?? $"G Pro ({productId:x4})";
        try
        {
            string selfName = await HidppName.GetDeviceNameAsync(conn, deviceIndex).ConfigureAwait(false);
            if (!string.IsNullOrEmpty(selfName))
                name = selfName;
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidOperationException or InvalidDataException)
        {
        }

        return new GProMouse(conn, deviceIndex, serial, name, productId,
            dpiIdx, extDpiIdx, extDpi,
            idx, kind,
            rateIdx, extRateIdx, rateOptions);
    }

    private static async Task<byte> RequireFeatureAsync(HidppConn conn, byte deviceIndex, ushort featureId, string name)
    {
        byte? idx = await HidppRoot.GetFeatureIndexAsync(conn, deviceIndex, featureId).ConfigureAwait(false)
            ?? throw new InvalidOperationException($"gpro: device does not support the {name} feature (0x{featureId:x4})");
        return idx.Value;
    }

    // ---- DPI ----

    private const byte DpiSensorIndex = 0x00;
    private const byte DpiFuncGetDpi = 0x20;
    private const byte DpiFuncSetDpi = 0x30;

    // The G Pro Wireless has a single HERO 25K sensor: 100-25600 DPI in steps of 50.
    private const int ClassicDpiMin = 100;
    private const int ClassicDpiMax = 25600;
    private const int ClassicDpiStep = 50;

    public (int Min, int Max, int Step) DpiRange =>
        _extDpiFeatureIndex != 0 ? (_extDpi.Min, _extDpi.Max, _extDpi.Step) : (ClassicDpiMin, ClassicDpiMax, ClassicDpiStep);

    /// <summary>
    /// Reads the sensor's current DPI live from the device, falling back
    /// to its default DPI if no override is set.
    /// </summary>
    public Task<int> GetDpiAsync() =>
        _extDpiFeatureIndex != 0 ? GetExtendedDpiAsync() : GetClassicDpiAsync();

    private async Task<int> GetClassicDpiAsync()
    {
        byte[] resp = await _conn.CallAsync(_deviceIndex, _dpiFeatureIndex, DpiFuncGetDpi, [DpiSensorIndex]).ConfigureAwait(false);
        if (resp.Length < 5)
            throw new InvalidDataException("gpro: short DPI response");
        int current = (resp[1] << 8) | resp[2];
        if (current == 0)
            current = (resp[3] << 8) | resp[4];
        return current;
    }

    /// <summary>
    /// Sets DPI, clamped to the sensor's range and rounded to the nearest
    /// supported step.
    /// </summary>
    public async Task SetDpiAsync(int dpi)
    {
        if (_extDpiFeatureIndex != 0)
        {
            await SetExtendedDpiAsync(dpi).ConfigureAwait(false);
            return;
        }
        dpi = ClampToStep(dpi, ClassicDpiMin, ClassicDpiMax, ClassicDpiStep);
        byte[] payload = [DpiSensorIndex, (byte)(dpi >> 8), (byte)dpi];
        await _conn.CallAsync(_deviceIndex, _dpiFeatureIndex, DpiFuncSetDpi, payload).ConfigureAwait(false);
    }

    // ---- Extended DPI (0x2202) ----

    private const byte ExtDpiFuncGetSensorCapabilities = 0x10;
    private const byte ExtDpiFuncGetDpiRanges = 0x20;
    private const byte ExtDpiFuncGetDpiParameters = 0x50;
    private const byte ExtDpiFuncSetDpiParameters = 0x60;

    private const byte ExtDpiCapHasY = 1 << 0; // sensor supports independent Y-axis DPI
    private const ushort ExtDpiRangeHyphen = 0b111 << 13;
    private const int ExtDpiMaxRangePages = 16;
    private const byte ExtDpiDirectionX = 0;

    private readonly record struct ExtDpiInfo(bool HasY, int Min, int Max, int Step);

    /// <summary>
    /// Reads the sensor's capabilities and supported DPI range: a stream
    /// of 16-bit words — fixed values and step-tagged ranges, 0x0000-terminated,
    /// possibly split across pages — collapsed to min/max/step.
    /// </summary>
    private static async Task<ExtDpiInfo> DiscoverExtendedDpiAsync(HidppConn conn, byte deviceIndex, byte featureIndex)
    {
        byte[] caps = await conn.CallAsync(deviceIndex, featureIndex, ExtDpiFuncGetSensorCapabilities, [DpiSensorIndex]).ConfigureAwait(false);
        if (caps.Length < 3)
            throw new InvalidDataException("gpro: short extended DPI capabilities response");
        bool hasY = (caps[2] & ExtDpiCapHasY) != 0;

        var stream = new List<byte>();
        for (int page = 0; page < ExtDpiMaxRangePages; page++)
        {
            byte[] resp = await conn.CallAsync(deviceIndex, featureIndex, ExtDpiFuncGetDpiRanges,
                [DpiSensorIndex, ExtDpiDirectionX, (byte)page]).ConfigureAwait(false);
            // The response echoes the addressing before the page body; verify it so
            // a mismatched page can't corrupt the accumulated stream.
            if (resp.Length < 4 || resp[0] != DpiSensorIndex || resp[1] != ExtDpiDirectionX || resp[2] != page)
                throw new InvalidDataException("gpro: unexpected extended DPI ranges response");
            stream.AddRange(resp[3..]);

            (int min, int max, int step, bool done) = ParseExtendedDpiRanges(stream);
            if (done)
                return new ExtDpiInfo(hasY, min, max, step);
        }
        throw new InvalidDataException("gpro: extended DPI range list never terminated");
    }

    /// <summary>
    /// Decodes the supported-DPI word stream; done=false (no error) while
    /// the 0x0000 terminator hasn't arrived yet, telling the caller to fetch
    /// another page.
    /// </summary>
    internal static (int Min, int Max, int Step, bool Done) ParseExtendedDpiRanges(IReadOnlyList<byte> stream)
    {
        var words = new List<ushort>();
        bool terminated = false;
        for (int off = 0; off + 2 <= stream.Count; off += 2)
        {
            ushort w = (ushort)((stream[off] << 8) | stream[off + 1]);
            if (w == 0)
            {
                terminated = true;
                break;
            }
            words.Add(w);
        }
        if (!terminated)
            return (0, 0, 0, false);

        int min = 0, max = 0, step = 0;
        int pending = 0;
        bool hasPending = false;
        for (int i = 0; i < words.Count; i++)
        {
            ushort w = words[i];
            if (w >= ExtDpiRangeHyphen)
            {
                int s = w & ~ExtDpiRangeHyphen;
                if (!hasPending || s == 0 || i + 1 >= words.Count || words[i + 1] >= ExtDpiRangeHyphen)
                    throw new InvalidDataException("gpro: malformed extended DPI range stream");
                int to = words[i + 1];
                if (to < pending)
                    throw new InvalidDataException("gpro: malformed extended DPI range stream");
                if (min == 0 || to < min) min = to;
                if (to > max) max = to;
                if (step == 0 || s < step) step = s;
                pending = to;
                i++;
            }
            else
            {
                if (min == 0 || w < min) min = w;
                if (w > max) max = w;
                pending = w;
                hasPending = true;
            }
        }

        if (max == 0)
            throw new InvalidDataException("gpro: empty extended DPI range list");
        if (step == 0)
            step = 50; // fixed values only; every known gaming sensor uses 50-DPI granularity
        return (min, max, step, true);
    }

    /// <summary>
    /// Reads the current X-axis DPI, falling back to default when no
    /// override is set. Response: [sensor, dpiX:2, defaultX:2, dpiY:2, defaultY:2, lod].
    /// </summary>
    private async Task<int> GetExtendedDpiAsync()
    {
        byte[] resp = await _conn.CallAsync(_deviceIndex, _extDpiFeatureIndex, ExtDpiFuncGetDpiParameters, [DpiSensorIndex]).ConfigureAwait(false);
        if (resp.Length < 10)
            throw new InvalidDataException("gpro: short extended DPI response");
        int current = (resp[1] << 8) | resp[2];
        if (current == 0)
            current = (resp[3] << 8) | resp[4];
        return current;
    }

    /// <summary>
    /// Writes DPI to both axes when the sensor has a Y axis (the GUI
    /// exposes a single linked value). The set call writes DPI and lift-off
    /// distance together, so current LOD is read first and preserved.
    /// </summary>
    private async Task SetExtendedDpiAsync(int dpi)
    {
        dpi = ClampToStep(dpi, _extDpi.Min, _extDpi.Max, _extDpi.Step);

        byte lod = 0;
        try
        {
            byte[] resp = await _conn.CallAsync(_deviceIndex, _extDpiFeatureIndex, ExtDpiFuncGetDpiParameters, [DpiSensorIndex]).ConfigureAwait(false);
            if (resp.Length >= 10)
                lod = resp[9];
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
        {
        }

        int y = _extDpi.HasY ? dpi : 0;
        byte[] payload =
        [
            DpiSensorIndex,
            (byte)(dpi >> 8), (byte)dpi,
            (byte)(y >> 8), (byte)y,
            lod,
        ];
        await _conn.CallAsync(_deviceIndex, _extDpiFeatureIndex, ExtDpiFuncSetDpiParameters, payload).ConfigureAwait(false);
    }

    // ---- Report rate ----

    private const byte ReportRateFuncGetList = 0x00;
    private const byte ReportRateFuncGet = 0x10;
    private const byte ReportRateFuncSet = 0x20;

    /// <summary>
    /// Queries which report intervals (1-8 ms, bitmask over bit i =
    /// interval i+1) the device supports, returned as Hz ordered fastest first.
    /// </summary>
    private static async Task<int[]> GetReportRateOptionsAsync(HidppConn conn, byte deviceIndex, byte featureIndex)
    {
        byte[] resp = await conn.CallAsync(deviceIndex, featureIndex, ReportRateFuncGetList, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 1)
            throw new InvalidDataException("gpro: short report rate list response");
        byte bitmask = resp[0];
        var options = new List<int>();
        for (int i = 0; i < 8; i++)
        {
            if ((bitmask & (1 << i)) == 0)
                continue;
            int intervalMs = i + 1;
            options.Add(1000 / intervalMs);
        }
        if (options.Count == 0)
            throw new InvalidDataException("gpro: device reports no supported report rates");
        return [.. options];
    }

    public IReadOnlyList<int> ReportRateOptions => _reportRateOptions;

    public Task<int> GetReportRateAsync() =>
        _extRateFeatureIndex != 0 ? GetExtendedReportRateAsync() : GetClassicReportRateAsync();

    private async Task<int> GetClassicReportRateAsync()
    {
        byte[] resp = await _conn.CallAsync(_deviceIndex, _reportRateFeatureIndex, ReportRateFuncGet, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 1 || resp[0] == 0)
            throw new InvalidDataException("gpro: invalid report rate response");
        return 1000 / resp[0];
    }

    /// <summary>
    /// Sets report rate, snapping hz to whichever supported option is closest.
    /// </summary>
    public async Task SetReportRateAsync(int hz)
    {
        if (_extRateFeatureIndex != 0)
        {
            await SetExtendedReportRateAsync(hz).ConfigureAwait(false);
            return;
        }
        int intervalMs = 1000 / ClosestOption(hz, _reportRateOptions);
        await _conn.CallAsync(_deviceIndex, _reportRateFeatureIndex, ReportRateFuncSet, [(byte)intervalMs]).ConfigureAwait(false);
    }

    // ---- Extended report rate (0x8061) ----

    private const byte ExtRateFuncGetRateList = 0x10;
    private const byte ExtRateFuncGetRate = 0x20;
    private const byte ExtRateFuncSetRate = 0x30;

    private const byte ExtRateConnWired = 0;
    private const byte ExtRateConnGamingWireless = 1; // Lightspeed

    /// <summary>
    /// Maps the feature's rate index (0-6) to Hz.
    /// </summary>
    private static readonly int[] ExtRateHz = [125, 250, 500, 1000, 2000, 4000, 8000];

    /// <summary>
    /// Reads the bitmask of rates supported on the current connection
    /// (bit i = ExtRateHz[i]) as Hz, fastest first.
    /// </summary>
    private static async Task<int[]> GetExtendedReportRateOptionsAsync(HidppConn conn, byte deviceIndex, byte featureIndex)
    {
        byte[] resp = await conn.CallAsync(deviceIndex, featureIndex, ExtRateFuncGetRateList, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 2)
            throw new InvalidDataException("gpro: short extended report rate list response");
        int mask = (resp[0] << 8) | resp[1];

        var options = new List<int>();
        for (int i = ExtRateHz.Length - 1; i >= 0; i--)
            if ((mask & (1 << i)) != 0)
                options.Add(ExtRateHz[i]);
        if (options.Count == 0)
            throw new InvalidDataException("gpro: device reports no supported report rates");
        return [.. options];
    }

    /// <summary>
    /// Reads the active rate. The read takes a connection type; the likely
    /// one is tried first based on how the mouse is addressed (direct = wired,
    /// receiver-paired = Lightspeed), falling back to the other.
    /// </summary>
    private async Task<int> GetExtendedReportRateAsync()
    {
        byte first = ExtRateConnGamingWireless, second = ExtRateConnWired;
        if (_deviceIndex == HidppConn.DeviceIndexDirect)
            (first, second) = (second, first);

        Exception? lastErr = null;
        foreach (byte connType in new[] { first, second })
        {
            try
            {
                byte[] resp = await _conn.CallAsync(_deviceIndex, _extRateFeatureIndex, ExtRateFuncGetRate, [connType]).ConfigureAwait(false);
                if (resp.Length < 1 || resp[0] >= ExtRateHz.Length)
                {
                    lastErr = new InvalidDataException("gpro: invalid extended report rate response");
                    continue;
                }
                return ExtRateHz[resp[0]];
            }
            catch (Exception e) when (e is TimeoutException or HidppErrorException)
            {
                lastErr = e;
            }
        }
        throw new InvalidOperationException("gpro: get extended report rate", lastErr);
    }

    /// <summary>
    /// Snaps hz to the closest supported option and writes its rate index.
    /// </summary>
    private async Task SetExtendedReportRateAsync(int hz)
    {
        hz = ClosestOption(hz, _reportRateOptions);
        int index = Array.IndexOf(ExtRateHz, hz);
        if (index < 0)
            throw new InvalidOperationException($"gpro: unsupported report rate {hz} Hz");
        await _conn.CallAsync(_deviceIndex, _extRateFeatureIndex, ExtRateFuncSetRate, [(byte)index]).ConfigureAwait(false);
    }

    // ---- Battery ----

    /// <summary>
    /// Reads battery percent/charging via whichever feature the device
    /// resolved to. Throws InvalidOperationException when the device has no
    /// supported battery feature.
    /// </summary>
    public Task<(int Percent, bool Charging)> GetBatteryAsync() =>
        HidppBattery.ReadBatteryAsync(_conn, _deviceIndex, _batteryFeatureIndex, _batteryKind);

    // ---- Shared helpers ----

    internal static int ClampToStep(int v, int min, int max, int step)
    {
        if (v < min) v = min;
        if (v > max) v = max;
        return min + (v - min + step / 2) / step * step;
    }

    internal static int ClosestOption(int hz, IReadOnlyList<int> options)
    {
        int best = options[0];
        foreach (int o in options)
            if (Math.Abs(o - hz) < Math.Abs(best - hz))
                best = o;
        return best;
    }

    public async ValueTask DisposeAsync() => await _conn.DisposeAsync().ConfigureAwait(false);
}
