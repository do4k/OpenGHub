using OpenGHub.Core.V4l2;

namespace OpenGHub.Core.Devices.Camera;

/// <summary>
/// Control device plugin for Logitech UVC webcams (C920 family, C922,
/// C930e), ported from internal/device/webcam. Not a HID device: connected
/// cameras are found by scanning /sys/class/video4linux modalias files and
/// controlled through /dev/videoN with the V4L2 ioctls in V4l2Device.
/// Control ranges are queried per device, never assumed.
/// </summary>
public sealed class V4l2Camera : IDisposable
{
    public const ushort VendorId = 0x046d;

    private const string ClassPath = "/sys/class/video4linux";
    private const string DevPath = "/dev";

    /// <summary>
    /// Known Logitech UVC webcams (name allowlist; ranges come from
    /// the driver).
    /// </summary>
    private static readonly Dictionary<ushort, string> ProductNames = new()
    {
        [0x082d] = "C920",
        [0x0892] = "C920 HD Pro",
        [0x085c] = "C922 Pro Stream",
        [0x0843] = "C930e",
    };

    /// <summary>
    /// Every control we know how to render, in display order.
    /// Names are stable identifiers the GUI keys on.
    /// </summary>
    private static readonly (string Name, uint Id)[] CandidateControls =
    {
        ("Zoom", V4l2Device.CtrlZoomAbsolute),
        ("Pan", V4l2Device.CtrlPanAbsolute),
        ("Tilt", V4l2Device.CtrlTiltAbsolute),
        ("Auto Focus", V4l2Device.CtrlFocusAuto),
        ("Focus", V4l2Device.CtrlFocusAbsolute),
        ("Auto Exposure", V4l2Device.CtrlExposureAuto),
        ("Exposure", V4l2Device.CtrlExposureAbsolute),
        ("Brightness", V4l2Device.CtrlBrightness),
        ("Contrast", V4l2Device.CtrlContrast),
        ("Saturation", V4l2Device.CtrlSaturation),
        ("Sharpness", V4l2Device.CtrlSharpness),
    };

    private readonly V4l2Device _dev;
    private readonly Dictionary<string, uint> _ids;

    public string Name { get; }
    public string Serial { get; }
    public ushort ProductId { get; }
    public string DevicePath { get; }
    public IReadOnlyList<CameraControl> Controls { get; }

    private V4l2Camera(V4l2Device dev, string name, string serial, ushort productId, string path,
        List<CameraControl> controls, Dictionary<string, uint> ids)
    {
        _dev = dev;
        _ids = ids;
        Name = name;
        Serial = serial;
        ProductId = productId;
        DevicePath = path;
        Controls = controls;
    }

    public void Dispose() => _dev.Dispose();

    /// <summary>
    /// A live preview sharing this camera's already-open device node.
    /// Callers own the preview and must Stop/Dispose it before the camera.
    /// </summary>
    public WebcamPreview CreatePreview() => new(_dev, Name);

    /// <summary>
    /// Opens one device node. Returns null for nodes that belong to a
    /// known camera but aren't its video-capture interface (UVC also exposes
    /// metadata nodes). OPENS THE NODE — callers must Dispose.
    /// </summary>
    /// <summary>
    /// Product name for a videoN node (for error cards), or null if
    /// the node isn't a known Logitech camera.
    /// </summary>
    public static string? ProductNameForNode(string node)
    {
        string modaliasPath = Path.Combine(ClassPath, node, "device", "modalias");
        if (!TryUsbIds(modaliasPath, out ushort vendor, out ushort product))
            return null;
        return vendor == VendorId && ProductNames.TryGetValue(product, out string? name) ? name : null;
    }

