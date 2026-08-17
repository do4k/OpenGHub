namespace LogiTux.Core.V4l2;

/// <summary>
/// A negotiated capture format — the subset of v4l2_pix_format that
/// frame decoding needs. Ported from internal/v4l2/v4l2.go (Format).
/// </summary>
public sealed record V4l2Format(int Width, int Height, uint PixelFormat, int BytesPerLine, int SizeImage);