//! Source-image inspection and loading.
//!
//! [`SourceInfo`] is deliberately minimal: dimensions plus the two facts that
//! change the *layout* (whether the EXIF tag swaps the axes). `JobGeometry`
//! reads nothing else, so the same `(options, dimensions)` always produces the
//! same geometry regardless of which pixel data is loaded.

use std::path::{Path, PathBuf};

use image::ImageDecoder;

use crate::modern::color::{Rgb, RgbaF64};

/// Hard input guards. These are guards, not options.
pub const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
/// Hard pixel-count guard: 100 megapixels.
pub const MAX_PIXELS: u64 = 100_000_000;

/// What the layout solver needs to know about the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    /// Where the image came from; carried for messages, never opened here.
    pub path: PathBuf,
    /// Decoded width in pixels, before any orientation or rotation.
    pub width: u32,
    /// Decoded height in pixels, before any orientation or rotation.
    pub height: u32,
    /// True when the EXIF orientation tag rotates the image by 90 or 270 degrees.
    pub exif_swaps_axes: bool,
}

impl SourceInfo {
    /// Describe an in-memory source of the given size.
    pub fn synthetic(width: u32, height: u32) -> Self {
        SourceInfo {
            path: PathBuf::new(),
            width,
            height,
            exif_swaps_axes: false,
        }
    }

    /// Display size after the EXIF orientation is applied.
    ///
    /// The EXIF tag is applied *before* the user rotation, so this only reports
    /// the axis swap; the rotation itself is applied by the geometry solver.
    pub fn exif_display_size(&self) -> (f64, f64) {
        if self.exif_swaps_axes {
            (f64::from(self.height), f64::from(self.width))
        } else {
            (f64::from(self.width), f64::from(self.height))
        }
    }

    /// Pixel count, for the megapixel guard.
    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// Inspect a source file without fully decoding it.
pub fn inspect<P: AsRef<Path>>(path: P) -> Result<SourceInfo, String> {
    let path = path.as_ref();
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{} is {:.1} MB, over the {} MB limit",
            path.display(),
            meta.len() as f64 / (1024.0 * 1024.0),
            MAX_FILE_BYTES / (1024 * 1024)
        ));
    }
    let reader = image::ImageReader::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let (width, height) = decoder.dimensions();
    let orientation = decoder.orientation().ok();
    let info = SourceInfo {
        path: path.to_path_buf(),
        width,
        height,
        exif_swaps_axes: matches!(
            orientation,
            Some(image::metadata::Orientation::Rotate90)
                | Some(image::metadata::Orientation::Rotate270)
                | Some(image::metadata::Orientation::Rotate90FlipH)
                | Some(image::metadata::Orientation::Rotate270FlipH)
        ),
    };
    if info.pixels() > MAX_PIXELS {
        return Err(format!(
            "{} is {} pixels, over the {} megapixel limit",
            path.display(),
            info.pixels(),
            MAX_PIXELS / 1_000_000
        ));
    }
    Ok(info)
}

/// Decode a file to 8-bit RGBA, honouring the byte and pixel guards.
pub fn decode_rgba<P: AsRef<Path>>(path: P) -> Result<image::RgbaImage, String> {
    let path = path.as_ref();
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{} is {:.1} MB, over the {} MB limit",
            path.display(),
            meta.len() as f64 / (1024.0 * 1024.0),
            MAX_FILE_BYTES / (1024 * 1024)
        ));
    }
    let reader = image::ImageReader::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "{} is {} pixels, over the {} megapixel limit",
            path.display(),
            u64::from(width) * u64::from(height),
            MAX_PIXELS / 1_000_000
        ));
    }
    let orientation = decoder.orientation().ok();
    let img = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8();
    Ok(match orientation {
        Some(image::metadata::Orientation::NoTransforms) | None => img,
        Some(o) => apply_orientation(&img, o),
    })
}