    public static V4l2Camera? Open(string node)
    {
        string path = Path.Combine(DevPath, node);

        // Find out who owns this node without opening it: the USB vendor/product
        // live in the device's modalias (usb:vVENDpPROD...).
        string modaliasPath = Path.Combine(ClassPath, node, "device", "modalias");
        if (!TryUsbIds(modaliasPath, out ushort vendor, out ushort product))
            return null;
        if (vendor != VendorId || !ProductNames.TryGetValue(product, out string? productName))
            return null;

        var dev = new V4l2Device(path);
        try
        {
            (_, uint caps) = dev.QueryCap();
            if ((caps & V4l2Device.CapVideoCapture) == 0)
            {
                dev.Dispose();
                return null; // e.g. a UVC metadata node, not the capture interface
            }

            var controls = new List<CameraControl>();
            var ids = new Dictionary<string, uint>(CandidateControls.Length);
            foreach ((string name, uint id) in CandidateControls)
            {
                if (!dev.TryQueryControl(id, out int min, out int max, out int step, out int defaultValue))
                    continue; // this unit doesn't have the control; not an error

                if (id == V4l2Device.CtrlExposureAuto)
                {
                    // Driver reports a menu (1=manual, 3=aperture-priority auto);
                    // present it as a boolean.
                    (min, max, step, defaultValue) = (0, 1, 1, 1);
                }
                controls.Add(new CameraControl(name, min, max, step, defaultValue));
                ids[name] = id;
            }
            if (controls.Count == 0)
            {
                dev.Dispose();
                return null;
            }

            return new V4l2Camera(dev, productName, SerialFor(node), product, path, controls, ids);
        }
        catch
        {
            dev.Dispose();
            throw;
        }
    }

    /// <summary>
    /// Enumerates known Logitech webcam nodes via /sys/class/video4linux.
    /// Returns nodes of matching cameras, capture or not — callers Open() them
    /// and get null back from the non-capture ones. Mirrors webcam.discover.
    /// </summary>
    public static IReadOnlyList<string> EnumerateNodes()
    {
        var nodes = new List<string>();
        if (!Directory.Exists(ClassPath))
            return nodes;

        foreach (string entry in Directory.EnumerateDirectories(ClassPath).Order())
        {
            string modaliasPath = Path.Combine(entry, "device", "modalias");
            if (!TryUsbIds(modaliasPath, out ushort vendor, out ushort product))
                continue;
            if (vendor == VendorId && ProductNames.ContainsKey(product))
                nodes.Add(Path.GetFileName(entry));
        }
        return nodes;
    }

    public int GetControl(string name)
    {
        uint id = _ids[name];
        int value = _dev.Get(id);
        if (id == V4l2Device.CtrlExposureAuto)
            return value == V4l2Device.ExposureManual ? 0 : 1;
        return value;
    }

    public void SetControl(string name, int value)
    {
        uint id = _ids[name];
        if (id == V4l2Device.CtrlExposureAuto)
            value = value == 0 ? V4l2Device.ExposureManual : V4l2Device.ExposureAperturePrio;
        _dev.Set(id, value);
    }

    /// <summary>
    /// Parses a sysfs modalias like "usb:v046Dp085Cd0016..." into
    /// vendor/product IDs.
    /// </summary>
    private static bool TryUsbIds(string modaliasPath, out ushort vendor, out ushort product)
    {
        vendor = default;
        product = default;
        if (!File.Exists(modaliasPath))
            return false;

        string s = File.ReadAllText(modaliasPath).Trim();
        if (!s.StartsWith("usb:v", StringComparison.Ordinal) ||
            s.Length < 14 || s[9] != 'p')
            return false;

        Span<char> v = stackalloc char[] { s[5], s[6], s[7], s[8] };
        Span<char> p = stackalloc char[] { s[10], s[11], s[12], s[13] };
        if (!ushort.TryParse(v, System.Globalization.NumberStyles.AllowHexSpecifier, null, out vendor) ||
            !ushort.TryParse(p, System.Globalization.NumberStyles.AllowHexSpecifier, null, out product))
        {
            return false;
        }
        return true;
    }

    /// <summary>
    /// Reads the USB device serial from sysfs: the videoN symlink
    /// points at the USB interface directory; the serial lives one level up.
    /// Falls back to the node name.
    /// </summary>
    private static string SerialFor(string node)
    {
        string path = Path.Combine(ClassPath, node, "device", "..", "serial");
        if (File.Exists(path))
        {
            string serial = File.ReadAllText(path).Trim();
            if (serial.Length > 0)
                return serial;
        }
        return node;
    }
}
