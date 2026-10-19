//! Pen tool path under construction: anchors with optional symmetric handles.

use super::request::{CommandRequest, mm};
use openstitch_design::{Bezier, Contour, Vec2};
use serde_json::{Value, json};

/// One anchor. `out` is the handle pulled out by dragging; the incoming handle mirrors it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenNode {
    pub at: Vec2,
    pub out: Option<Vec2>,
}

impl PenNode {
    pub fn corner(at: Vec2) -> Self {
        PenNode { at, out: None }
    }

    /// The incoming handle: `out` reflected through the anchor.
    pub fn incoming(&self) -> Option<Vec2> {
        self.out.map(|o| self.at * 2.0 - o)
    }
}

/// Segment from `a` to `b`: a curve when either end has handles, else straight.
fn segment(a: &PenNode, b: &PenNode) -> Option<Bezier> {
    if a.out.is_none() && b.out.is_none() {
        return None;
    }
    Some(Bezier {
        c1: a.out.unwrap_or(a.at),
        c2: b.incoming().unwrap_or(b.at),
    })
}

/// The contour the nodes describe.
pub fn pen_contour(nodes: &[PenNode], closed: bool) -> Contour {
    let mut c = Contour::new(nodes.iter().map(|n| n.at).collect(), closed);
    let n = nodes.len();
    for i in 0..c.segment_count() {
        c.set_curve(i, segment(&nodes[i], &nodes[(i + 1) % n]));
    }
    c
}

/// Whether the nodes make a valid `shape.path`: two points for an open path; a closed path
/// needs three, or two joined by a curve.
pub fn is_valid(nodes: &[PenNode], closed: bool) -> bool {
    match nodes.len() {
        0 | 1 => false,
        2 => !closed || pen_contour(nodes, true).has_curves(),
        _ => true,
    }
}

fn point(p: Vec2) -> Value {
    json!([mm(p.x), mm(p.y)])
}

/// `shape.path` for the nodes, with `curves` only when some segment is curved.
pub fn path_request(nodes: &[PenNode], closed: bool) -> CommandRequest {
    let c = pen_contour(nodes, closed);
    let mut params = json!({
        "points": c.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
        "closed": closed,
        "name": "Path",
    });
    if c.has_curves() {
        params["curves"] = (0..c.segment_count())
            .map(|i| {
                c.curve(i)
                    .map_or(Value::Null, |z| json!([point(z.c1), point(z.c2)]))
            })
            .collect();
    }
    CommandRequest::new("shape.path", params)
}
