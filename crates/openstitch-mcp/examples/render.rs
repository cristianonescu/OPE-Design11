//! Runs a command script (a JSON array of `{"command", "params"}`) through a `Session` and
//! writes the compiled design as a PNG, for documentation evidence.
//!
//! `cargo run -p openstitch-mcp --example render -- examples/line-types.json out.png [pixels]
//! [--needles]`
//!
//! `--script-dir` resolves the script's relative file paths against its own directory (for
//! scripts that read files, such as fonts); otherwise they resolve in the temp directory.
//!
//! `--needles` draws thin thread with every needle point as a dot, so needle placement (for
//! example programmable fill textures) is visible.

use openstitch_design::Compiler;
use openstitch_engine::Session;
use serde_json::Value;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [script, out, rest @ ..] = args.as_slice() else {
        eprintln!("usage: render <script.json> <out.png> [pixels]");
        return ExitCode::FAILURE;
    };
    let pixels = rest.iter().find_map(|p| p.parse().ok()).unwrap_or(1200);
    let needles = rest.iter().any(|a| a == "--needles");
    let steps: Vec<Value> = match std::fs::read(script)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{script}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let base = if rest.iter().any(|a| a == "--script-dir") {
        std::path::Path::new(script)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| ".".into(), std::path::Path::to_path_buf)
    } else {
        std::env::temp_dir()
    };
    let mut session = Session::new(base);
    for step in &steps {
        let id = step["command"].as_str().unwrap_or_default();
        if let Err(e) = session.execute(id, step.get("params")) {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    }
    let pattern = Compiler::default().compile(session.design());
    let image = if needles {
        needle_png(&pattern, pixels)
    } else {
        openstitch_mcp::preview::png(session.design(), &pattern, pixels, false)
    };
    match image.and_then(|png| std::fs::write(out, png).map_err(|e| e.to_string())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Thin thread lines with a dot at every needle point.
fn needle_png(pattern: &openstitch_core::StitchPattern, pixels: u32) -> Result<Vec<u8>, String> {
    use openstitch_core::Command;
    use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
    let b = pattern.bounds();
    let margin = 10.0;
    let (w, h) = (
        b.max_x - b.min_x + 2.0 * margin,
        b.max_y - b.min_y + 2.0 * margin,
    );
    let scale = f64::from(pixels) / w.max(h);
    let (pw, ph) = ((w * scale).round() as u32, (h * scale).round() as u32);
    let mut pixmap = Pixmap::new(pw.max(1), ph.max(1)).ok_or("bitmap")?;
    pixmap.fill(Color::WHITE);
    let map = |x: f64, y: f64| {
        (
            ((x - b.min_x + margin) * scale) as f32,
            ((y - b.min_y + margin) * scale) as f32,
        )
    };
    let mut block = 0;
    let mut prev: Option<(f32, f32)> = None;
    let mut dots = PathBuilder::new();
    for s in &pattern.stitches {
        let p = map(s.x, s.y);
        match s.command {
            Command::Stitch => {
                if let Some(q) = prev {
                    let t = pattern
                        .threads
                        .get(block)
                        .cloned()
                        .unwrap_or(openstitch_core::Thread::rgb(0));
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(t.red, t.green, t.blue, 160);
                    paint.anti_alias = true;
                    let mut line = PathBuilder::new();
                    line.move_to(q.0, q.1);
                    line.line_to(p.0, p.1);
                    if let Some(path) = line.finish() {
                        let stroke = Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
                dots.push_circle(p.0, p.1, 1.4);
                prev = Some(p);
            }
            Command::ColorChange => {
                block += 1;
                prev = None;
            }
            Command::Jump | Command::Trim => prev = None,
            _ => {}
        }
    }
    if let Some(path) = dots.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, pixmap.width(), pixmap.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(pixmap.data())
            .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    Ok(out)
}
