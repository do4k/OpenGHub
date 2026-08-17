using LogiTux.Core.Devices.Camera;
using LogiTux.Core.Devices.Litra;
using LogiTux.Core.Devices.Mouse;
using LogiTux.Core.Hid;
using LogiTux.Core.Hidpp;

const ushort vendorId = 0x046d;
bool demo = args.Contains("--demo");
bool verbose = args.Contains("--verbose");

string? only = null;
for (int i = 0; i < args.Length - 1; i++)
    if (args[i] == "--only") only = args[i + 1];
only ??= args.FirstOrDefault(a => a.StartsWith("--only=", StringComparison.Ordinal))?["--only=".Length..];

int? setDpi = null;
string? setDpiArg = args.FirstOrDefault(a => a.StartsWith("--set-dpi=", StringComparison.Ordinal));
if (setDpiArg is not null && int.TryParse(setDpiArg["--set-dpi=".Length..], out int v))
    setDpi = v;

var backend = new HidBackend();
var devices = backend.Enumerate(vendorId);
if (only is not null)
    devices = devices.Where(d => d.Path.Contains(only, StringComparison.Ordinal)).ToList();

Console.WriteLine($"Found {devices.Count} Logitech HID devices:");
foreach (var d in devices)
    Console.WriteLine($"  {d.Path,-14} v:{d.VendorId:x4} p:{d.ProductId:x4}  serial='{d.Serial}'");

foreach (var d in devices)
{
    if (d.ProductId is LitraLight.ProductGlow or LitraLight.ProductBeam)
    {
        await ProbeLitraAsync(backend, d, demo);
        continue;
    }

    if (GProMouse.IsMouseProduct(d.ProductId))
    {
        await ProbeMouseAsync(backend, d, setDpi);
        continue;
    }

    await ProbeHidppAsync(backend, d, verbose);
}

foreach (string node in V4l2Camera.EnumerateNodes())
    ProbeCamera(node);

Console.WriteLine("Done.");

static void ProbeCamera(string node)
{
    using V4l2Camera? camera = V4l2Camera.Open(node);
    if (camera is null)
    {
        Console.WriteLine($"  /dev/{node}: not a capture interface");
        return;
    }
    Console.WriteLine($"  /dev/{node}: {camera.Name} (serial '{camera.Serial}')");
    foreach (CameraControl c in camera.Controls)
    {
        try
        {
            Console.WriteLine($"    {c.Name,-14} range [{c.Min}, {c.Max}] step {c.Step} default {c.Default}  now {camera.GetControl(c.Name)}");
        }
        catch (IOException e)
        {
            Console.WriteLine($"    {c.Name,-14} range [{c.Min}, {c.Max}]  (read failed: {e.Message})");
        }
    }
}

static async Task ProbeMouseAsync(HidBackend backend, DeviceInfo d, int? setDpi)
{
    GProMouse? mouse;
    try
    {
        mouse = await GProMouse.OpenAsync(backend, d);
    }
    catch (UnauthorizedAccessException)
    {
        Console.WriteLine($"  {d.Path}: permission denied — run the PoC with sudo (see note above)");
        return;
    }
    catch (IOException e)
    {
        Console.WriteLine($"  {d.Path}: {e.Message}");
        return;
    }
    catch (InvalidOperationException e)
    {
        Console.WriteLine($"  {d.Path}: {e.Message}");
        return;
    }
    if (mouse is null)
        return; // duplicate receiver sibling; the canonical node printed it

    await using var _ = mouse;
    (int min, int max, int step) = mouse.DpiRange;
    Console.WriteLine($"  {d.Path}: {mouse.Name} (serial '{mouse.Serial}', product {mouse.ProductId:x4})");

    try
    {
        int dpi = await mouse.GetDpiAsync();
        Console.WriteLine($"    DPI: {dpi} (range {min}-{max}, step {step})");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    DPI: {e.Message}");
    }

    try
    {
        int rate = await mouse.GetReportRateAsync();
        Console.WriteLine($"    report rate: {rate} Hz (options: {string.Join(", ", mouse.ReportRateOptions)})");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    report rate: {e.Message}");
    }

    try
    {
        (int percent, bool charging) = await mouse.GetBatteryAsync();
        Console.WriteLine($"    battery: {percent}% {(charging ? "(charging)" : "(discharging)")}");
    }
    catch (InvalidOperationException)
    {
        Console.WriteLine("    battery: no supported battery feature on this unit");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    battery: {e.Message}");
    }

    if (setDpi is int target)
    {
        try
        {
            int original = await mouse.GetDpiAsync();
            await mouse.SetDpiAsync(target);
            int after = await mouse.GetDpiAsync();
            Console.WriteLine($"    set DPI {target}: read back {after} (restoring {original})");
            await mouse.SetDpiAsync(original);
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or InvalidOperationException)
        {
            Console.WriteLine($"    set DPI: {e.Message}");
        }
    }
    else
    {
        Console.WriteLine("    pass --set-dpi=N to write a DPI and read it back");
    }
}

