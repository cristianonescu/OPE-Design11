//! Point tool on a stitch pattern: needle points (the pattern's stitch and jump entries) are
//! selected by click, Shift+click or a marquee, dragged as one `stitches.movePoints`, deleted
//! with Delete (`stitches.deletePoints`), and a double-click on a stitch line inserts a point
//! there (`stitches.insertPoint`). Indexes are positions in the pattern's stitch list.

use super::{
    DRAG_THRESHOLD, Gesture, PointerEvent, ToolContext, ToolController, ToolEffect, ToolPreview,
    move_delta,
};
use crate::editor::hit::{object_at, rect_from};
use crate::editor::request::{CommandRequest, mm};
use crate::editor::tools::Phase;
use openstitch_design::stitch_edit::is_point;
use openstitch_design::{DesignObject, Vec2, v2};
use serde_json::json;
use std::collections::BTreeSet;

/// What the canvas draws for the needle points of the Point tool's stitch pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct NeedleOverlay {
    pub id: u64,
    /// (index, position as shown, selected, jump end).
    pub points: Vec<(usize, Vec2, bool, bool)>,
}

/// Needle points of `o` as (index, position, jump end).
pub fn needle_points(o: &DesignObject) -> Vec<(usize, Vec2, bool)> {
    o.stitch_block
        .iter()
        .flat_map(|b| b.stitches.iter().enumerate())
        .filter(|(_, s)| is_point(s.command))
        .map(|(i, s)| (i, v2(s.x, s.y), s.command == openstitch_core::Command::Jump))
        .collect()
}

/// The needle point nearest `p` within `radius` (later points win ties, as they are drawn on
/// top).
pub fn needle_at(o: &DesignObject, p: Vec2, radius: f64) -> Option<usize> {
    needle_points(o)
        .into_iter()
        .filter(|(_, q, _)| q.distance(p) <= radius)
        .min_by(|a, b| {
            a.1.distance(p)
                .total_cmp(&b.1.distance(p))
                .then(b.0.cmp(&a.0))
        })
        .map(|(i, ..)| i)
}