fn apply_orientation(img: &image::RgbaImage, o: image::metadata::Orientation) -> image::RgbaImage {
    use image::metadata::Orientation as O;
    match o {
        O::NoTransforms => img.clone(),
        O::FlipHorizontal => image::imageops::flip_horizontal(img),
        O::FlipVertical => image::imageops::flip_vertical(img),
        O::Rotate180 => image::imageops::rotate180(img),
        O::Rotate90 => image::imageops::rotate90(img),
        O::Rotate270 => image::imageops::rotate270(img),
        O::Rotate90FlipH => image::imageops::flip_horizontal(&image::imageops::rotate90(img)),
        O::Rotate270FlipH => image::imageops::flip_horizontal(&image::imageops::rotate270(img)),
    }
}

/// A decoded source plus the linear sample buffer the engine consumes.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedImage {
    /// Width in pixels after every preparation step.
    pub width: u32,
    /// Height in pixels after every preparation step.
    pub height: u32,
    /// Row-major linear RGBA, `0..=1`, four samples per pixel.
    pub pixels: Vec<RgbaF64>,
    /// True when the optional box-filter resample ran.
    pub resampled: bool,
}

impl PreparedImage {
    /// Build from an 8-bit RGBA buffer, converting to linear `f64` samples.
    pub fn from_rgba(img: &image::RgbaImage) -> PreparedImage {
        let mut pixels = Vec::with_capacity(img.width() as usize * img.height() as usize * 4);
        for p in img.pixels() {
            pixels.push(RgbaF64::from_u8(p.0[0], p.0[1], p.0[2], p.0[3]));
        }
        PreparedImage {
            width: img.width(),
            height: img.height(),
            pixels,
            resampled: false,
        }
    }

    /// Sample at an integer pixel index, or `None` when out of bounds.
    pub fn pixel(&self, x: i64, y: i64) -> Option<RgbaF64> {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return None;
        }
        let i = (y as usize) * (self.width as usize) + (x as usize);
        self.pixels.get(i).copied()
    }

    /// Total pixel count.
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Aspect ratio.
    pub fn aspect(&self) -> f64 {
        f64::from(self.width) / f64::from(self.height)
    }

    /// A solid-colour image, handy for tests and fixtures.
    pub fn solid(width: u32, height: u32, c: Rgb) -> PreparedImage {
        let v = RgbaF64::from_rgb(c);
        PreparedImage {
            width,
            height,
            pixels: vec![v; width as usize * height as usize],
            resampled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_are_the_documented_ones() {
        assert_eq!(MAX_FILE_BYTES, 104_857_600);
        assert_eq!(MAX_PIXELS, 100_000_000);
    }

    #[test]
    fn synthetic_source_reports_dimensions() {
        let s = SourceInfo::synthetic(4000, 3000);
        assert_eq!(s.exif_display_size(), (4000.0, 3000.0));
        assert_eq!(s.pixels(), 12_000_000);
        let s = SourceInfo { exif_swaps_axes: true, ..SourceInfo::synthetic(4000, 3000) };
        assert_eq!(s.exif_display_size(), (3000.0, 4000.0));
    }

    #[test]
    fn prepared_image_indexing() {
        let img = PreparedImage::solid(3, 2, Rgb::BLACK);
        assert_eq!(img.pixel(0, 0).map(|p| p.a), Some(1.0));
        assert_eq!(img.pixel(2, 1).map(|p| p.a), Some(1.0));
        assert!(img.pixel(3, 0).is_none());
        assert!(img.pixel(-1, 0).is_none());
        assert_eq!(img.pixel_count(), 6);
        assert!((img.aspect() - 1.5).abs() < 1e-12);
    }

    #[test]
    fn inspect_rejects_a_missing_file() {
        let e = inspect("definitely-not-here.png").unwrap_err();
        assert!(e.contains("definitely-not-here.png"), "{e}");
    }
}