using LogiTux.Core.Hid;
using LogiTux.Core.Hidpp;

namespace LogiTux.Core.Devices.Headset;

/// <summary>
/// PRO X Wireless Gaming Headset (product 0x0aba, its Lightspeed receiver).
/// HID++ device is addressed at the direct index. Ported from
/// internal/device/prox (battery part only for now — sidetone/EQ to come).
/// </summary>
public sealed class ProxHeadset : IAsyncDisposable
{
    public const ushort VendorId = 0x046d;
    public const ushort ProductId = 0x0aba;

    private static readonly TimeSpan ProbeTimeout = TimeSpan.FromMilliseconds(300);
    private static readonly TimeSpan OperationTimeout = TimeSpan.FromSeconds(2);

    private readonly HidppConn _conn;
    private readonly byte _batteryFeatureIndex;
    private readonly HidppBattery.BatteryKind _batteryKind;
    private readonly string _name;

    private ProxHeadset(HidppConn conn, byte batteryFeatureIndex, HidppBattery.BatteryKind kind, string name)
    {
        _conn = conn;
        _batteryFeatureIndex = batteryFeatureIndex;
        _batteryKind = kind;
        _name = name;
    }

    public string Name => _name;

    /// <summary>Opens the headset's receiver node. Returns null when the device
    /// isn't reachable (headset off/asleep).</summary>
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
            return new ProxHeadset(conn, idx, kind, name);
        }
        catch
        {
            await conn.DisposeAsync().ConfigureAwait(false);
            return null;
        }
    }

    /// <summary>Throws InvalidOperationException when the device has no supported
    /// battery feature.</summary>
    public Task<(int Percent, bool Charging)> GetBatteryAsync() =>
        HidppBattery.ReadBatteryAsync(_conn, HidppConn.DeviceIndexDirect, _batteryFeatureIndex, _batteryKind);

    public ValueTask DisposeAsync() => _conn.DisposeAsync();
}