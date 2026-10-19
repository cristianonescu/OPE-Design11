//! Hoop Settings ▸ Move Sections: drag the split lines of a split design (PE-DESIGN guide,
//! "Adjusting the split positions in split embroidery designs"). Releasing a line sets the
//! new positions with `sections.set`; the engine refuses sections larger than the hoop allows.
//! Model units are 0.1 mm; split positions are mm from the page's left / top edge.

use super::request::CommandRequest;
use openstitch_design::sections::{self, SectionKind};
use openstitch_design::{Design, Vec2};
use serde_json::json;

/// Which way a split line runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// A vertical line at an x position (`splitsX`).
    Vertical,
    /// A horizontal line at a y position (`splitsY`).
    Horizontal,
}

/// The interior split positions (mm) of a split design, as the plan places them: vertical
/// lines (x), then horizontal lines (y). Empty for designs that are not split.
pub fn current_splits(design: &Design) -> (Vec<f64>, Vec<f64>) {
    let plan = sections::plan(design);
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for s in plan
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Split)
    {
        if s.rect.min_x > 0.0 && !xs.contains(&(s.rect.min_x / 10.0)) {
            xs.push(s.rect.min_x / 10.0);
        }
        if s.rect.min_y > 0.0 && !ys.contains(&(s.rect.min_y / 10.0)) {
            ys.push(s.rect.min_y / 10.0);
        }
    }
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    (xs, ys)
}

/// Whether Move Sections applies: a split design (sections set on a custom page) with more
/// than one section.
pub fn can_move(design: &Design) -> bool {
    let (xs, ys) = current_splits(design);
    design.machine.sections.is_some() && !(xs.is_empty() && ys.is_empty())
}

#[derive(Clone, Debug, PartialEq)]
struct Drag {
    axis: Axis,
    index: usize,
    splits: Vec<f64>,
    /// Where the line is now, mm.
    at: f64,
}

/// Move Sections mode: when active, pressing near a split line grabs it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionMover {
    pub active: bool,
    drag: Option<Drag>,
}

/// Lines keep at least this far (mm) from their neighbours and the page edges.
const GAP_MM: f64 = 1.0;

impl SectionMover {
    pub fn toggle(&mut self) {
        self.active = !self.active;
        self.drag = None;
    }

    /// Grabs the split line within `tolerance` (model units) of `at`. Returns whether one
    /// was grabbed.
    pub fn press(&mut self, design: &Design, at: Vec2, tolerance: f64) -> bool {
        self.drag = None;
        if !self.active || !can_move(design) {
            return false;
        }
        let (xs, ys) = current_splits(design);
        let near = |splits: &[f64], v: f64, axis: Axis| {
            splits
                .iter()
                .enumerate()
                .map(|(i, &s)| (axis, i, (s * 10.0 - v).abs()))
                .filter(|&(.., d)| d <= tolerance)
                .collect::<Vec<_>>()
        };
        let page = design.page;
        let inside = at.x >= 0.0 && at.y >= 0.0 && at.x <= page.width && at.y <= page.height;
        if !inside {
            return false;
        }
        let best = near(&xs, at.x, Axis::Vertical)
            .into_iter()
            .chain(near(&ys, at.y, Axis::Horizontal))
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((axis, index, _)) = best {
            let splits = if axis == Axis::Vertical { xs } else { ys };
            self.drag = Some(Drag {
                axis,
                index,
                at: splits[index],
                splits,
            });
        }
        self.drag.is_some()
    }

    /// Moves the grabbed line to the pointer, kept between its neighbours.
    pub fn drag(&mut self, design: &Design, at: Vec2) {
        let page = design.page;
        let Some(d) = &mut self.drag else { return };
        let (v, length) = match d.axis {
            Axis::Vertical => (at.x, page.width),
            Axis::Horizontal => (at.y, page.height),
        };
        let lo = if d.index == 0 {
            0.0
        } else {
            d.splits[d.index - 1]
        } + GAP_MM;
        let hi = d.splits.get(d.index + 1).copied().unwrap_or(length / 10.0) - GAP_MM;
        d.at = ((v / 10.0).clamp(lo, hi.max(lo)) * 10.0).round() / 10.0;
    }

    /// Lets go of the line: the `sections.set` request for its new position, if it moved.
    pub fn release(&mut self, design: &Design, at: Vec2) -> Option<CommandRequest> {
        self.drag(design, at);
        let d = self.drag.take()?;
        if (d.at - d.splits[d.index]).abs() < 0.05 {
            return None;
        }
        let mut splits = d.splits;
        splits[d.index] = d.at;
        let key = match d.axis {
            Axis::Vertical => "splitsX",
            Axis::Horizontal => "splitsY",
        };
        Some(CommandRequest::new("sections.set", json!({ key: splits })))
    }

    /// The line being dragged and where it is now (mm).
    pub fn dragged(&self) -> Option<(Axis, f64)> {
        self.drag.as_ref().map(|d| (d.axis, d.at))
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }
}
