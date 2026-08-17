namespace LogiTux.Core.Devices.Camera;

/// <summary>
/// One published preview frame from WebcamPreview. JPEG frames carry the
/// raw compressed bytes (IsJpeg true, decoding happens in the UI layer);
/// YUYV sources are already converted to BGRA pixels. Ported from
/// internal/device/webcam/webcam.go's preview frame holder.
/// </summary>
public sealed record PreviewFrame(byte[] Pixels, int Width, int Height, bool IsJpeg, ulong Sequence);