//! Contour (offset) fill, beyond PE-DESIGN (backlog item B1; sketch in
//! `docs/research/beyond/contour-fill.md`). The region is filled with copies of its own outline
//! moved inward by a constant distance, the spacing: the first ring half a spacing inside the
//! outline, then one every spacing until the middle. Holes (the object's own and paired hole
//! sewing) grow rings of their own; an inset that splits (a U shape becomes two arms) sews one
//! branch whole before the next. The ring tree, the blend between rings and the sampling are
//! PE-DESIGN's Spiral Stitch's (see [`crate::spiral`]); this module adds what PE-DESIGN lacks:
//!
//! - **Rings**: every ring a closed running-stitch loop of its own (echo quilting), each
//!   loop's start moved on by a golden-ratio fraction of a stitch so the needle points of
//!   neighbouring rings do not line up.
//! - **Single spiral**: one continuous path per branch, as PE-DESIGN's Spiral but half a
//!   spacing inside the outline.
//! - **Double spiral**: inwards over every other ring and back out over the rest, so the path
//!   ends next to where it began (useful when the next object continues from the border).
//! - **Echo lines**: up to 30 rings *outside* the shape, the outline moved outward 1, 2, …
//!   spacings, sewn first from the outermost in, so the needle works into the shape.
//! - **Join**: corners of the offset rings rounded or mitred (matters for echo rings and for
//!   inward corners).
//!
//! Stitches are running stitches no longer than the run pitch. Lengths are 0.1 mm; output is
//! deterministic.

use crate::boolean::{self, BoolOp};
use crate::geometry::{Contour, Vec2};
use crate::model::ContourStrategy;
use crate::spiral::{Mode, Tree, is_clockwise, loop_from};

/// Most echo rings outside the shape.
pub const MAX_ECHO_LINES: u32 = 30;

/// What the generator needs from the region's attributes.
#[derive(Clone, Copy, Debug)]
pub struct ContourParams {
    /// Distance between rings, 0.1 mm.
    pub spacing: f64,
    /// Running stitch length, 0.1 mm.
    pub pitch: f64,
    pub strategy: ContourStrategy,
    /// Round corners of the offset rings (else mitred).
    pub rounded: bool,
    /// Rings outside the shape, 0–30.
    pub echo_lines: u32,
    /// Rings and turns run counter-clockwise on screen (else clockwise).
    pub counterclockwise: bool,
    /// Sew from the middle out instead of from the outside in.
    pub start_inside: bool,
    /// Where the needle comes in (R1), if set.
    pub entry: Option<Vec2>,
}

/// The contour fill of `closed` minus `holes`: needle-point paths in sewing order, no stitch
/// longer than `p.pitch`. The caller joins them (travel inside the region, or jumps).
pub fn contour(closed: &[Contour], holes: &[Contour], p: &ContourParams) -> Vec<Vec<Vec2>> {
    if !(p.spacing > 0.0 && p.pitch > 0.0) {
        return Vec::new();
    }
    let base: Vec<Contour> = if holes.is_empty() {
        closed.to_vec()
    } else {
        boolean::contours(&boolean::combine(closed, holes, BoolOp::Difference))
    };
    let clockwise = !p.counterclockwise;
    let echo = echo(&base, p, clockwise);
    let tree = Tree::build(&base, p.spacing, p.pitch, 0.5, p.rounded, clockwise);
    let mode = match p.strategy {
        ContourStrategy::Rings => Mode::Rings,
        ContourStrategy::SingleSpiral => Mode::Spiral,
        ContourStrategy::DoubleSpiral => Mode::DoubleSpiral,
    };
    let mut pieces = tree.sew(p.pitch, echo.last().copied().or(p.entry), mode);
    if !echo.is_empty() {
        // The echo rings lie outside the region, where no travel can be routed: they lead
        // straight into the first ring.
        match pieces.first_mut() {
            Some(first) => {
                let mut joined = echo;
                joined.extend(first.iter().copied());
                *first = crate::generate::subdivide(&joined, p.pitch);
            }
            None => pieces.push(echo),
        }
    }
    if p.start_inside {
        pieces.reverse();
        pieces.iter_mut().for_each(|piece| piece.reverse());
    }
    pieces
}

/// The echo rings, outermost first, each once round from the point nearest the needle and
/// leading straight into the next; one path.
fn echo(base: &[Contour], p: &ContourParams, clockwise: bool) -> Vec<Vec2> {
    let n = p.echo_lines.min(MAX_ECHO_LINES);
    let mut out: Vec<Vec2> = Vec::new();
    let mut pos = p.entry;
    for k in (1..=n).rev() {
        let regions = boolean::offset(base, k as f64 * p.spacing, p.rounded);
        for region in &regions {
            let Some(outer) = region.first() else {
                continue;
            };
            let mut points = outer.outline_points();
            if is_clockwise(&points) != clockwise {
                points.reverse();
            }
            let here = pos
                .or_else(|| out.last().copied())
                .unwrap_or_else(|| lowest(&points));
            let ring = loop_from(&points, here, p.pitch);
            if let Some(&end) = ring.last() {
                pos = Some(end);
            }
            out.extend(ring);
        }
    }
    crate::generate::subdivide(&out, p.pitch)
}

/// The lowest point on screen (largest y), leftmost among equals.
fn lowest(points: &[Vec2]) -> Vec2 {
    points
        .iter()
        .copied()
        .max_by(|a, b| a.y.total_cmp(&b.y).then(b.x.total_cmp(&a.x)))
        .unwrap_or_default()
}
