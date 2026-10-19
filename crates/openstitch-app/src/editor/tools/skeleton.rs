//! Satin Skeleton tool: draw the centre line of a satin column like the Bézier pen (click for
//! corners, drag for smooth points), each point carrying a column width shown as a handle on
//! either side of the line; drag a width handle to change that point's width. Return or a
//! double-click ends the line as one `shape.skeletonSatin`. With nothing being drawn, the
//! selected object's stored skeleton shows the same handles: dragging a width handle or a
//! centre point is one `object.setSkeleton`.

use super::{
    DRAG_THRESHOLD, Gesture, Phase, PointerEvent, Tool, ToolContext, ToolController, ToolEffect,
    ToolPreview,
};
use crate::editor::pen::{PenNode, pen_contour};
use crate::editor::request::{CommandRequest, mm};
use openstitch_design::skeleton::{self, DEFAULT_WIDTH, MAX_WIDTH, Skeleton};
use openstitch_design::{Contour, DesignObject, Vec2, v2};
use serde_json::json;

/// What the canvas draws while the Satin Skeleton tool is active.
#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonOverlay {
    /// The object whose stored skeleton is shown; `None` for the one being drawn.
    pub id: Option<u64>,
    /// Centre line, curves flattened.
    pub centre: Contour,
    /// The column's outline.
    pub column: Option<Contour>,
    /// Points with a width handle: (point, left handle, right handle).
    pub widths: Vec<(Vec2, Vec2, Vec2)>,
}

/// The centre line of `nodes` with curves flattened, each flattened point's width interpolated
/// along its segment between the widths of its anchors (mm).
pub fn flatten(nodes: &[PenNode], widths: &[f64]) -> Skeleton {
    let width = |i: usize| {
        widths
            .get(i)
            .or(widths.last())
            .copied()
            .unwrap_or(DEFAULT_WIDTH)
    };
    let c = pen_contour(nodes, false);
    let mut out = Skeleton {
        points: c.points.first().copied().into_iter().collect(),
        widths: if nodes.is_empty() {
            vec![]
        } else {
            vec![width(0)]
        },
    };
    for i in 0..c.segment_count() {
        let poly = c.segment_polyline(i);
        let total: f64 = poly.windows(2).map(|w| w[0].distance(w[1])).sum();
        let mut at = 0.0;
        for w in poly.windows(2) {
            at += w[0].distance(w[1]);
            let f = if total > 0.0 { at / total } else { 1.0 };
            out.points.push(w[1]);
            out.widths.push(width(i) + (width(i + 1) - width(i)) * f);
        }
    }
    out
}

/// Unit left normal (the side [`skeleton::satin`] starts on) of the line through `points` at
/// point `i`, from its neighbours.
fn normal(points: &[Vec2], i: usize) -> Option<Vec2> {
    let t = points[(i + 1).min(points.len() - 1)] - points[i.saturating_sub(1)];
    (t.length() > 1e-9).then(|| v2(t.y, -t.x) * (1.0 / t.length()))
}

/// Width handles of `points` / `widths` (mm): (point, left handle, right handle).
fn width_handles(points: &[Vec2], widths: &[f64]) -> Vec<(Vec2, Vec2, Vec2)> {
    (0..points.len())
        .filter_map(|i| {
            let n = normal(points, i)?;
            let half = widths.get(i).copied().unwrap_or(0.0) * 5.0;
            Some((points[i], points[i] + n * half, points[i] - n * half))
        })
        .collect()
}

