//! Stitch Design Factory editor model: the sewing-stitch pattern being edited, hit-testing and
//! edits of its points (needle points, satin stations, decorative-satin outline, cross cells),
//! and the commands that put it in the project or export it. No egui; `sewing_ui` draws it.
//! Coordinates are mm: x along the feed, y across.

use super::request::CommandRequest;
use openstitch_design::sewing::{MAX_LENGTH, SewingKind, SewingStitch};
use openstitch_design::{Vec2, v2};
use serde_json::{Value, json};

/// Editing grid (mm).
pub const SNAP: f64 = 0.25;

/// Something the pointer can grab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Decorative needle point.
    Point(usize),
    /// Satin station `i`, left (`false`) or right (`true`) edge.
    Edge(usize, bool),
    /// Decorative-satin outline vertex.
    Vertex(usize),
}

/// The pattern being edited.
#[derive(Clone, Debug, PartialEq)]
pub struct SewingEditor {
    pub doc: SewingStitch,
    pub snap: bool,
}

impl Default for SewingEditor {
    fn default() -> Self {
        SewingEditor::new(SewingKind::Decorative)
    }
}

impl SewingEditor {
    pub fn new(kind: SewingKind) -> Self {
        SewingEditor {
            doc: SewingStitch::new(kind),
            snap: true,
        }
    }

    pub fn from_stitch(s: &SewingStitch) -> Self {
        SewingEditor {
            doc: s.clone(),
            snap: true,
        }
    }

    /// `p` snapped (when on) and kept inside the band 0…200 mm × ±width/2.
    pub fn clamp(&self, p: Vec2) -> Vec2 {
        let p = if self.snap {
            v2((p.x / SNAP).round() * SNAP, (p.y / SNAP).round() * SNAP)
        } else {
            p
        };
        let half = self.doc.width / 2.0;
        v2(p.x.clamp(0.0, MAX_LENGTH), p.y.clamp(-half, half))
    }

    /// The handle within `tolerance` mm of `p`, nearest first.
    pub fn hit(&self, p: Vec2, tolerance: f64) -> Option<Handle> {
        let d = &self.doc;
        let candidates: Vec<(Handle, Vec2)> = match d.kind {
            SewingKind::Decorative => d
                .points
                .iter()
                .enumerate()
                .map(|(i, q)| (Handle::Point(i), *q))
                .collect(),
            SewingKind::Satin => d
                .profile
                .iter()
                .enumerate()
                .flat_map(|(i, &[x, l, r])| {
                    [
                        (Handle::Edge(i, false), v2(x, l)),
                        (Handle::Edge(i, true), v2(x, r)),
                    ]
                })
                .collect(),
            SewingKind::DecorativeSatin => d
                .outline
                .iter()
                .enumerate()
                .map(|(i, q)| (Handle::Vertex(i), *q))
                .collect(),
            SewingKind::Cross => Vec::new(),
        };
        candidates
            .into_iter()
            .map(|(h, q)| (h, q.distance(p)))
            .filter(|(_, dist)| *dist <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(h, _)| h)
    }

    /// Adds at `p`: a needle point at the end (decorative), a satin station at that x, an
    /// outline vertex on the nearest edge (decorative satin), or toggles the cross cell there.
    pub fn add(&mut self, p: Vec2) {
        let q = self.clamp(p);
        let d = &mut self.doc;
        match d.kind {
            SewingKind::Decorative => d.points.push(q),
            SewingKind::Satin => {
                if d.profile.iter().any(|s| (s[0] - q.x).abs() < 1e-9) {
                    return;
                }
                let half = (q.y.abs()).max(0.25);
                let at = d
                    .profile
                    .iter()
                    .position(|s| s[0] > q.x)
                    .unwrap_or(d.profile.len());
                d.profile.insert(at, [q.x, -half, half]);
            }
            SewingKind::DecorativeSatin => {
                let n = d.outline.len();
                let best = (0..n)
                    .min_by(|&i, &j| {
                        let e = |k: usize| seg_distance(q, d.outline[k], d.outline[(k + 1) % n]);
                        e(i).total_cmp(&e(j))
                    })
                    .unwrap_or(0);
                d.outline.insert(best + 1, q);
            }
            SewingKind::Cross => {
                let c = d.cell_size;
                let top = -f64::from(d.rows) * c / 2.0;
                if q.x < 0.0 || q.y < top {
                    return;
                }
                let col = (p.x / c).floor().max(0.0) as u32;
                let row = ((p.y - top) / c).floor().max(0.0) as u32;
                if row >= d.rows {
                    return;
                }
                match d.cells.iter().position(|&cell| cell == [col, row]) {
                    Some(i) => {
                        d.cells.remove(i);
                    }
                    None => d.cells.push([col, row]),
                }
            }
        }
    }

    /// Moves a handle to `p` (satin stations keep their order along the feed and left ≤ right).
    pub fn drag(&mut self, h: Handle, p: Vec2) {
        let q = self.clamp(p);
        let d = &mut self.doc;
        match h {
            Handle::Point(i) => {
                if let Some(pt) = d.points.get_mut(i) {
                    *pt = q;
                }
            }
            Handle::Vertex(i) => {
                if let Some(pt) = d.outline.get_mut(i) {
                    *pt = q;
                }
            }
            Handle::Edge(i, right) => {
                let n = d.profile.len();
                if i >= n {
                    return;
                }
                let lo = if i > 0 {
                    d.profile[i - 1][0] + SNAP
                } else {
                    0.0
                };
                let hi = if i + 1 < n {
                    d.profile[i + 1][0] - SNAP
                } else {
                    MAX_LENGTH
                };
                let s = &mut d.profile[i];
                s[0] = q.x.clamp(lo, hi.max(lo));
                if right {
                    s[2] = q.y.max(s[1]);
                } else {
                    s[1] = q.y.min(s[2]);
                }
            }
        }
    }

