using OpenGHub.Core.Devices.Camera;
using OpenGHub.Core.Devices.Headset;
using OpenGHub.Core.Devices.Litra;
using OpenGHub.Core.Devices.Mouse;
using OpenGHub.Core.Hid;
using OpenGHub.Core.Hidpp;

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

byte[]? setRgb = null;
string? setRgbArg = args.FirstOrDefault(a => a.StartsWith("--set-rgb=", StringComparison.Ordinal));
if (setRgbArg is not null && setRgbArg.Length == "--set-rgb=".Length + 6
    && byte.TryParse(setRgbArg.AsSpan("--set-rgb=".Length, 2), System.Globalization.NumberStyles.HexNumber, null, out byte r)
    && byte.TryParse(setRgbArg.AsSpan("--set-rgb=".Length + 2, 2), System.Globalization.NumberStyles.HexNumber, null, out byte g)
    && byte.TryParse(setRgbArg.AsSpan("--set-rgb=".Length + 4, 2), System.Globalization.NumberStyles.HexNumber, null, out byte b))
{
    setRgb = [r, g, b];
}

int? setSidetone = null;
string? setSidetoneArg = args.FirstOrDefault(a => a.StartsWith("--set-sidetone=", StringComparison.Ordinal));
if (setSidetoneArg is not null && int.TryParse(setSidetoneArg["--set-sidetone=".Length..], out int s))
    setSidetone = s;

string? setEqArg = args.FirstOrDefault(a => a.StartsWith("--set-eq=", StringComparison.Ordinal));

bool preview = args.Contains("--preview");
bool previewYuyv = args.Contains("--preview-yuyv");

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

    if (d.ProductId == ProxHeadset.ProductId)
    {
        await ProbeHeadsetAsync(backend, d, setSidetone, setEqArg);
        continue;
    }

    if (GProMouse.IsMouseProduct(d.ProductId))
    {
        await ProbeMouseAsync(backend, d, setDpi, setRgb);
        continue;
    }

    await ProbeHidppAsync(backend, d, verbose);
}

foreach (string node in V4l2Camera.EnumerateNodes())
    ProbeCamera(node, preview || previewYuyv, previewYuyv);

Console.WriteLine("Done.");

static void ProbeCamera(string node, bool preview, bool forceYuyv = false)
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

    if (!preview)
        return;
    using WebcamPreview p = camera.CreatePreview();
    try
    {
        if (forceYuyv)
        {
            p.Start(OpenGHub.Core.V4l2.V4l2Device.PixFmtYUYV);
            Console.WriteLine("    preview: forcing YUYV");
        }
        else
        {
            p.Start();
        }
    }
    catch (Exception e)
    {
        Console.WriteLine($"    preview: failed to start: {e.Message}");
        return;
    }
    var stopwatch = System.Diagnostics.Stopwatch.StartNew();
    ulong lastSeq = 0;
    int frames = 0;
    int width = 0;
    int height = 0;
    bool jpeg = false;
    while (stopwatch.Elapsed < TimeSpan.FromSeconds(2))
    {
        if (p.TryGetFrame(out PreviewFrame frame) && frame.Sequence != lastSeq)
        {
            lastSeq = frame.Sequence;
            frames++;
            width = frame.Width;
            height = frame.Height;
            jpeg = frame.IsJpeg;
        }
        Thread.Sleep(10);
    }
    p.Stop();
    Console.WriteLine($"    preview: {frames} frames in {stopwatch.Elapsed.TotalSeconds:F1} s @ {width}x{height} ({(jpeg ? "MJPEG" : "YUYV")})");
    if (forceYuyv && lastSeq != 0)
    {
        p.Start(OpenGHub.Core.V4l2.V4l2Device.PixFmtYUYV);
        PreviewFrame? f = null;
        for (int i = 0; i < 30 && f is null; i++)
        {
            if (p.TryGetFrame(out PreviewFrame frame))
                f = frame;
            Thread.Sleep(10);
        }
        p.Stop();
        if (f is not null)
            Console.WriteLine($"    yuyv pixel probe: {string.Join(",", f.Pixels.Take(12))}");
    }
}