/// The stitch line nearest `p` within `tolerance`: (index of the point it runs into, the
/// nearest position on it).
pub fn line_at(o: &DesignObject, p: Vec2, tolerance: f64) -> Option<(usize, Vec2)> {
    let pts = needle_points(o);
    pts.windows(2)
        .filter(|w| !w[1].2)
        .map(|w| {
            let (a, b) = (w[0].1, w[1].1);
            let d = b - a;
            let len2 = d.x * d.x + d.y * d.y;
            let t = if len2 > 0.0 {
                (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (w[1].0, a + d * t)
        })
        .filter(|(_, q)| q.distance(p) <= tolerance)
        .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
}

impl ToolContext<'_> {
    /// The stitch pattern the Point tool edits: the selection when it is exactly one visible
    /// stitch pattern.
    pub fn needle_target(&self) -> Option<&DesignObject> {
        if self.selection.len() != 1 {
            return None;
        }
        let id = *self.selection.first()?;
        self.design.objects.iter().find(|o| {
            o.id == id
                && o.kind == openstitch_design::ObjectKind::Stitches
                && o.visible
                && o.stitch_block.is_some()
        })
    }
}

impl ToolController {
    /// Selected needle points of the Point tool's stitch pattern.
    pub fn selected_needles(&self, ctx: &ToolContext) -> BTreeSet<usize> {
        match ctx.needle_target() {
            Some(o) if self.needle_owner == Some(o.id) => self.needles.clone(),
            _ => BTreeSet::new(),
        }
    }

    pub fn needle_overlay(&self, ctx: &ToolContext) -> Option<NeedleOverlay> {
        if self.tool() != super::Tool::Point {
            return None;
        }
        let o = ctx.needle_target()?;
        let selected = self.selected_needles(ctx);
        let delta = match &self.gesture {
            Some(Gesture::Needles { delta: Some(d), .. }) => *d,
            _ => v2(0.0, 0.0),
        };
        Some(NeedleOverlay {
            id: o.id,
            points: needle_points(o)
                .into_iter()
                .map(|(i, p, jump)| {
                    let s = selected.contains(&i);
                    (i, if s { p + delta } else { p }, s, jump)
                })
                .collect(),
        })
    }

    pub(super) fn needle_tool(
        &mut self,
        e: PointerEvent,
        ctx: &ToolContext,
        o: &DesignObject,
    ) -> Vec<ToolEffect> {
        let p = e.location;
        if self.needle_owner != Some(o.id) {
            self.needles.clear();
            self.needle_owner = Some(o.id);
        }
        match (e.phase, self.gesture.clone()) {
            (Phase::Down, _) => {
                if let Some(i) = needle_at(o, p, ctx.point_radius()) {
                    if e.shift {
                        if !self.needles.remove(&i) {
                            self.needles.insert(i);
                        }
                    } else if !self.needles.contains(&i) {
                        self.needles = BTreeSet::from([i]);
                    }
                    self.gesture = Some(Gesture::Needles {
                        start: p,
                        delta: None,
                        marquee: false,
                    });
                    return vec![];
                }
                if e.clicks >= 2
                    && let Some((i, at)) = line_at(o, p, ctx.tolerance())
                {
                    self.needles = BTreeSet::from([i]);
                    return vec![ToolEffect::run(
                        CommandRequest::new(
                            "stitches.insertPoint",
                            json!({ "id": o.id, "index": i, "x": mm(at.x), "y": mm(at.y) }),
                        ),
                        false,
                    )];
                }
                let inside = o.bounds().is_some_and(|b| {
                    p.x >= b.min_x && p.x <= b.max_x && p.y >= b.min_y && p.y <= b.max_y
                });
                if inside {
                    if !e.shift {
                        self.needles.clear();
                    }
                    self.gesture = Some(Gesture::Needles {
                        start: p,
                        delta: None,
                        marquee: true,
                    });
                    return vec![];
                }
                self.needles.clear();
                match object_at(ctx.design, p, ctx.tolerance()) {
                    Some(id) if id == o.id => vec![],
                    Some(id) => vec![ToolEffect::Select(BTreeSet::from([id]))],
                    None => vec![ToolEffect::Select(BTreeSet::new())],
                }
            }
            (Phase::Dragged, Some(Gesture::Needles { start, marquee, .. })) => {
                let moved = p.distance(start) >= DRAG_THRESHOLD * ctx.units_per_point;
                if marquee {
                    self.preview = moved.then(|| ToolPreview::Marquee(rect_from(start, p)));
                    return vec![];
                }
                let delta = moved.then(|| move_delta(start, p, e.shift));
                self.gesture = Some(Gesture::Needles {
                    start,
                    delta,
                    marquee,
                });
                vec![]
            }
            (
                Phase::Up,
                Some(Gesture::Needles {
                    start,
                    delta,
                    marquee,
                }),
            ) => {
                self.gesture = None;
                self.preview = None;
                if marquee {
                    if p.distance(start) >= DRAG_THRESHOLD * ctx.units_per_point {
                        let r = rect_from(start, p);
                        self.needles.extend(
                            needle_points(o)
                                .into_iter()
                                .filter(|(_, q, _)| {
                                    q.x >= r.min_x
                                        && q.x <= r.max_x
                                        && q.y >= r.min_y
                                        && q.y <= r.max_y
                                })
                                .map(|(i, ..)| i),
                        );
                    }
                    return vec![];
                }
                match delta {
                    Some(d) if !self.needles.is_empty() => vec![ToolEffect::run(
                        CommandRequest::new(
                            "stitches.movePoints",
                            json!({ "id": o.id, "indexes": self.needles.iter().collect::<Vec<_>>(), "dx": mm(d.x), "dy": mm(d.y) }),
                        ),
                        false,
                    )],
                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }

    /// Delete with the Point tool on a stitch pattern: removes the selected needle points.
    pub(super) fn delete_needles(&mut self, ctx: &ToolContext) -> Option<Vec<ToolEffect>> {
        let o = ctx.needle_target()?;
        let selected = self.selected_needles(ctx);
        if selected.is_empty() {
            return None;
        }
        self.needles.clear();
        Some(vec![ToolEffect::run(
            CommandRequest::new(
                "stitches.deletePoints",
                json!({ "id": o.id, "indexes": selected.into_iter().collect::<Vec<_>>() }),
            ),
            false,
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Tool;
    use openstitch_engine::Session;

    fn pattern() -> Session {
        let mut s = Session::new(std::env::temp_dir());
        s.execute(
            "shape.rectangle",
            Some(&json!({"x": 10, "y": 10, "width": 10, "height": 5})),
        )
        .expect("rect");
        s.execute("object.toStitches", Some(&json!({"id": 1})))
            .expect("convert");
        s
    }

    fn ev(phase: Phase, at: Vec2) -> PointerEvent {
        PointerEvent::new(phase, at)
    }

    fn requests(effects: &[ToolEffect]) -> Vec<CommandRequest> {
        effects
            .iter()
            .flat_map(|e| match e {
                ToolEffect::Execute { requests, .. } => requests.clone(),
                ToolEffect::Select(_) => vec![],
            })
            .collect()
    }

    #[test]
    fn select_drag_delete_insert_and_marquee() {
        let mut s = pattern();
        let selection = BTreeSet::from([1]);
        let design = s.design().clone();
        let ctx = ToolContext {
            design: &design,
            selection: &selection,
            units_per_point: 0.5,
            scale_takes_origin: true,
        };
        let o = ctx.needle_target().expect("stitch pattern");
        let pts = needle_points(o);
        let (i, p, _) = pts[pts.len() / 2];
        let mut t = ToolController::default();
        t.set_tool(Tool::Point);

        // Click selects; drag moves the selection by one command.
        assert!(t.handle(ev(Phase::Down, p), &ctx).is_empty());
        assert_eq!(t.selected_needles(&ctx), BTreeSet::from([i]));
        t.handle(ev(Phase::Dragged, p + v2(10.0, 0.0)), &ctx);
        let overlay = t.needle_overlay(&ctx).expect("overlay");
        assert!(
            overlay
                .points
                .iter()
                .any(|q| q.0 == i && q.2 && q.1 == p + v2(10.0, 0.0))
        );
        let r = requests(&t.handle(ev(Phase::Up, p + v2(10.0, 0.0)), &ctx));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].command, "stitches.movePoints");
        assert_eq!(r[0].params["indexes"], json!([i]));
        assert_eq!(r[0].params["dx"], json!(1.0));
        s.execute(r[0].command, Some(&r[0].params)).expect("moves");

        // Delete removes the selected point.
        let r = requests(&t.delete_point(&ctx).expect("needles selected"));
        assert_eq!(r[0].command, "stitches.deletePoints");
        assert!(t.selected_needles(&ctx).is_empty());

        // Double-click on a stitch line inserts a point there: the longest stitch before the
        // deleted point (the fill starts with short tie-in steps).
        let k = (0..i.saturating_sub(1))
            .max_by(|&x, &y| {
                let len = |k: usize| pts[k].1.distance(pts[k + 1].1);
                len(x).total_cmp(&len(y))
            })
            .expect("stitches before the deleted point");
        let (_, a, _) = pts[k];
        let (j, b, _) = pts[k + 1];
        let mid = a.lerp(b, 0.5);
        assert!(a.distance(b) > 4.0, "a line long enough to click");
        let mut e = ev(Phase::Down, mid);
        e.clicks = 2;
        let r = requests(&t.handle(e, &ctx));
        assert_eq!(r[0].command, "stitches.insertPoint");
        assert_eq!(r[0].params["index"], json!(j));
        t.handle(ev(Phase::Up, mid), &ctx);

        // A marquee inside the pattern selects every point in it.
        // Start inside the bounds away from needle points (a single press on a line starts a
        // marquee too).
        let b = o.bounds().expect("bounds");
        let start = (1..20)
            .flat_map(|i| (1..20).map(move |j| (i, j)))
            .map(|(i, j)| {
                v2(
                    b.min_x + b.width() * f64::from(i) / 20.0,
                    b.min_y + b.height() * f64::from(j) / 20.0,
                )
            })
            .find(|&q| needle_at(o, q, ctx.point_radius()).is_none())
            .expect("an empty spot");
        t.handle(ev(Phase::Down, start), &ctx);
        t.handle(ev(Phase::Dragged, v2(b.max_x + 5.0, b.max_y + 5.0)), &ctx);
        t.handle(ev(Phase::Up, v2(b.max_x + 5.0, b.max_y + 5.0)), &ctx);
        let inside = pts
            .iter()
            .filter(|q| q.1.x >= start.x && q.1.y >= start.y)
            .count();
        assert!(inside > 0);
        assert_eq!(t.selected_needles(&ctx).len(), inside);
    }
}
