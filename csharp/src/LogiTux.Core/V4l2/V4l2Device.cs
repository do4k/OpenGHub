using System.Runtime.InteropServices;

namespace LogiTux.Core.V4l2;

/// <summary>Minimal Video4Linux2 control API: querying and setting controls
/// (zoom, pan/tilt, focus, exposure, image tuning) via the VIDIOC_QUERYCAP /
/// QUERYCTRL / G_CTRL / S_CTRL ioctls, mirroring internal/v4l2/v4l2.go.
/// No capture support (no libv4l, no cgo) — pure BCL P/Invoke.</summary>
public sealed class V4l2Device : IDisposable
{
    // Control IDs, from linux/v4l2-controls.h. User class is 0x00980900 + n;
    // camera class is 0x009a0900 + n.
    public const uint CtrlBrightness = 0x00980900 + 0;
    public const uint CtrlContrast = 0x00980900 + 1;
    public const uint CtrlSaturation = 0x00980900 + 2;
    public const uint CtrlSharpness = 0x00980900 + 27;

    public const uint CtrlExposureAuto = 0x009a0900 + 1;
    public const uint CtrlExposureAbsolute = 0x009a0900 + 2;
    public const uint CtrlPanAbsolute = 0x009a0900 + 8;
    public const uint CtrlTiltAbsolute = 0x009a0900 + 9;
    public const uint CtrlFocusAbsolute = 0x009a0900 + 10;
    public const uint CtrlFocusAuto = 0x009a0900 + 12;
    public const uint CtrlZoomAbsolute = 0x009a0900 + 13;

    /// <summary>v4l2_exposure_auto_type menu values for CtrlExposureAuto.</summary>
    public const int ExposureManual = 1;
    public const int ExposureAperturePrio = 3;

    /// <summary>Capability flag (v4l2_capability.device_caps): the capture node
    /// among a webcam's several /dev/videoN siblings.</summary>
    public const uint CapVideoCapture = 0x00000001;

    private const uint FlagDisabled = 0x0001;

    // Struct sizes fixed by the kernel ABI (64-bit).
    private const int SizeofCapability = 104;
    private const int SizeofQueryctrl = 68;
    private const int SizeofControl = 8;

    // ioctl request-code construction, mirroring the _IOC macro: dir 30-31,
    // size 16-29, type 8-15, nr 0-7.
    private const uint IocRead = 2;
    private const uint IocWrite = 1;
    private const int IocTypeV = 'V'; // 0x56

    private static ulong Vidioc(uint dir, int nr, uint size) =>
        (ulong)dir << 30 | (ulong)size << 16 | (ulong)IocTypeV << 8 | (uint)nr;

    private static readonly ulong VidiocQuerycap = Vidioc(IocRead, 0, SizeofCapability);
    private static readonly ulong VidiocGCtrl = Vidioc(IocRead | IocWrite, 27, SizeofControl);
    private static readonly ulong VidiocSCtrl = Vidioc(IocRead | IocWrite, 28, SizeofControl);
    private static readonly ulong VidiocQueryctrl = Vidioc(IocRead | IocWrite, 36, SizeofQueryctrl);

    [DllImport("libc", SetLastError = true, EntryPoint = "open")]
    private static extern int Open([MarshalAs(UnmanagedType.LPUTF8Str)] string path, int flags);

    [DllImport("libc", SetLastError = true, EntryPoint = "close")]
    private static extern int Close(int fd);

    [DllImport("libc", SetLastError = true)]
    private static extern int ioctl(int fd, ulong request, byte[] data);

    private const int O_Rdwr = 2;

    private readonly string _path;
    private int _fd;

    public V4l2Device(string path)
    {
        _path = path;
        _fd = Open(path, O_Rdwr);
        if (_fd < 0)
            throw new IOException($"v4l2: open {path}: {Marshal.GetLastPInvokeError()}");
    }

    public void Dispose()
    {
        if (_fd >= 0)
        {
            Close(_fd);
            _fd = -1;
        }
    }

    private void Ioctl(ulong request, byte[] data)
    {
        if (ioctl(_fd, request, data) != 0)
            throw new IOException($"v4l2: ioctl {_path}: error {Marshal.GetLastPInvokeError()}");
    }

    /// <summary>Returns the device's card name and device_caps flags
    /// (v4l2_capability layout: driver[16], card[32], bus_info[32], version
    /// u32, capabilities u32, device_caps u32, reserved u32[3]).</summary>
    public (string Card, uint DeviceCaps) QueryCap()
    {
        var buf = new byte[SizeofCapability];
        Ioctl(VidiocQuerycap, buf);
        return (CString(buf, 16, 32), BitConverter.ToUInt32(buf, 88));
    }

    /// <summary>Range of a control, or false if the device lacks it (or it is
    /// disabled), mirroring QueryControl's skip-don't-fail contract.
    /// (v4l2_queryctrl layout: id u32, type u32, name[32], minimum s32,
    /// maximum s32, step s32, default_value s32, flags u32, reserved u32[2].)</summary>
    public bool TryQueryControl(uint id, out int min, out int max, out int step, out int defaultValue)
    {
        min = max = step = defaultValue = 0;
        var buf = new byte[SizeofQueryctrl];
        BitConverter.GetBytes(id).CopyTo(buf, 0);
        try
        {
            Ioctl(VidiocQueryctrl, buf);
        }
        catch (IOException)
        {
            return false; // control not present
        }
        if (BitConverter.ToUInt32(buf, 56) is uint flags && (flags & FlagDisabled) != 0)
            return false; // present but disabled (e.g. auto mode owns it)
        min = BitConverter.ToInt32(buf, 40);
        max = BitConverter.ToInt32(buf, 44);
        step = BitConverter.ToInt32(buf, 48);
        defaultValue = BitConverter.ToInt32(buf, 52);
        return true;
    }

    /// <summary>Reads a control's current value (v4l2_control layout: id u32,
    /// value s32).</summary>
    public int Get(uint id)
    {
        var buf = new byte[SizeofControl];
        BitConverter.GetBytes(id).CopyTo(buf, 0);
        Ioctl(VidiocGCtrl, buf);
        return BitConverter.ToInt32(buf, 4);
    }

    /// <summary>Writes a control's value.</summary>
    public void Set(uint id, int value)
    {
        var buf = new byte[SizeofControl];
        BitConverter.GetBytes(id).CopyTo(buf, 0);
        BitConverter.GetBytes(value).CopyTo(buf, 4);
        Ioctl(VidiocSCtrl, buf);
    }

    private static string CString(byte[] b, int offset, int length)
    {
        int end = Array.IndexOf(b, (byte)0, offset, Math.Min(length, b.Length - offset));
        if (end < 0)
            end = Math.Min(offset + length, b.Length);
        return System.Text.Encoding.ASCII.GetString(b, offset, end - offset);
    }
}