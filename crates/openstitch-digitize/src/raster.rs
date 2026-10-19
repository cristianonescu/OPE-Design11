//! Bounded image decoding and the RGBA raster every conversion works on.

use crate::DigitizeError;
use openstitch_design::{Vec2, v2};

/// Largest accepted image file.
pub const MAX_FILE_BYTES: usize = 32 << 20;
/// Largest accepted image side, pixels.
pub const MAX_SIDE: u32 = 8192;
/// Largest accepted pixel count (64 MiB as RGBA).
pub const MAX_PIXELS: u64 = 16 << 20;

/// 8-bit RGBA pixels, row-major, straight (not premultiplied) alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn check_size(width: u32, height: u32) -> Result<(), DigitizeError> {
    if width == 0 || height == 0 {
        return Err(DigitizeError::Empty(format!(
            "image is {width}×{height} pixels"
        )));
    }
    if width > MAX_SIDE || height > MAX_SIDE || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(DigitizeError::TooLarge(format!(
            "image is {width}×{height} pixels; the limit is {MAX_SIDE} per side and {MAX_PIXELS} pixels"
        )));
    }
    Ok(())
}

impl Raster {
    /// A raster from RGBA bytes; fails if the size is out of bounds or does not match.
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, DigitizeError> {
        check_size(width, height)?;
        if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err(DigitizeError::Decode(
                "pixel data does not match the size".into(),
            ));
        }
        Ok(Raster {
            width,
            height,
            rgba,
        })
    }

    /// A raster filled by `f(x, y)`.
    pub fn from_fn(
        width: u32,
        height: u32,
        f: impl Fn(u32, u32) -> [u8; 4],
    ) -> Result<Self, DigitizeError> {
        check_size(width, height)?;
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Self::from_rgba(width, height, rgba)
    }

    /// Decodes PNG, JPEG or BMP bytes. The header is checked against the size limits before
    /// any pixel memory is allocated, and the decoder's own allocation limit is set too.
    pub fn decode(bytes: &[u8]) -> Result<Self, DigitizeError> {
        use image::{ImageDecoder, ImageReader, Limits};
        if bytes.len() > MAX_FILE_BYTES {
            return Err(DigitizeError::TooLarge(format!(
                "image file is {} bytes; the limit is {MAX_FILE_BYTES}",
                bytes.len()
            )));
        }
        let mut reader = ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| DigitizeError::Decode(e.to_string()))?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(MAX_PIXELS * 8);
        reader.limits(limits);
        let decoder = reader
            .into_decoder()
            .map_err(|e| DigitizeError::Decode(e.to_string()))?;
        let (w, h) = decoder.dimensions();
        check_size(w, h)?;
        let image = image::DynamicImage::from_decoder(decoder)
            .map_err(|e| DigitizeError::Decode(e.to_string()))?;
        let rgba = image.into_rgba8();
        Self::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
    }

    /// PNG encoding of the raster (for previews of intermediate stages).
    pub fn encode_png(&self) -> Result<Vec<u8>, DigitizeError> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut out,
            &self.rgba,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| DigitizeError::Decode(e.to_string()))?;
        Ok(out.into_inner())
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Alpha-weighted mean colour of the pixel box `[x0, x1) × [y0, y1)` (clamped to the
    /// image): `[r, g, b, coverage]` with coverage 0–255 the mean alpha.
    pub fn box_mean(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> [u8; 4] {
        let (x1, y1) = (x1.min(self.width), y1.min(self.height));
        let (x0, y0) = (x0.min(x1), y0.min(y1));
        let mut sum = [0u64; 4];
        for y in y0..y1 {
            for x in x0..x1 {
                let p = self.pixel(x, y);
                let a = u64::from(p[3]);
                sum[0] += u64::from(p[0]) * a;
                sum[1] += u64::from(p[1]) * a;
                sum[2] += u64::from(p[2]) * a;
                sum[3] += a;
            }
        }
        let n = u64::from(x1 - x0) * u64::from(y1 - y0);
        if n == 0 || sum[3] == 0 {
            return [0, 0, 0, 0];
        }
        let c = |s: u64| ((s + sum[3] / 2) / sum[3]) as u8;
        [
            c(sum[0]),
            c(sum[1]),
            c(sum[2]),
            ((sum[3] + n / 2) / n) as u8,
        ]
    }

    /// Area-averaged copy whose longer side is at most `max_side` (never enlarged).
    pub fn fit(&self, max_side: u32) -> Raster {
        let max_side = max_side.max(1);
        let long = self.width.max(self.height);
        if long <= max_side {
            return self.clone();
        }
        let scale =
            |v: u32| ((u64::from(v) * u64::from(max_side)).div_ceil(u64::from(long))) as u32;
        let (w, h) = (scale(self.width).max(1), scale(self.height).max(1));
        self.resample(w, h)
    }

    /// Area-averaged resample to exactly `w`×`h` (each output pixel averages the source box
    /// it covers, at least one source pixel).
    pub fn resample(&self, w: u32, h: u32) -> Raster {
        let span = |i: u32, out: u32, src: u32| {
            let a = (u64::from(i) * u64::from(src) / u64::from(out)) as u32;
            let b = (u64::from(i + 1) * u64::from(src) / u64::from(out)) as u32;
            (a.min(src - 1), b.max(a + 1).min(src))
        };
        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
        for y in 0..h {
            let (y0, y1) = span(y, h, self.height);
            for x in 0..w {
                let (x0, x1) = span(x, w, self.width);
                rgba.extend_from_slice(&self.box_mean(x0, y0, x1, y1));
            }
        }
        Raster {
            width: w,
            height: h,
            rgba,
        }
    }

    /// Luminance 0–255 (Rec. 601) composited over white.
    pub fn luma_over_white(&self, x: u32, y: u32) -> f64 {
        let [r, g, b, a] = self.pixel(x, y);
        let a = f64::from(a) / 255.0;
        let l = 0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b);
        l * a + 255.0 * (1.0 - a)
    }
}

/// Where an image lands in the design: top-left corner and size of one pixel, 0.1 mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec2,
    /// Design units (0.1 mm) per pixel of the raster being converted.
    pub scale: f64,
}

impl Frame {
    /// A frame placing a `pixels_wide`-pixel image `width_mm` wide with its top-left corner at
    /// (`x_mm`, `y_mm`).
    pub fn new(x_mm: f64, y_mm: f64, width_mm: f64, pixels_wide: u32) -> Frame {
        Frame {
            origin: v2(x_mm * 10.0, y_mm * 10.0),
            scale: width_mm * 10.0 / f64::from(pixels_wide.max(1)),
        }
    }

    /// The same placement for a raster resampled to `pixels_wide` from `from_pixels` wide.
    pub fn rescaled(&self, from_pixels: u32, pixels_wide: u32) -> Frame {
        Frame {
            origin: self.origin,
            scale: self.scale * f64::from(from_pixels.max(1)) / f64::from(pixels_wide.max(1)),
        }
    }

    pub fn point(&self, x: f64, y: f64) -> Vec2 {
        v2(
            self.origin.x + x * self.scale,
            self.origin.y + y * self.scale,
        )
    }

    /// Millimetres per pixel.
    pub fn mm_per_px(&self) -> f64 {
        self.scale / 10.0
    }
}
