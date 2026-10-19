//! Draws generated stitches as a PNG with a pure-Rust rasterizer (tiny-skia, encoded by the
//! `png` crate): the Design Page outline, then each sewn run in its thread colour, optionally
//! with travel (jumps and stitches after a cut) dashed in grey.

use openstitch_core::{Command, StitchPattern};
use openstitch_design::Design;
use tiny_skia::{
    Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, StrokeDash, Transform,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Sewn,
    Travel,
}

/// A polyline of consecutive needle moves of one kind in one non-empty colour block.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub block: usize,
    pub kind: RunKind,
    pub points: Vec<(f64, f64)>,
}

/// Splits the stream into runs, into sewn and travel runs: a stitch right after a
/// trim or colour change is travel (the thread was cut), and block numbers skip empty blocks.
pub fn runs(pattern: &StitchPattern) -> Vec<Run> {
    let stitches = &pattern.stitches;
    let mut raw = Vec::with_capacity(stitches.len());
    let mut non_empty = std::collections::BTreeSet::new();
    let mut current = 0usize;
    for s in stitches {
        raw.push(current);
        match s.command {
            Command::ColorChange => current += 1,
            Command::End => {}
            _ => {
                non_empty.insert(current);
            }
        }
    }
    let compact: std::collections::HashMap<usize, usize> =
        non_empty.iter().enumerate().map(|(i, b)| (*b, i)).collect();

    let mut runs: Vec<Run> = Vec::new();
    let mut last: Option<(f64, f64)> = None;
    let mut thread_cut = true; // nothing is attached before the first stitch
    for (i, s) in stitches.iter().enumerate() {
        match s.command {
            Command::Stitch | Command::Jump => {
                let p = (s.x, s.y);
                let kind = if s.command == Command::Stitch && !thread_cut {
                    RunKind::Sewn
                } else {
                    RunKind::Travel
                };
                if let (Some(from), Some(&block)) = (last, compact.get(&raw[i])) {
                    match runs.last_mut() {
                        Some(run)
                            if run.block == block
                                && run.kind == kind
                                && run.points.last() == Some(&from) =>
                        {
                            run.points.push(p);
                        }
                        _ => runs.push(Run {
                            block,
                            kind,
                            points: vec![from, p],
                        }),
                    }
                }
                if s.command == Command::Stitch {
                    thread_cut = false;
                }
                last = Some(p);
            }
            Command::Trim | Command::ColorChange => thread_cut = true,
            Command::Stop | Command::End => {}
        }
    }
    runs
}

/// PNG bytes whose longest side is `max_pixels`.
pub fn png(
    design: &Design,
    pattern: &StitchPattern,
    max_pixels: u32,
    show_travel: bool,
) -> Result<Vec<u8>, String> {
    let (mut min_x, mut min_y, mut max_x, mut max_y) =
        (0.0, 0.0, design.page.width, design.page.height);
    if pattern.count(Command::Stitch) > 0 {
        let b = pattern.bounds();
        min_x = f64::min(min_x, b.min_x);
        min_y = f64::min(min_y, b.min_y);
        max_x = f64::max(max_x, b.max_x);
        max_y = f64::max(max_y, b.max_y);
    }
    let margin = (max_x - min_x).max(max_y - min_y) * 0.02;
    let (min_x, min_y) = (min_x - margin, min_y - margin);
    let (frame_w, frame_h) = (max_x + margin - min_x, max_y + margin - min_y);
    let scale = f64::from(max_pixels) / frame_w.max(frame_h).max(1.0);
    let width = ((frame_w * scale).round() as u32).max(1);
    let height = ((frame_h * scale).round() as u32).max(1);
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| format!("cannot create a {width}x{height} bitmap"))?;
    pixmap.fill(Color::WHITE);

    // Design coordinates are 0.1 mm with +y down, like the bitmap.
    let transform = Transform::from_row(
        scale as f32,
        0.0,
        0.0,
        scale as f32,
        (-min_x * scale) as f32,
        (-min_y * scale) as f32,
    );
    let hairline = (1.0 / scale) as f32;
    let mut paint = Paint::default();

    if let Some(page) = Rect::from_xywh(
        0.0,
        0.0,
        design.page.width as f32,
        design.page.height as f32,
    ) {
        paint.set_color_rgba8(191, 191, 191, 255);
        let stroke = Stroke {
            width: hairline,
            ..Stroke::default()
        };
        pixmap.stroke_path(
            &PathBuilder::from_rect(page),
            &paint,
            &stroke,
            transform,
            None,
        );
    }

    let threads: Vec<_> = pattern.color_blocks().into_iter().map(|(t, _)| t).collect();
    let thread_width = (1.2 / scale).max(4.0) as f32; // ~0.4 mm, at least 1.2 px
    let sewn = Stroke {
        width: thread_width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    let travel = Stroke {
        width: hairline,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        dash: StrokeDash::new(vec![4.0 * hairline, 3.0 * hairline], 0.0),
        ..Stroke::default()
    };
    for run in runs(pattern) {
        if run.kind == RunKind::Travel && !show_travel {
            continue;
        }
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
        let stroke = match run.kind {
            RunKind::Sewn => {
                let t = threads.get(run.block).copied().flatten();
                paint.set_color_rgba8(
                    t.map_or(0, |t| t.red),
                    t.map_or(0, |t| t.green),
                    t.map_or(0, |t| t.blue),
                    255,
                );
                &sewn
            }
            RunKind::Travel => {
                paint.set_color_rgba8(140, 140, 140, 255);
                &travel
            }
        };
        pixmap.stroke_path(&path, &paint, stroke, transform, None);
    }
    encode(&pixmap)
}

/// Opaque RGB PNG (the background is opaque, so premultiplied RGBA equals straight RGB).
fn encode(pixmap: &Pixmap) -> Result<Vec<u8>, String> {
    let rgb: Vec<u8> = pixmap
        .data()
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    writer
        .write_image_data(&rgb)
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    Ok(out)
}