static async Task ProbeMouseAsync(HidBackend backend, DeviceInfo d, int? setDpi, byte[]? setRgb)
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

    if (mouse.RgbSupported)
    {
        Console.WriteLine("    RGB: supported (logo zone)");
        if (setRgb is byte[] rgb)
        {
            try
            {
                await mouse.SetColorAsync(rgb[0], rgb[1], rgb[2]);
                Console.WriteLine($"    set RGB #{rgb[0]:x2}{rgb[1]:x2}{rgb[2]:x2}: written (no read-back protocol)");
            }
            catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException or InvalidOperationException)
            {
                Console.WriteLine($"    set RGB: {e.Message}");
            }
        }
        else
        {
            Console.WriteLine("    pass --set-rgb=RRGGBB to write a static LED color");
        }
    }
    else
    {
        Console.WriteLine("    RGB: not supported on this unit");
    }

    try
    {
        IReadOnlyList<ButtonInfo> buttons = await mouse.GetButtonsAsync();
        Console.WriteLine($"    buttons: {buttons.Count} controls ({string.Join(", ", buttons.Select(b => $"{b.Name}{(b.IsDivertable ? "*" : "")}"))})");
    }
    catch (InvalidOperationException)
    {
        Console.WriteLine("    buttons: no reprogrammable controls feature");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    buttons: {e.Message}");
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
        Console.WriteLine($"  {d.Path}: permission denied — hidraw needs root or a udev rule (see the Open G Hub install/); run the PoC with sudo");
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

static async Task ProbeHeadsetAsync(HidBackend backend, DeviceInfo d, int? setSidetone, string? setEqArg)
{
    ProxHeadset? headset;
    try
    {
        headset = await ProxHeadset.OpenAsync(backend, d);
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
    if (headset is null)
        return; // duplicate receiver sibling; the canonical node printed it

    await using var _ = headset;
    Console.WriteLine($"  {d.Path}: {headset.Name} (product {ProxHeadset.ProductId:x4})");

    try
    {
        (int percent, bool charging) = await headset.GetBatteryAsync();
        Console.WriteLine($"    battery: {percent}% {(charging ? "(charging)" : "(discharging)")}");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    battery: {e.Message}");
    }

    try
    {
        int sidetone = await headset.GetSidetoneAsync();
        Console.WriteLine($"    sidetone: {sidetone}/100");
        if (setSidetone is int target)
        {
            await headset.SetSidetoneAsync(target);
            int after = await headset.GetSidetoneAsync();
            Console.WriteLine($"    set sidetone {target}: read back {after} (restoring {sidetone})");
            await headset.SetSidetoneAsync(sidetone);
        }
        else
        {
            Console.WriteLine("    pass --set-sidetone=N to write sidetone and read it back");
        }
    }
    catch (InvalidOperationException)
    {
        Console.WriteLine("    sidetone: feature not present on this unit");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    sidetone: {e.Message}");
    }

    try
    {
        Console.WriteLine($"    equalizer: {headset.EqFrequencies.Count} bands "
            + $"({string.Join(", ", headset.EqFrequencies.Select(f => $"{f} Hz"))}) range {headset.EqRange.Min}..{headset.EqRange.Max} dB");
        IReadOnlyList<int> levels = await headset.GetEqLevelsAsync();
        Console.WriteLine($"    eq levels: [{string.Join(", ", levels)}]");
        if (setEqArg is not null)
        {
            int[]? targets = setEqArg["--set-eq=".Length..].Split(',', StringSplitOptions.RemoveEmptyEntries)
                .Select(s => int.TryParse(s, out int v) ? v : (int?)null)
                .Where(v => v is not null)
                .Select(v => v!.Value)
                .ToArray();
            if (targets.Length == headset.EqFrequencies.Count)
            {
                await headset.SetEqLevelsAsync(targets);
                IReadOnlyList<int> after = await headset.GetEqLevelsAsync();
                Console.WriteLine($"    set eq [{string.Join(", ", targets)}]: read back [{string.Join(", ", after)}] (restoring [{string.Join(", ", levels)}])");
                await headset.SetEqLevelsAsync(levels);
            }
            else
            {
                Console.WriteLine($"    --set-eq needs {headset.EqFrequencies.Count} band values, got {targets.Length}");
            }
        }
        else
        {
            Console.WriteLine("    pass --set-eq=db,db,... to write EQ levels and read them back");
        }
    }
    catch (InvalidOperationException)
    {
        Console.WriteLine("    equalizer: feature not present on this unit");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    equalizer: {e.Message}");
    }

    try
    {
        bool nr = await headset.GetNoiseReductionAsync();
        Console.WriteLine($"    noise reduction: {nr}");
    }
    catch (Exception e) when (e is TimeoutException or HidppErrorException or InvalidDataException)
    {
        Console.WriteLine($"    noise reduction: not reported by this unit ({e.Message})");
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