namespace LogiTux.Core.Hid;

/// <summary>
/// Describes a discovered HID device.
/// </summary>
public sealed record DeviceInfo(
    string Path,
    ushort VendorId,
    ushort ProductId,
    string? Serial);
