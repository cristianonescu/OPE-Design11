//! Thumbnails: sewn stitches drawn in their thread colours on a background, fitted into a
//! square of a given size (tiny-skia, pure Rust).

use openstitch_core::{Command, StitchPattern};
use tiny_skia::{Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Largest thumbnail side drawn, in pixels.
pub const MAX_SIDE: u32 = 4096;

/// An RGBA image with straight (unpremultiplied) alpha, rows top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// RGB bytes, alpha dropped (thumbnails are drawn opaque).
    pub fn rgb(&self) -> Vec<u8> {
        self.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect()
    }

    pub fn png(&self) -> Vec<u8> {
        encode_png(self.width, self.height, &self.rgb())
    }
}

/// Draws `pattern` centred in a `side`×`side` square on `background` (RGB).
pub fn render(pattern: &StitchPattern, side: u32, background: [u8; 3]) -> Image {
    render_rect(pattern, side, side, background)
}

/// Draws `pattern` centred in a `width`×`height` image, keeping its proportions.
pub fn render_rect(pattern: &StitchPattern, width: u32, height: u32, background: [u8; 3]) -> Image {
    let (w, h) = (width.clamp(1, MAX_SIDE), height.clamp(1, MAX_SIDE));
    let Some(mut pixmap) = Pixmap::new(w, h) else {
        return Image {
            width: w,
            height: h,
            rgba: vec![255; (w * h * 4) as usize],
        };
    };
    pixmap.fill(Color::from_rgba8(
        background[0],
        background[1],
        background[2],
        255,
    ));
    if pattern.count(Command::Stitch) > 0 {
        draw(&mut pixmap, pattern, w, h);
    }
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Image {
        width: w,
        height: h,
        rgba,
    }
}

fn draw(pixmap: &mut Pixmap, pattern: &StitchPattern, w: u32, h: u32) {
    let b = pattern.bounds();
    let margin = 0.04 * f64::from(w.min(h));
    let scale = ((f64::from(w) - 2.0 * margin) / b.width().max(1.0))
        .min((f64::from(h) - 2.0 * margin) / b.height().max(1.0))
        .max(1e-6);
    let ox = (f64::from(w) - b.width() * scale) / 2.0 - b.min_x * scale;
    let oy = (f64::from(h) - b.height() * scale) / 2.0 - b.min_y * scale;
    let at = |x: f64, y: f64| ((x * scale + ox) as f32, (y * scale + oy) as f32);
    // About 0.35 mm of thread, at least a fine visible line.
    let stroke = Stroke {
        width: ((3.5 * scale) as f32).clamp(1.0, 6.0),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    let mut block = 0usize;
    let mut pb = PathBuilder::new();
    let mut last: Option<(f64, f64)> = None;
    let mut cut = true;
    let flush = |pb: &mut PathBuilder, pixmap: &mut Pixmap, paint: &mut Paint, block: usize| {
        let t = pattern.threads.get(block).or(pattern.threads.last());
        let (r, g, b) = t.map_or((0, 0, 0), |t| (t.red, t.green, t.blue));
        paint.set_color_rgba8(r, g, b, 255);
        if let Some(path) = std::mem::take(pb).finish() {
            pixmap.stroke_path(&path, paint, &stroke, Transform::identity(), None);
        }
    };
    for s in &pattern.stitches {
        match s.command {
            Command::Stitch => {
                match last {
                    Some((x, y)) if !cut => {
                        if pb.is_empty() {
                            let (px, py) = at(x, y);
                            pb.move_to(px, py);
                        }
                        let (px, py) = at(s.x, s.y);
                        pb.line_to(px, py);
                    }
                    _ => {
                        let (px, py) = at(s.x, s.y);
                        pb.move_to(px, py);
                    }
                }
                cut = false;
                last = Some((s.x, s.y));
            }
            Command::Jump => {
                cut = true;
                last = Some((s.x, s.y));
            }
            Command::Trim => cut = true,
            Command::ColorChange => {
                flush(&mut pb, pixmap, &mut paint, block);
                block += 1;
                cut = true;
            }
            Command::Stop | Command::End => {}
        }
    }
    flush(&mut pb, pixmap, &mut paint, block);
}

/// Encodes 8-bit RGB as PNG.
pub fn encode_png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let written = enc.write_header().and_then(|mut w| w.write_image_data(rgb));
        if written.is_err() {
            return Vec::new();
        }
    }
    out
}