static async Task ProbeHidppAsync(HidBackend backend, DeviceInfo d, bool verbose)
{
    Stream stream;
    try
    {
        stream = backend.Open(d);
    }
    catch (UnauthorizedAccessException)
    {
        Console.WriteLine($"  {d.Path}: permission denied — hidraw needs root or a udev rule (see LogiTux install/); run the PoC with sudo");
        return;
    }

    await using var conn = new HidppConn(stream);
    conn.SetTimeout(TimeSpan.FromMilliseconds(250));

    var live = new List<byte>();
    foreach (byte idx in new byte[] { HidppConn.DeviceIndexDirect, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06 })
    {
        try
        {
            (byte major, byte minor) = await HidppRoot.PingAsync(conn, idx);
            live.Add(idx);
            Console.WriteLine($"  {d.Path}: HID++ device at index 0x{idx:x2}, protocol {major}.{minor}");
        }
        catch (TimeoutException)
        {
            // index not present behind this node — expected while probing
        }
        catch (HidppErrorException e)
        {
            Console.WriteLine($"  {d.Path}: index 0x{idx:x2} rejected ping: {e.Message}");
        }
        catch (IOException e)
        {
            if (verbose) Console.WriteLine($"  {d.Path}: ping 0x{idx:x2}: {e.Message}");
        }
    }

    if (live.Count == 0)
    {
        Console.WriteLine($"  {d.Path}: no HID++ device responded on any index");
        return;
    }

    foreach (byte idx in live)
    {
        try
        {
            string name = await HidppName.GetDeviceNameAsync(conn, idx);
            byte? featureSet = await HidppRoot.GetFeatureIndexAsync(conn, idx, 0x0001);
            Console.WriteLine($"  {d.Path}: device 0x{idx:x2} is \"{name}\" (feature-set index 0x{featureSet:x2})");
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidOperationException or InvalidDataException)
        {
            if (verbose) Console.WriteLine($"  {d.Path}: 0x{idx:x2} name/feature-set lookup: {e.Message}");
        }

        try
        {
            (byte featureIdx, HidppBattery.BatteryKind kind) = await HidppBattery.ResolveBatteryFeatureAsync(conn, idx);
            if (kind != HidppBattery.BatteryKind.None)
            {
                (int percent, bool charging) = await HidppBattery.ReadBatteryAsync(conn, idx, featureIdx, kind);
                Console.WriteLine($"  {d.Path}: device 0x{idx:x2} battery: {percent}% {(charging ? "(charging)" : "(discharging)")} [via {kind}]");
            }
            else
            {
                Console.WriteLine($"  {d.Path}: device 0x{idx:x2} battery: no supported feature");
            }
        }
        catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
        {
            if (verbose) Console.WriteLine($"  {d.Path}: 0x{idx:x2} battery: {e.Message}");
        }
    }
}

static async Task ProbeLitraAsync(HidBackend backend, DeviceInfo d, bool demo)
{
    Stream stream;
    try
    {
        stream = backend.Open(d);
    }
    catch (UnauthorizedAccessException)
    {
        Console.WriteLine($"  {d.Path}: permission denied — run the PoC with sudo (see note above)");
        return;
    }

    await using var _ = stream;
    var light = new LitraLight(stream);
    Console.WriteLine($"  {d.Path}: {LitraLight.FriendlyName(d.ProductId)} (write-only protocol, no state read-back)");

    if (!demo)
    {
        Console.WriteLine("  pass --demo to power it on at 60% brightness / 3200 K so you can see writes working");
        return;
    }

    await light.SetPowerAsync(true);
    await light.SetBrightnessAsync(60);
    await light.SetTemperatureAsync(3200);
    Console.WriteLine("  sent: power on, brightness 60%, temperature 3200 K — the light should be on now");
}