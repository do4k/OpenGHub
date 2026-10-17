using OpenGHub.Core.Hid;
using OpenGHub.Core.Hidpp;

namespace OpenGHub.Core.Devices.Headset;

/// <summary>
/// PRO X Wireless Gaming Headset (product 0x0aba, its Lightspeed receiver).
/// HID++ device is addressed at the direct index. Ported from
/// internal/device/prox.
/// </summary>
public sealed partial class ProxHeadset : IAsyncDisposable
{
    public const ushort VendorId = 0x046d;
    public const ushort ProductId = 0x0aba;

    private const ushort FeatureSidetone = 0x8300;
    private const ushort FeatureEqualizer = 0x8310;

    private static readonly TimeSpan ProbeTimeout = TimeSpan.FromMilliseconds(300);
    private static readonly TimeSpan OperationTimeout = TimeSpan.FromSeconds(2);

    private readonly HidppConn _conn;
    private readonly byte _batteryFeatureIndex;
    private readonly HidppBattery.BatteryKind _batteryKind;
    private readonly string _name;

    // Optional features; a zero index means the unit doesn't expose them.
    private readonly byte _sidetoneFeatureIndex;
    private readonly byte _eqFeatureIndex;
    private readonly int[] _eqFrequenciesHz;
    private readonly int _eqMinDb;
    private readonly int _eqMaxDb;

    private ProxHeadset(
        HidppConn conn, byte batteryFeatureIndex, HidppBattery.BatteryKind kind, string name,
        byte sidetoneFeatureIndex, byte eqFeatureIndex, int[] eqFrequenciesHz, int eqMinDb, int eqMaxDb)
    {
        _conn = conn;
        _batteryFeatureIndex = batteryFeatureIndex;
        _batteryKind = kind;
        _name = name;
        _sidetoneFeatureIndex = sidetoneFeatureIndex;
        _eqFeatureIndex = eqFeatureIndex;
        _eqFrequenciesHz = eqFrequenciesHz;
        _eqMinDb = eqMinDb;
        _eqMaxDb = eqMaxDb;
    }

    public string Name => _name;

    /// <summary>
    /// Opens the headset's receiver node. Returns null when the device
    /// isn't reachable (headset off/asleep).
    /// </summary>
    public static async Task<ProxHeadset?> OpenAsync(HidBackend backend, DeviceInfo info)
    {
        var conn = new HidppConn(backend.Open(info));
        try
        {
            conn.SetTimeout(ProbeTimeout);
            await HidppRoot.PingAsync(conn, HidppConn.DeviceIndexDirect).ConfigureAwait(false);
            conn.SetTimeout(OperationTimeout);

            string name = "PRO X Wireless Headset";
            try
            {
                string selfName = await HidppName.GetDeviceNameAsync(conn, HidppConn.DeviceIndexDirect).ConfigureAwait(false);
                if (!string.IsNullOrEmpty(selfName))
                    name = selfName;
            }
            catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidOperationException or InvalidDataException)
            {
            }

            (byte idx, HidppBattery.BatteryKind kind) =
                await HidppBattery.ResolveBatteryFeatureAsync(conn, HidppConn.DeviceIndexDirect).ConfigureAwait(false);

            // Sidetone and EQ are optional: a unit without them shouldn't
            // block battery from working.
            byte sidetoneIdx = await HidppRoot.GetFeatureIndexAsync(conn, HidppConn.DeviceIndexDirect, FeatureSidetone).ConfigureAwait(false) ?? 0;

            byte eqIdx = 0;
            int[] eqFrequencies = Array.Empty<int>();
            int eqMinDb = 0;
            int eqMaxDb = 0;
            if (await HidppRoot.GetFeatureIndexAsync(conn, HidppConn.DeviceIndexDirect, FeatureEqualizer).ConfigureAwait(false) is byte eq)
            {
                try
                {
                    (eqFrequencies, eqMinDb, eqMaxDb) = await DiscoverEqualizerAsync(conn, eq).ConfigureAwait(false);
                    eqIdx = eq;
                }
                catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
                {
                }
            }

            return new ProxHeadset(conn, idx, kind, name,
                sidetoneIdx, eqIdx, eqFrequencies, eqMinDb, eqMaxDb);
        }
        catch
        {
            await conn.DisposeAsync().ConfigureAwait(false);
            return null;
        }
    }

    /// <summary>
    /// Throws InvalidOperationException when the device has no supported
    /// battery feature.
    /// </summary>
    public Task<(int Percent, bool Charging)> GetBatteryAsync() =>
        HidppBattery.ReadBatteryAsync(_conn, HidppConn.DeviceIndexDirect, _batteryFeatureIndex, _batteryKind);

    public ValueTask DisposeAsync() => _conn.DisposeAsync();
}
