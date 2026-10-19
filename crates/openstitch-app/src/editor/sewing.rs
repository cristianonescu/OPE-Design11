//! Stitch Design Factory editor model: the sewing-stitch pattern being edited, hit-testing and
//! edits of its points (needle points, satin stations, decorative-satin outline, cross cells),
//! and the commands that put it in the project or export it. No egui; `sewing_ui` draws it.
//! Coordinates are mm: x along the feed, y across.

use super::request::CommandRequest;
use openstitch_design::sewing::{MAX_LENGTH, SewingKind, SewingStitch};
use openstitch_design::{Vec2, v2};
use serde_json::{Value, json};

/// Default editing grid (mm).
pub const SNAP: f64 = 0.25;
/// Grid Spacing choices (mm).
pub const GRID_SPACINGS: [f64; 5] = [0.25, 0.5, 1.0, 2.0, 5.0];

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
    /// Grid Spacing (mm): the grid drawn and what points snap to.
    pub grid: f64,
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
            grid: SNAP,
        }
    }

    pub fn from_stitch(s: &SewingStitch) -> Self {
        SewingEditor {
            doc: s.clone(),
            snap: true,
            grid: SNAP,
        }
    }

    /// `p` snapped (when on) and kept inside the band 0…200 mm × ±width/2.
    pub fn clamp(&self, p: Vec2) -> Vec2 {
        let g = self.grid;
        let p = if self.snap {
            v2((p.x / g).round() * g, (p.y / g).round() * g)
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

    /// Mirrors the pattern along the feed (`horizontal`: start and end swap, sewing order
    /// reversed) or across it.
    pub fn flip(&mut self, horizontal: bool) {
        let d = &mut self.doc;
        let xs = || {
            d.points
                .iter()
                .chain(&d.outline)
                .map(|p| p.x)
                .chain(d.profile.iter().map(|s| s[0]))
        };
        // Mirrored about the middle of what is drawn, so flipping twice restores it.
        let span = xs().fold(f64::INFINITY, f64::min) + xs().fold(f64::NEG_INFINITY, f64::max);
        if horizontal {
            for p in d.points.iter_mut().chain(d.outline.iter_mut()) {
                p.x = span - p.x;
            }
            d.points.reverse();
            d.outline.reverse();
            for s in &mut d.profile {
                s[0] = span - s[0];
            }
            d.profile.reverse();
            let cols = d.cells.iter().map(|c| c[0]);
            let cols = cols.clone().min().unwrap_or(0) + cols.max().unwrap_or(0);
            for c in &mut d.cells {
                c[0] = cols - c[0];
            }
        } else {
            for p in d.points.iter_mut().chain(d.outline.iter_mut()) {
                p.y = -p.y;
            }
            for s in &mut d.profile {
                *s = [s[0], -s[2], -s[1]];
            }
            for c in &mut d.cells {
                c[1] = d.rows.saturating_sub(1).saturating_sub(c[1]);
            }
        }
    }

    /// Select Object: moves the whole pattern by `delta` mm (snapped to the grid when on; cross
    /// patterns by whole cells). Refused, leaving it unchanged, when it would leave the band.
    pub fn move_by(&mut self, delta: Vec2) -> bool {
        let g = self.grid;
        let delta = if self.snap {
            v2((delta.x / g).round() * g, (delta.y / g).round() * g)
        } else {
            delta
        };
        if delta.x == 0.0 && delta.y == 0.0 {
            return false;
        }
        self.doc.translate(delta).is_ok()
    }

    /// Pen Tool: a stroke dragged through `stroke` (mm), sampled every grid step (at least
    /// 0.5 mm). Decorative patterns gain needle points at the end, satin patterns stations,
    /// cross patterns the cells passed over; a decorative satin outline is replaced by the
    /// stroke when it encloses an area. Returns whether the pattern changed.
    pub fn pen(&mut self, stroke: &[Vec2]) -> bool {
        let step = self.grid.max(0.5);
        let mut samples: Vec<Vec2> = Vec::new();
        for &p in stroke {
            let q = self.clamp(p);
            if samples.last().is_none_or(|l| l.distance(q) >= step - 1e-9) {
                samples.push(q);
            }
        }
        let before = self.doc.clone();
        match self.doc.kind {
            SewingKind::Decorative | SewingKind::Satin => {
                for q in samples {
                    self.add(q);
                }
            }
            SewingKind::DecorativeSatin => {
                if samples.len() >= 3 {
                    self.doc.outline = samples;
                }
            }
            SewingKind::Cross => {
                let d = &self.doc;
                let c = d.cell_size;
                let top = -f64::from(d.rows) * c / 2.0;
                let mut cells = d.cells.clone();
                for q in stroke {
                    let (col, row) = ((q.x / c).floor(), ((q.y - top) / c).floor());
                    if col < 0.0 || row < 0.0 || row >= f64::from(d.rows) {
                        continue;
                    }
                    let cell = [col as u32, row as u32];
                    if !cells.contains(&cell) {
                        cells.push(cell);
                    }
                }
                self.doc.cells = cells;
            }
        }
        if self.doc.validate().is_err() {
            self.doc = before;
            return false;
        }
        self.doc != before
    }

    /// Swap: start and end exchanged (the needle points sewn in reverse order).
    pub fn swap(&mut self) {
        self.doc.swap();
    }

    /// Duplicate: a copy of the pattern placed where it ends.
    pub fn duplicate(&mut self) -> Result<(), String> {
        self.doc = self.doc.duplicate()?;
        Ok(())
    }

    /// The bounding box of what is drawn (mm): handles and needle points.
    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        let d = &self.doc;
        let pts: Vec<Vec2> = d
            .needles()
            .into_iter()
            .chain(d.points.iter().copied())
            .chain(d.outline.iter().copied())
            .chain(d.profile.iter().flat_map(|&[x, l, r]| [v2(x, l), v2(x, r)]))
            .collect();
        let first = *pts.first()?;
        Some(pts.iter().fold((first, first), |(lo, hi), p| {
            (
                v2(lo.x.min(p.x), lo.y.min(p.y)),
                v2(hi.x.max(p.x), hi.y.max(p.y)),
            )
        }))
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
    fn flips_keep_every_kind_valid() {
        for kind in SewingKind::ALL {
            let mut e = SewingEditor::new(kind);
            let before = e.doc.clone();
            for horizontal in [true, false] {
                e.flip(horizontal);
                assert!(e.doc.validate().is_ok(), "{kind:?} {horizontal}");
                e.flip(horizontal);
                assert_eq!(e.doc, before, "{kind:?}: flipping twice restores it");
            }
        }
        let mut e = SewingEditor::new(SewingKind::Decorative);
        let first = e.doc.points[0];
        e.flip(false);
        assert_eq!(e.doc.points[0], v2(first.x, -first.y));
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

    #[test]
    fn grid_spacing_sets_what_points_snap_to() {
        let mut e = SewingEditor::new(SewingKind::Decorative);
        e.grid = 2.0;
        e.add(v2(9.1, 0.9));
        assert_eq!(*e.doc.points.last().expect("point"), v2(10.0, 0.0));
        e.snap = false;
        e.add(v2(11.1, 0.9));
        assert_eq!(*e.doc.points.last().expect("point"), v2(11.1, 0.9));
    }

    #[test]
    fn select_object_moves_the_whole_pattern_inside_the_band() {
        let mut e = SewingEditor::new(SewingKind::Decorative);
        assert!(e.move_by(v2(1.1, 0.4)));
        assert_eq!(e.doc.points[0], v2(1.0, 0.5));
        let before = e.doc.clone();
        assert!(!e.move_by(v2(0.0, 2.0)), "would leave the width");
        assert!(!e.move_by(v2(0.05, 0.0)), "less than a grid step");
        assert_eq!(e.doc, before);
        let (lo, hi) = e.bounds().expect("bounds");
        assert_eq!((lo, hi), (v2(1.0, -2.5), v2(9.0, 3.5)));
    }

    #[test]
    fn pen_strokes_add_to_every_kind() {
        let stroke: Vec<Vec2> = (0..=40)
            .map(|i| v2(10.0 + f64::from(i) * 0.1, 1.0))
            .collect();
        let mut e = SewingEditor::new(SewingKind::Decorative);
        let n = e.doc.points.len();
        assert!(e.pen(&stroke));
        assert_eq!(e.doc.points.len(), n + 9, "every 0.5 mm over 4 mm");
        assert_eq!(*e.doc.points.last().expect("point"), v2(14.0, 1.0));

        let mut e = SewingEditor::new(SewingKind::Satin);
        assert!(e.pen(&stroke));
        assert!(e.doc.profile.len() > 3 && e.doc.validate().is_ok());

        let mut e = SewingEditor::new(SewingKind::DecorativeSatin);
        let ring: Vec<Vec2> = (0..24)
            .map(|i| {
                let a = f64::from(i) * std::f64::consts::TAU / 24.0;
                v2(5.0 + 3.0 * a.cos(), 3.0 * a.sin())
            })
            .collect();
        assert!(e.pen(&ring));
        assert!(e.doc.outline.len() >= 12);
        assert!(!e.pen(&stroke), "a straight stroke encloses nothing");

        let mut e = SewingEditor::new(SewingKind::Cross);
        assert!(e.pen(&stroke));
        assert!(e.doc.cells.contains(&[5, 2]) && e.doc.cells.contains(&[7, 2]));
        assert!(!e.pen(&stroke), "painting twice adds nothing");
    }

    #[test]
    fn swap_and_duplicate_change_the_pattern() {
        let mut e = SewingEditor::new(SewingKind::Decorative);
        e.swap();
        assert_eq!(e.doc.needles()[0], v2(8.0, 0.0));
        e.duplicate().expect("duplicate");
        assert_eq!(e.doc.points.len(), 10);
        assert!(e.define_request().is_ok());
        let mut e = SewingEditor::new(SewingKind::DecorativeSatin);
        assert!(e.duplicate().is_err());
    }
}
