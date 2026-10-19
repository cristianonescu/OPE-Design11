//! Render model: a compiled [`StitchPattern`] split into drawable strokes per colour block, cut
//! off at the sewing simulator's position.

use openstitch_core::{Command, StitchPattern};
use openstitch_design::{Vec2, v2};

/// Fallback when a pattern has fewer threads than colour blocks.
pub const UNKNOWN_THREAD: [u8; 3] = [0x80, 0x80, 0x80];

/// One colour block: polylines of consecutive needle penetrations.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorLayer {
    pub rgb: [u8; 3],
    pub strokes: Vec<Vec<Vec2>>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RenderModel {
    /// In sewing order; later layers draw on top.
    pub layers: Vec<ColorLayer>,
    /// Jumps (travel without sewing), drawn dimmed.
    pub travel: Vec<(Vec2, Vec2)>,
    /// Needle position after the last shown stitch.
    pub needle: Option<Vec2>,
    /// Stitches (penetrations) shown.
    pub shown: usize,
}

impl RenderModel {
    /// Builds strokes for the first `limit` stitches (`usize::MAX` for all). A stitch sews from
    /// the previous position; a jump moves without sewing; trims and colour changes break strokes.
    pub fn build(pattern: &StitchPattern, limit: usize) -> RenderModel {
        let rgb = |i: usize| {
            pattern
                .threads
                .get(i)
                .map_or(UNKNOWN_THREAD, |t| [t.red, t.green, t.blue])
        };
        let mut m = RenderModel::default();
        let mut color = 0;
        let mut layer = ColorLayer {
            rgb: rgb(0),
            strokes: Vec::new(),
        };
        let mut stroke: Vec<Vec2> = Vec::new();
        let mut pos: Option<Vec2> = None;
        for s in &pattern.stitches {
            if m.shown >= limit {
                break;
            }
            let p = v2(s.x, s.y);
            match s.command {
                Command::Stitch => {
                    if stroke.is_empty() {
                        stroke.push(pos.unwrap_or(p));
                    }
                    stroke.push(p);
                    pos = Some(p);
                    m.shown += 1;
                }
                Command::Jump => {
                    flush(&mut layer, &mut stroke);
                    if let Some(from) = pos.filter(|&from| from != p) {
                        m.travel.push((from, p));
                    }
                    pos = Some(p);
                }
                Command::Trim | Command::Stop => flush(&mut layer, &mut stroke),
                Command::ColorChange => {
                    flush(&mut layer, &mut stroke);
                    color += 1;
                    let next = ColorLayer {
                        rgb: rgb(color),
                        strokes: Vec::new(),
                    };
                    let done = std::mem::replace(&mut layer, next);
                    if !done.strokes.is_empty() {
                        m.layers.push(done);
                    }
                }
                Command::End => break,
            }
        }
        flush(&mut layer, &mut stroke);
        if !layer.strokes.is_empty() {
            m.layers.push(layer);
        }
        m.needle = pos;
        m
    }
}

fn flush(layer: &mut ColorLayer, stroke: &mut Vec<Vec2>) {
    if stroke.len() >= 2 {
        layer.strokes.push(std::mem::take(stroke));
    } else {
        stroke.clear();
    }
}

/// Statistics shown under the simulator, matching the engine's `design.generate` result.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PatternStats {
    pub stitches: usize,
    pub jumps: usize,
    pub trims: usize,
    pub color_changes: usize,
    pub width_mm: f64,
    pub height_mm: f64,
    pub colors: Vec<[u8; 3]>,
}

impl PatternStats {
    pub fn of(p: &StitchPattern) -> PatternStats {
        let b = p.bounds();
        PatternStats {
            stitches: p.count(Command::Stitch),
            jumps: p.count(Command::Jump),
            trims: p.count(Command::Trim),
            color_changes: p.count(Command::ColorChange),
            width_mm: (b.width()).round() / 10.0,
            height_mm: (b.height()).round() / 10.0,
            colors: p.threads.iter().map(|t| [t.red, t.green, t.blue]).collect(),
        }
    }
}

/// 0-based colour block that the `n`-th stitch (1-based) belongs to, and the block count.
pub fn color_at(p: &StitchPattern, n: usize) -> (usize, usize) {
    let blocks = p.color_block_count();
    let mut seen = 0;
    let mut color = 0;
    for s in &p.stitches {
        match s.command {
            Command::Stitch => {
                seen += 1;
                if seen >= n {
                    break;
                }
            }
            Command::ColorChange => color += 1,
            _ => {}
        }
    }
    (color.min(blocks.saturating_sub(1)), blocks)
}
