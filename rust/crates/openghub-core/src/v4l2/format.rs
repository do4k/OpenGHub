//! A negotiated capture format — the subset of `v4l2_pix_format` that frame
//! decoding needs. Ported from `internal/v4l2/v4l2.go` (`Format`).

/// A negotiated capture format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V4l2Format {
    pub width: i32,
    pub height: i32,
    pub pixel_format: u32,
    pub bytes_per_line: i32,
    pub size_image: i32,
}