/// The width handle of `points` nearest `p` within `radius`.
fn width_handle_at(points: &[Vec2], widths: &[f64], p: Vec2, radius: f64) -> Option<usize> {
    (0..points.len())
        .filter_map(|i| {
            let n = normal(points, i)?;
            let half = widths.get(i).copied().unwrap_or(0.0) * 5.0;
            let d = (points[i] + n * half)
                .distance(p)
                .min((points[i] - n * half).distance(p));
            (d <= radius).then_some((i, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The width (mm, 0.1 mm steps) a width handle of `points[i]` dragged to `p` gives.
fn width_from(points: &[Vec2], i: usize, p: Vec2) -> Option<f64> {
    let n = normal(points, i)?;
    let d = p - points[i];
    let w = 2.0 * (d.x * n.x + d.y * n.y).abs() / 10.0;
    Some(((w * 10.0).round() / 10.0).clamp(0.0, MAX_WIDTH))
}

/// Whether `o`'s outline is the column of its skeleton (made by the tool or by `reshape`), so
/// an edit should reshape it too.
fn follows_column(o: &DesignObject, sk: &Skeleton) -> bool {
    let (Some(column), [c]) = (skeleton::outline(sk), o.contours.as_slice()) else {
        return false;
    };
    column.points.len() == c.points.len()
        && column
            .points
            .iter()
            .zip(&c.points)
            .all(|(a, b)| a.distance(*b) < 1.0)
}

fn skeleton_params(sk: &Skeleton) -> (serde_json::Value, serde_json::Value) {
    (
        sk.points
            .iter()
            .map(|p| json!([mm(p.x), mm(p.y)]))
            .collect(),
        sk.widths
            .iter()
            .map(|w| json!((w * 100.0).round() / 100.0))
            .collect(),
    )
}

impl ToolContext<'_> {
    /// The object whose skeleton the Satin Skeleton tool edits: the selection when it is
    /// exactly one visible object sewn along a skeleton.
    pub fn skeleton_target(&self) -> Option<(&DesignObject, &Skeleton)> {
        if self.selection.len() != 1 {
            return None;
        }
        let id = *self.selection.first()?;
        let o = self
            .design
            .objects
            .iter()
            .find(|o| o.id == id && o.visible)?;
        Some((o, o.region.as_ref()?.skeleton.as_ref()?))
    }
}

impl ToolController {
    /// Column width (mm) of the next points the Satin Skeleton tool places.
    pub fn skeleton_width(&self) -> f64 {
        self.skeleton_width.unwrap_or(DEFAULT_WIDTH)
    }

    pub fn set_skeleton_width(&mut self, mm: f64) {
        self.skeleton_width = Some(if mm.is_finite() {
            mm.clamp(0.1, MAX_WIDTH)
        } else {
            DEFAULT_WIDTH
        });
    }

    /// Widths (mm) of the anchors being drawn, as edited by a width drag in progress.
    fn drawn_widths(&self) -> Vec<f64> {
        let mut w = self.widths.clone();
        if let Some(Gesture::SkeletonWidth {
            index,
            stored: None,
            edited: Some(e),
        }) = &self.gesture
            && let Some(x) = w.get_mut(*index)
        {
            *x = *e;
        }
        w
    }

    /// The anchors being drawn.
    fn anchor_points(&self) -> Vec<Vec2> {
        self.pen.iter().map(|n| n.at).collect()
    }

    /// The stored skeleton of the target, with the drag in progress applied.
    fn edited_stored(&self, id: u64, sk: &Skeleton) -> Skeleton {
        let mut sk = sk.clone();
        match &self.gesture {
            Some(Gesture::SkeletonWidth {
                index,
                stored: Some(s),
                edited: Some(w),
            }) if *s == id && *index < sk.widths.len() => sk.widths[*index] = *w,
            Some(Gesture::SkeletonPoint {
                id: s,
                index,
                edited: Some(q),
                ..
            }) if *s == id && *index < sk.points.len() => sk.points[*index] = *q,
            _ => {}
        }
        sk
    }

    /// The centre line, column and width handles to draw while the Satin Skeleton tool is
    /// active: the line being drawn, else the selected object's stored skeleton.
    pub fn skeleton_overlay(&self, ctx: &ToolContext) -> Option<SkeletonOverlay> {
        if self.tool() != Tool::Skeleton {
            return None;
        }
        if !self.pen.is_empty() {
            let widths = self.drawn_widths();
            let flat = flatten(&self.pen, &widths);
            return Some(SkeletonOverlay {
                id: None,
                centre: Contour::new(flat.points.clone(), false),
                column: skeleton::outline(&flat),
                widths: width_handles(&self.anchor_points(), &widths),
            });
        }
        let (o, sk) = ctx.skeleton_target()?;
        let sk = self.edited_stored(o.id, sk);
        Some(SkeletonOverlay {
            id: Some(o.id),
            centre: Contour::new(sk.points.clone(), false),
            column: skeleton::outline(&sk),
            widths: width_handles(&sk.points, &sk.widths),
        })
    }

    fn skeleton_preview(&mut self, cursor: Option<Vec2>) {
        let mut nodes = self.pen.clone();
        nodes.extend(cursor.map(PenNode::corner));
        self.preview =
            (!nodes.is_empty()).then(|| ToolPreview::Outline(pen_contour(&nodes, false)));
    }

    pub(super) fn skeleton_tool(&mut self, e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        let p = e.location;
        let radius = ctx.point_radius();
        match (e.phase, self.gesture.clone()) {
            (Phase::Down, _) => {
                let drawn = self.anchor_points();
                if let Some(index) = width_handle_at(&drawn, &self.widths, p, radius) {
                    self.gesture = Some(Gesture::SkeletonWidth {
                        index,
                        stored: None,
                        edited: None,
                    });
                    return vec![];
                }
                if self.pen.is_empty()
                    && let Some((o, sk)) = ctx.skeleton_target()
                {
                    if let Some(index) = width_handle_at(&sk.points, &sk.widths, p, radius) {
                        self.gesture = Some(Gesture::SkeletonWidth {
                            index,
                            stored: Some(o.id),
                            edited: None,
                        });
                        return vec![];
                    }
                    if let Some(index) = (0..sk.points.len())
                        .filter(|&i| sk.points[i].distance(p) <= radius)
                        .min_by(|&a, &b| {
                            sk.points[a]
                                .distance(p)
                                .total_cmp(&sk.points[b].distance(p))
                        })
                    {
                        self.gesture = Some(Gesture::SkeletonPoint {
                            id: o.id,
                            index,
                            start: p,
                            edited: None,
                        });
                        return vec![];
                    }
                }
                if e.clicks >= 2 && self.pen.len() >= 2 {
                    return self.finish_skeleton();
                }
                if self
                    .pen
                    .last()
                    .is_some_and(|last| last.at.distance(p) <= ctx.tolerance() / 2.0)
                {
                    return vec![];
                }
                self.pen.push(PenNode::corner(p));
                self.widths.push(self.skeleton_width());
                self.gesture = Some(Gesture::PenAnchor {
                    index: self.pen.len() - 1,
                });
                self.skeleton_preview(None);
                vec![]
            }
            (Phase::Dragged, Some(Gesture::PenAnchor { index })) => {
                if let Some(n) = self.pen.get_mut(index) {
                    n.out = (p.distance(n.at) >= DRAG_THRESHOLD * ctx.units_per_point).then_some(p);
                }
                self.skeleton_preview(None);
                vec![]
            }
            (Phase::Dragged, Some(Gesture::SkeletonWidth { index, stored, .. })) => {
                let edited = match stored {
                    None => width_from(&self.anchor_points(), index, p),
                    Some(id) => ctx
                        .skeleton_target()
                        .filter(|(o, sk)| o.id == id && index < sk.points.len())
                        .and_then(|(_, sk)| width_from(&sk.points, index, p)),
                };
                self.gesture = Some(Gesture::SkeletonWidth {
                    index,
                    stored,
                    edited,
                });
                vec![]
            }
            (
                Phase::Dragged,
                Some(Gesture::SkeletonPoint {
                    id, index, start, ..
                }),
            ) => {
                let edited = (p.distance(start) >= DRAG_THRESHOLD * ctx.units_per_point)
                    .then(|| self.snapped(p));
                self.gesture = Some(Gesture::SkeletonPoint {
                    id,
                    index,
                    start,
                    edited,
                });
                vec![]
            }
            (
                Phase::Up,
                Some(g @ (Gesture::SkeletonWidth { .. } | Gesture::SkeletonPoint { .. })),
            ) => {
                let edits_stored = !matches!(g, Gesture::SkeletonWidth { stored: None, .. });
                let effects = if edits_stored {
                    self.set_stored(ctx)
                } else {
                    if let Gesture::SkeletonWidth {
                        index,
                        edited: Some(w),
                        ..
                    } = g
                        && let Some(x) = self.widths.get_mut(index)
                    {
                        *x = w;
                    }
                    vec![]
                };
                self.gesture = None;
                effects
            }
            (Phase::Up, Some(Gesture::PenAnchor { .. })) => {
                self.gesture = None;
                vec![]
            }
            (Phase::Moved, _) if !self.pen.is_empty() => {
                self.skeleton_preview(Some(p));
                vec![]
            }
            _ => vec![],
        }
    }

    /// `object.setSkeleton` for the stored skeleton as the gesture in progress edits it.
    fn set_stored(&self, ctx: &ToolContext) -> Vec<ToolEffect> {
        let Some((o, sk)) = ctx.skeleton_target() else {
            return vec![];
        };
        let edited = self.edited_stored(o.id, sk);
        if edited == *sk || edited.validate().is_err() {
            return vec![];
        }
        let (points, widths) = skeleton_params(&edited);
        vec![ToolEffect::run(
            CommandRequest::new(
                "object.setSkeleton",
                json!({ "id": o.id, "points": points, "widths": widths, "reshape": follows_column(o, sk) }),
            ),
            false,
        )]
    }

    /// Ends the line being drawn as one `shape.skeletonSatin`; a line too short (or of too
    /// few points) to sew is discarded.
    pub fn finish_skeleton(&mut self) -> Vec<ToolEffect> {
        let nodes = std::mem::take(&mut self.pen);
        let widths = std::mem::take(&mut self.widths);
        self.preview = None;
        self.gesture = None;
        if self.tool() != Tool::Skeleton || nodes.len() < 2 {
            return vec![];
        }
        let flat = flatten(&nodes, &widths);
        if flat.points.len() > skeleton::MAX_POINTS || flat.validate().is_err() {
            return vec![];
        }
        let (points, widths) = skeleton_params(&flat);
        vec![ToolEffect::run(
            CommandRequest::new(
                "shape.skeletonSatin",
                json!({ "points": points, "widths": widths, "name": "Skeleton satin" }),
            ),
            true,
        )]
    }
}