    /// Deletes a handle's point (keeping each kind's minimum).
    pub fn delete(&mut self, h: Handle) -> bool {
        let d = &mut self.doc;
        match h {
            Handle::Point(i) if d.points.len() > 2 && i < d.points.len() => {
                d.points.remove(i);
                true
            }
            Handle::Edge(i, _) if d.profile.len() > 2 && i < d.profile.len() => {
                d.profile.remove(i);
                true
            }
            Handle::Vertex(i) if d.outline.len() > 3 && i < d.outline.len() => {
                d.outline.remove(i);
                true
            }
            _ => false,
        }
    }

    fn document(&self) -> Value {
        serde_json::from_str(&self.doc.to_json(false)).unwrap_or_default()
    }

    /// `sewing.define` for the open project (one Undo).
    pub fn define_request(&self) -> Result<CommandRequest, String> {
        self.doc.validate()?;
        Ok(CommandRequest::new(
            "sewing.define",
            json!({ "stitch": self.document() }),
        ))
    }

    /// `sewing.export` of this pattern (it must be in the project) to `path`.
    pub fn export_request(&self, path: &str) -> CommandRequest {
        CommandRequest::new("sewing.export", json!({ "id": self.doc.id, "path": path }))
    }
}

fn seg_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l2 = d.x * d.x + d.y * d.y;
    let t = if l2 > 0.0 {
        (((p - a).x * d.x + (p - a).y * d.y) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + d * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_engine::Session;

    #[test]
    fn every_kind_reaches_the_project_and_exports() {
        let dir = std::env::temp_dir().join(format!(
            "openstitch-sdf-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let mut s = Session::sandboxed(&dir);
        for kind in SewingKind::ALL {
            let e = SewingEditor::new(kind);
            let r = e.define_request().expect("valid");
            s.execute(r.command, Some(&r.params)).expect("define");
            for ext in ["json", "svg", "pmv"] {
                let r = e.export_request(&format!("{}.{ext}", kind.id()));
                s.execute(r.command, Some(&r.params)).expect("export");
            }
        }
        assert_eq!(s.design().sewing.len(), 4);
    }

    #[test]
    fn decorative_points_are_added_dragged_and_deleted() {
        let mut e = SewingEditor::new(SewingKind::Decorative);
        e.add(v2(9.1, 2.6));
        assert_eq!(*e.doc.points.last().expect("point"), v2(9.0, 2.5));
        e.add(v2(10.0, 50.0));
        assert_eq!(
            e.doc.points.last().map(|p| p.y),
            Some(3.5),
            "clamped to the width"
        );
        let h = e.hit(v2(2.05, -2.9), 0.5).expect("hit");
        assert_eq!(h, Handle::Point(1));
        e.drag(h, v2(2.4, -1.1));
        assert_eq!(e.doc.points[1], v2(2.5, -1.0));
        assert!(e.delete(h));
        assert!(e.hit(v2(100.0, 0.0), 0.5).is_none());
        e.doc.points.truncate(2);
        assert!(!e.delete(Handle::Point(0)), "two points stay");
        assert!(e.define_request().is_ok());
    }

    #[test]
    fn satin_stations_keep_their_order_and_edges() {
        let mut e = SewingEditor::new(SewingKind::Satin);
        e.add(v2(2.0, 1.5));
        assert_eq!(e.doc.profile[1], [2.0, -1.5, 1.5]);
        e.add(v2(2.0, 3.0));
        assert_eq!(e.doc.profile.len(), 4, "one station per x");
        // Dragging a station past its neighbour stops before it.
        e.drag(Handle::Edge(1, true), v2(6.0, 3.0));
        assert_eq!(e.doc.profile[1][0], 4.0 - SNAP);
        // A right edge never crosses the left one.
        e.drag(Handle::Edge(1, true), v2(3.0, -3.5));
        assert_eq!(e.doc.profile[1][2], e.doc.profile[1][1]);
        assert!(e.doc.validate().is_ok());
        assert!(e.delete(Handle::Edge(1, false)));
        e.doc.profile.truncate(2);
        assert!(!e.delete(Handle::Edge(0, false)));
    }

    #[test]
    fn outline_vertices_go_on_the_nearest_edge() {
        let mut e = SewingEditor::new(SewingKind::DecorativeSatin);
        // Default diamond (0,0) (3,-3) (6,0) (3,3); a point near the first edge.
        e.add(v2(1.5, -2.0));
        assert_eq!(e.doc.outline[1], v2(1.5, -2.0));
        assert_eq!(e.doc.outline.len(), 5);
        assert!(e.delete(Handle::Vertex(1)));
        e.doc.outline.truncate(3);
        assert!(!e.delete(Handle::Vertex(0)), "a triangle stays");
    }

    #[test]
    fn cross_cells_toggle_inside_the_grid_only() {
        let mut e = SewingEditor::new(SewingKind::Cross);
        let before = e.doc.cells.len();
        e.add(v2(7.0, -2.5)); // column 3, row 0 (rows span -3…3)
        assert_eq!(e.doc.cells.last(), Some(&[3, 0]));
        e.add(v2(7.0, -2.5));
        assert_eq!(e.doc.cells.len(), before, "a second click removes it");
        e.add(v2(7.0, 3.4));
        assert_eq!(e.doc.cells.len(), before, "outside the three rows");
        assert!(e.hit(v2(0.0, 0.0), 10.0).is_none(), "cells have no handles");
    }
}
