//! File-manager thumbnails: the stitches of an `.osd` project or a machine embroidery file
//! (PES, DST, JEF, ... — every format `openstitch-formats` reads), cropped to the design and
//! centred on a transparent square.

use crate::preview::{RunKind, runs};
use openstitch_core::{Command, StitchPattern};
use openstitch_design::Compiler;
use openstitch_formats::FileType;
use std::path::Path;
use tiny_skia::{Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Files larger than this are not read (the same cap the engine uses for imports).
pub const MAX_INPUT: u64 = 64 << 20;
pub const MIN_SIZE: u32 = 16;
pub const MAX_SIZE: u32 = 1024;

/// The stitches of `path`: a compiled `.osd` project, or a machine file by its extension.
pub fn load(path: &Path) -> Result<StitchPattern, String> {
    let len = std::fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if len > MAX_INPUT {
        return Err(format!(
            "{} is larger than {} MiB",
            path.display(),
            MAX_INPUT >> 20
        ));
    }
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let is_project = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(openstitch_engine::project::EXTENSION));
    if is_project {
        let design = openstitch_engine::project::decode(&data).map_err(|e| e.to_string())?;
        return Ok(Compiler::for_design(&design).compile(&design));
    }
    let ty = FileType::from_path(path)
        .ok_or_else(|| format!("{}: not an embroidery file", path.display()))?;
    ty.read(&data)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A `size`×`size` RGBA PNG of the sewn stitches, scaled to fit with a small margin. Travel is
/// left out; thread is drawn at about 0.4 mm, but never thinner than one pixel.
pub fn png(pattern: &StitchPattern, size: u32) -> Result<Vec<u8>, String> {
    let size = size.clamp(MIN_SIZE, MAX_SIZE);
    let mut pixmap =
        Pixmap::new(size, size).ok_or_else(|| format!("cannot create a {size}px bitmap"))?;
    pixmap.fill(Color::TRANSPARENT);
    if pattern.count(Command::Stitch) > 0 {
        let b = pattern.bounds();
        let extent = b.width().max(b.height()).max(1.0);
        let margin = f64::from(size) * 0.04;
        let scale = (f64::from(size) - 2.0 * margin) / extent;
        let (cx, cy) = ((b.min_x + b.max_x) / 2.0, (b.min_y + b.max_y) / 2.0);
        let half = f64::from(size) / 2.0;
        let transform = Transform::from_row(
            scale as f32,
            0.0,
            0.0,
            scale as f32,
            (half - cx * scale) as f32,
            (half - cy * scale) as f32,
        );
        let stroke = Stroke {
            width: ((4.0 * scale).max(1.0) / scale) as f32,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        let threads: Vec<_> = pattern.color_blocks().into_iter().map(|(t, _)| t).collect();
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        for run in runs(pattern)
            .into_iter()
            .filter(|r| r.kind == RunKind::Sewn)
        {
            let mut pb = PathBuilder::new();
            let mut points = run.points.iter();
            let Some(&(x, y)) = points.next() else {
                continue;
            };
            pb.move_to(x as f32, y as f32);
            for &(x, y) in points {
                pb.line_to(x as f32, y as f32);
            }
            let Some(path) = pb.finish() else { continue };
            let t = threads.get(run.block).copied().flatten();
            paint.set_color_rgba8(
                t.map_or(0, |t| t.red),
                t.map_or(0, |t| t.green),
                t.map_or(0, |t| t.blue),
                255,
            );
            pixmap.stroke_path(&path, &paint, &stroke, transform, None);
        }
    }
    encode_rgba(&pixmap)
}

/// Straight-alpha RGBA PNG from tiny-skia's premultiplied pixels.
fn encode_rgba(pixmap: &Pixmap) -> Result<Vec<u8>, String> {
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    writer
        .write_image_data(&rgba)
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_core::Thread;

    fn square() -> StitchPattern {
        let mut p = StitchPattern::new();
        p.threads.push(Thread::rgb(0xFF0000));
        p.jump_to(0.0, 0.0);
        for (x, y) in [
            (0.0, 0.0),
            (200.0, 0.0),
            (200.0, 100.0),
            (0.0, 100.0),
            (0.0, 0.0),
        ] {
            p.stitch_to(x, y);
        }
        p.push(Command::End);
        p
    }

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .expect("png");
        let mut buf = vec![0; reader.output_buffer_size().expect("size")];
        let info = reader.next_frame(&mut buf).expect("frame");
        assert_eq!(info.color_type, png::ColorType::Rgba);
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    #[test]
    fn thumbnail_is_square_transparent_and_draws_threads() {
        let (w, h, px) = decode(&png(&square(), 128).expect("png"));
        assert_eq!((w, h), (128, 128));
        let at = |x: u32, y: u32| &px[((y * w + x) * 4) as usize..][..4];
        assert_eq!(at(64, 64)[3], 0, "inside the outline is empty");
        assert_eq!(at(0, 0)[3], 0, "corner is transparent");
        // The left edge of the 2:1 rectangle sits at the margin, vertically centred.
        let edge = (0..20)
            .map(|x| at(x, 64))
            .find(|p| p[3] == 255)
            .expect("edge");
        assert_eq!(&edge[..3], &[255, 0, 0]);
    }

    #[test]
    fn size_is_clamped_and_empty_patterns_render_blank() {
        let (w, _, px) = decode(&png(&StitchPattern::new(), 4).expect("png"));
        assert_eq!(w, MIN_SIZE);
        assert!(px.chunks(4).all(|p| p[3] == 0));
        let (w, _, _) = decode(&png(&square(), 100_000).expect("png"));
        assert_eq!(w, MAX_SIZE);
    }

    #[test]
    fn loads_projects_and_machine_files_and_rejects_others() {
        let dir = std::env::temp_dir().join(format!("openstitch-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let pes = dir.join("a.pes");
        std::fs::write(&pes, FileType::Pes.write(&square()).expect("pes")).expect("write");
        assert!(load(&pes).expect("pes").count(Command::Stitch) >= 5);

        let mut design = openstitch_design::Design::default();
        design.add(openstitch_design::DesignObject::shape(
            vec![openstitch_design::Contour::rectangle(
                100.0, 100.0, 200.0, 200.0,
            )],
            Some(openstitch_design::RegionSewing::new(
                openstitch_design::RegionType::Fill,
                Thread::rgb(0x00AA00),
            )),
            None,
        ));
        let osd = dir.join("b.osd");
        std::fs::write(
            &osd,
            openstitch_engine::project::encode(&design).expect("osd"),
        )
        .expect("write");
        assert!(load(&osd).expect("osd").count(Command::Stitch) > 50);

        let txt = dir.join("c.txt");
        std::fs::write(&txt, "hello").expect("write");
        assert!(load(&txt).is_err());
        std::fs::write(dir.join("d.pes"), b"#PES0001 garbage").expect("write");
        assert!(load(&dir.join("d.pes")).is_err());
        assert!(load(&dir.join("missing.pes")).is_err());
    }
}
