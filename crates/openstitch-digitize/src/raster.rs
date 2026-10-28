//! Bounded image decoding and the RGBA raster every conversion works on.

use crate::DigitizeError;
use openstitch_design::{ImageAdjust, Vec2, v2};

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

    /// The raster turned by `quarters` × 90° clockwise.
    pub fn rotated(&self, quarters: u32) -> Raster {
        let q = quarters % 4;
        if q == 0 {
            return self.clone();
        }
        let (w, h) = if q == 2 {
            (self.width, self.height)
        } else {
            (self.height, self.width)
        };
        let mut rgba = Vec::with_capacity(self.rgba.len());
        for y in 0..h {
            for x in 0..w {
                // The source pixel that lands on (x, y).
                let (sx, sy) = match q {
                    1 => (y, self.height - 1 - x),
                    2 => (self.width - 1 - x, self.height - 1 - y),
                    _ => (self.width - 1 - y, x),
                };
                rgba.extend_from_slice(&self.pixel(sx, sy));
            }
        }
        Raster {
            width: w,
            height: h,
            rgba,
        }
    }

    /// The pixels `[x0, x1) × [y0, y1)`, clamped to the image and at least one pixel.
    pub fn cropped(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> Raster {
        let x0 = x0.min(self.width - 1);
        let y0 = y0.min(self.height - 1);
        let x1 = x1.clamp(x0 + 1, self.width);
        let y1 = y1.clamp(y0 + 1, self.height);
        let mut rgba = Vec::with_capacity((x1 - x0) as usize * (y1 - y0) as usize * 4);
        for y in y0..y1 {
            let i = (y as usize * self.width as usize + x0 as usize) * 4;
            rgba.extend_from_slice(&self.rgba[i..i + (x1 - x0) as usize * 4]);
        }
        Raster {
            width: x1 - x0,
            height: y1 - y0,
            rgba,
        }
    }

    /// Brightness, contrast and saturation changed, each −100 to 100 (0 = unchanged):
    /// brightness adds up to ±255 × ½, contrast scales the distance from mid-grey (−100 = flat
    /// grey, 100 = 3 ×), saturation scales the distance from the pixel's grey (−100 = grey).
    /// Alpha is kept.
    pub fn adjusted(&self, brightness: f64, contrast: f64, saturation: f64) -> Raster {
        let b = brightness.clamp(-100.0, 100.0) / 100.0 * 127.5;
        let c = contrast.clamp(-100.0, 100.0) / 100.0;
        let contrast = if c < 0.0 { 1.0 + c } else { 1.0 + 2.0 * c };
        let saturation = 1.0 + saturation.clamp(-100.0, 100.0) / 100.0;
        if b == 0.0 && contrast == 1.0 && saturation == 1.0 {
            return self.clone();
        }
        let mut rgba = self.rgba.clone();
        for px in rgba.as_chunks_mut::<4>().0 {
            let [r, g, bl] = [px[0], px[1], px[2]].map(f64::from);
            let grey = 0.299 * r + 0.587 * g + 0.114 * bl;
            for c in &mut px[..3] {
                let v = grey + (f64::from(*c) - grey) * saturation;
                let v = (v - 127.5) * contrast + 127.5 + b;
                *c = v.round().clamp(0.0, 255.0) as u8;
            }
        }
        Raster {
            width: self.width,
            height: self.height,
            rgba,
        }
    }

    /// The background image as shown on the Design Page: turned, cropped, then its colours
    /// changed by `a`.
    pub fn with_adjust(&self, a: &ImageAdjust) -> Raster {
        let r = self.rotated(u32::from(a.rotation));
        let (w, h) = (f64::from(r.width), f64::from(r.height));
        let [l, t, rt, b] = a.crop.map(|c| c.clamp(0.0, ImageAdjust::MAX_CROP));
        let r = if a.crop == [0.0; 4] {
            r
        } else {
            r.cropped(
                (l * w).round() as u32,
                (t * h).round() as u32,
                ((1.0 - rt) * w).round() as u32,
                ((1.0 - b) * h).round() as u32,
            )
        };
        r.adjusted(a.brightness, a.contrast, a.saturation)
    }

    /// The raster encoded as PNG, BMP or JPEG, chosen by file extension (`png`, `bmp`, `jpg`,
    /// `jpeg`; any case). BMP and JPEG are written over white, without alpha.
    pub fn encode_as(&self, extension: &str) -> Result<Vec<u8>, DigitizeError> {
        let format = match extension.to_ascii_lowercase().as_str() {
            "png" => return self.encode_png(),
            "bmp" => image::ImageFormat::Bmp,
            "jpg" | "jpeg" => image::ImageFormat::Jpeg,
            other => {
                return Err(DigitizeError::Decode(format!(
                    "cannot write .{other} images (use .png, .bmp or .jpg)"
                )));
            }
        };
        let mut rgb = Vec::with_capacity(self.width as usize * self.height as usize * 3);
        for px in self.rgba.as_chunks::<4>().0 {
            let a = u32::from(px[3]);
            for &c in &px[..3] {
                rgb.push(((u32::from(c) * a + 255 * (255 - a)) / 255) as u8);
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut out,
            &rgb,
            self.width,
            self.height,
            image::ExtendedColorType::Rgb8,
            format,
        )
        .map_err(|e| DigitizeError::Decode(e.to_string()))?;
        Ok(out.into_inner())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Raster {
        // 3 × 2: pixel value encodes its position.
        Raster::from_fn(3, 2, |x, y| [(10 * y + x) as u8, 0, 0, 255]).expect("raster")
    }

    #[test]
    fn rotation_turns_clockwise_and_round_trips() {
        let r = sample();
        let q = r.rotated(1);
        assert_eq!((q.width(), q.height()), (2, 3));
        // Clockwise: the bottom-left source pixel lands top-left.
        assert_eq!(q.pixel(0, 0)[0], 10);
        assert_eq!(q.pixel(1, 0)[0], 0);
        assert_eq!(q.pixel(1, 2)[0], 2);
        assert_eq!(r.rotated(2).pixel(0, 0)[0], 12);
        assert_eq!(r.rotated(3).pixel(0, 0)[0], 2);
        assert_eq!(q.rotated(3), r);
        assert_eq!(r.rotated(4), r);
    }

    #[test]
    fn crop_clamps_to_the_image() {
        let r = sample();
        let c = r.cropped(1, 1, 3, 2);
        assert_eq!((c.width(), c.height()), (2, 1));
        assert_eq!(c.pixel(0, 0)[0], 11);
        let tiny = r.cropped(9, 9, 0, 0);
        assert_eq!((tiny.width(), tiny.height()), (1, 1));
        assert_eq!(tiny.pixel(0, 0)[0], 12);
    }

    #[test]
    fn adjustments_brighten_flatten_and_desaturate() {
        let r = Raster::from_fn(1, 1, |_, _| [200, 100, 50, 128]).expect("raster");
        assert_eq!(r.adjusted(0.0, 0.0, 0.0), r);
        let bright = r.adjusted(100.0, 0.0, 0.0).pixel(0, 0);
        assert!(bright[0] == 255 && bright[2] > 50 && bright[3] == 128);
        let flat = r.adjusted(0.0, -100.0, 0.0).pixel(0, 0);
        assert!(flat[..3].iter().all(|&c| c == 128));
        let grey = r.adjusted(0.0, 0.0, -100.0).pixel(0, 0);
        assert!(grey[0] == grey[1] && grey[1] == grey[2]);
        let strong = r.adjusted(0.0, 100.0, 0.0).pixel(0, 0);
        assert!(strong[0] > 200 && strong[2] < 50);
    }

    #[test]
    fn encodes_png_bmp_and_jpeg() {
        let r = Raster::from_fn(8, 8, |x, y| [(x * 30) as u8, (y * 30) as u8, 90, 255])
            .expect("raster");
        for ext in ["png", "BMP", "jpg", "jpeg"] {
            let bytes = r.encode_as(ext).expect(ext);
            let back = Raster::decode(&bytes).expect("decode");
            assert_eq!((back.width(), back.height()), (8, 8), "{ext}");
        }
        assert_eq!(
            Raster::decode(&r.encode_as("bmp").expect("bmp")).expect("bmp"),
            r,
            "BMP is lossless"
        );
        assert!(r.encode_as("gif").is_err());
    }

    #[test]
    fn background_adjustments_turn_then_crop() {
        let r = Raster::from_fn(10, 4, |x, y| [(10 * y + x) as u8, 0, 0, 255]).expect("raster");
        assert_eq!(r.with_adjust(&ImageAdjust::default()), r);
        let a = ImageAdjust {
            rotation: 1,
            crop: [0.0, 0.5, 0.0, 0.0],
            ..ImageAdjust::default()
        };
        let out = r.with_adjust(&a);
        // Turned to 4 × 10, then the top half cut off.
        assert_eq!((out.width(), out.height()), (4, 5));
        assert_eq!(out.pixel(0, 0)[0], 35);
    }
}
