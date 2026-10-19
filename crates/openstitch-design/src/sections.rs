//! Designs larger than one hooping: split designs (a custom Design Page cut into hoop
//! sections, sewn left to right, top to bottom, with alignment stitching), multi-position
//! hoops (three installation positions a, b, c) and the jumbo frame (two sides, the second
//! sewn with the frame turned 180°). See `docs/research/split-print-transfer.md` for the
//! public behaviour these follow and what had to be chosen where the guide only shows images.
//!
//! All coordinates are 0.1 mm on the Design Page; settings are millimetres.

use crate::compile::Compiler;
use crate::geometry::{Rect, Vec2, v2};
use crate::machine::{Hoop, brother_hoop};
use crate::model::Design;
use openstitch_core::{Command, Stitch, StitchPattern, Thread};
use serde::{Deserialize, Serialize};

/// The jumbo frame's hoop id and its Design Page (the guide: 360 × 360 frame, 350 × 350 page).
pub const JUMBO: &str = "360x360";
pub const JUMBO_PAGE_MM: f64 = 350.0;
/// Sections are this much smaller than their hoop in each direction (mm).
pub const SECTION_MARGIN_MM: f64 = 10.0;
/// Alignment stitching: running pitch and the fixing (tie-in) stitch length, 0.1 mm.
pub const ALIGN_PITCH: f64 = 70.0;
pub const ALIGN_TIE: f64 = 3.0;
/// At most this many rows and columns (labels use one letter each).
pub const MAX_SPLITS: usize = 26;

/// Split-design settings for a custom Design Page.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SectionSettings {
    /// Brother hoop whose sewing field, less [`SECTION_MARGIN_MM`], is the section size.
    pub hoop: String,
    /// "Use smaller section sizes": a further 10 mm smaller on every side.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub smaller: bool,
    /// Interior split positions from the page's left / top edge, mm. Empty: an even split.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub splits_x: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub splits_y: Vec<f64>,
}

impl SectionSettings {
    /// Section width and height in mm.
    pub fn section_size(&self) -> Option<(f64, f64)> {
        let h = brother_hoop(&self.hoop).filter(|h| sectionable(h))?;
        let m = SECTION_MARGIN_MM
            + if self.smaller {
                2.0 * SECTION_MARGIN_MM
            } else {
                0.0
            };
        Some((h.width - m, h.height - m))
    }
}

/// Hoops that can give the section size (not multi-position hoops or the jumbo frame).
pub fn sectionable(h: &Hoop) -> bool {
    !h.multi_position && h.id != JUMBO && h.width > 3.0 * SECTION_MARGIN_MM
}

/// Interior boundaries splitting `length` into the fewest equal parts no longer than `max`.
pub fn even_splits(length: f64, max: f64) -> Vec<f64> {
    let n = (length / max - 1e-9).ceil().max(1.0) as usize;
    (1..n).map(|k| length * k as f64 / n as f64).collect()
}

/// Checks user split positions: increasing, inside the page, no part longer than `max`.
pub fn check_splits(splits: &[f64], length: f64, max: f64) -> Result<(), String> {
    if splits.len() + 1 > MAX_SPLITS {
        return Err(format!("at most {MAX_SPLITS} sections per direction"));
    }
    let mut last = 0.0;
    for &s in splits.iter().chain([length].iter()) {
        if !s.is_finite() || s <= last {
            return Err("split positions must increase inside the page".into());
        }
        if s - last > max + 1e-6 {
            return Err(format!(
                "a section would be {:.1} mm long; the section size allows {max:.1} mm",
                s - last
            ));
        }
        last = s;
    }
    if last > length + 1e-9 {
        return Err("split positions must lie inside the page".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SectionKind {
    /// A hoop section of a split design.
    Split,
    /// One installation position of a multi-position hoop.
    Position,
    /// One side of the jumbo frame.
    JumboSide,
}

/// One hooping.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub kind: SectionKind,
    /// "Aa", "Ab", … (row, column) for split designs; "1a", "2b", … (order, position) for
    /// multi-position hoops; "side1", "side2" for the jumbo frame.
    pub label: String,
    /// The part of the page sewn in this hooping.
    pub rect: Rect,
    /// Sewn with the frame turned 180° (jumbo Side 2).
    pub rotate180: bool,
    /// Alignment stitching sewn before / after the design part, as edges.
    pub marks_before: Vec<(Vec2, Vec2)>,
    pub marks_after: Vec<(Vec2, Vec2)>,
    /// Multi-position hoops: the objects sewn in this hooping, in order (whole objects, not
    /// clipped). `None`: the stitches inside `rect`.
    pub objects: Option<Vec<u64>>,
}

/// What [`plan`] found: the hoopings in sewing order and objects that fit no position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub sections: Vec<Section>,
    pub errors: Vec<String>,
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
    Rect {
        min_x: x0,
        min_y: y0,
        max_x: x1,
        max_y: y1,
    }
}

/// The hoopings for `design`: a split grid when sections are set on a custom page, the three
/// positions of a multi-position hoop, the two jumbo sides, or one section covering the page.
pub fn plan(design: &Design) -> Plan {
    let m = &design.machine;
    let page = rect(0.0, 0.0, design.page.width, design.page.height);
    match m.hoop.as_deref().and_then(brother_hoop) {
        Some(h) if h.multi_position => return multi_position(design, h),
        Some(h) if h.id == JUMBO => return jumbo(design, m.jumbo_overlap),
        _ => {}
    }
    match (&m.sections, m.hoop.is_none()) {
        (Some(s), true) => split(design, s),
        _ => Plan {
            sections: vec![Section {
                kind: SectionKind::Split,
                label: "Aa".into(),
                rect: page,
                rotate180: false,
                marks_before: Vec::new(),
                marks_after: Vec::new(),
                objects: None,
            }],
            errors: Vec::new(),
        },
    }
}

fn split(design: &Design, s: &SectionSettings) -> Plan {
    let Some((sw, sh)) = s.section_size() else {
        return Plan {
            sections: Vec::new(),
            errors: vec![format!("hoop {} cannot give a section size", s.hoop)],
        };
    };
    let (w, h) = (design.page.width / 10.0, design.page.height / 10.0);
    let xs = if s.splits_x.is_empty() {
        even_splits(w, sw)
    } else {
        s.splits_x.clone()
    };
    let ys = if s.splits_y.is_empty() {
        even_splits(h, sh)
    } else {
        s.splits_y.clone()
    };
    let mut errors = Vec::new();
    for (v, len, max) in [(&xs, w, sw), (&ys, h, sh)] {
        if let Err(e) = check_splits(v, len, max) {
            errors.push(e);
        }
    }
    if !errors.is_empty() {
        return Plan {
            sections: Vec::new(),
            errors,
        };
    }
    let edges = |v: &[f64], len: f64| -> Vec<f64> {
        std::iter::once(0.0)
            .chain(v.iter().copied())
            .chain(std::iter::once(len))
            .map(|mm| mm * 10.0)
            .collect()
    };
    let (ex, ey) = (edges(&xs, w), edges(&ys, h));
    let (cols, rows) = (ex.len() - 1, ey.len() - 1);
    let mut sections = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let (x0, x1, y0, y1) = (ex[c], ex[c + 1], ey[r], ey[r + 1]);
            let (left, right) = ((v2(x0, y0), v2(x0, y1)), (v2(x1, y0), v2(x1, y1)));
            let (top, bottom) = ((v2(x0, y0), v2(x1, y0)), (v2(x0, y1), v2(x1, y1)));
            let mut before = Vec::new();
            let mut after = Vec::new();
            // Edges shared with sections already sewn are stitched first, to hoop against;
            // edges towards later sections after the design part.
            if c > 0 {
                before.push(left);
            }
            if r > 0 {
                before.push(top);
            }
            if c + 1 < cols {
                after.push(right);
            }
            if r + 1 < rows {
                after.push(bottom);
            }
            sections.push(Section {
                kind: SectionKind::Split,
                label: format!("{}{}", (b'A' + r as u8) as char, (b'a' + c as u8) as char),
                rect: rect(x0, y0, x1, y1),
                rotate180: false,
                marks_before: before,
                marks_after: after,
                objects: None,
            });
        }
    }
    Plan { sections, errors }
}

/// Areas a, b, c of a multi-position hoop: the hoop's single-pattern size, a at the top, c at
/// the bottom, b centred between them (the guide shows the offsets only in a picture).
pub fn position_areas(h: &Hoop) -> [Rect; 3] {
    let (w, area_h) = if h.width <= 100.0 {
        (100.0, 100.0)
    } else {
        (130.0, 180.0)
    };
    let x0 = (h.width - w) / 2.0;
    let span = h.height - area_h;
    [0.0, span / 2.0, span].map(|y| rect(x0 * 10.0, y * 10.0, (x0 + w) * 10.0, (y + area_h) * 10.0))
}

fn contains(outer: &Rect, inner: &Rect) -> bool {
    inner.min_x >= outer.min_x - 1e-6
        && inner.min_y >= outer.min_y - 1e-6
        && inner.max_x <= outer.max_x + 1e-6
        && inner.max_y <= outer.max_y + 1e-6
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.min_x < b.max_x && b.min_x < a.max_x && a.min_y < b.max_y && b.min_y < a.max_y
}

fn multi_position(design: &Design, h: &Hoop) -> Plan {
    let areas = position_areas(h);
    let mut errors = Vec::new();
    // (object id, position, bounds) in sewing order.
    let mut items: Vec<(u64, usize, Rect)> = Vec::new();
    for o in design.objects.iter().filter(|o| o.visible) {
        let Some(b) = o.bounds() else { continue };
        match areas.iter().position(|a| contains(a, &b)) {
            Some(p) => items.push((o.id, p, b)),
            None => errors.push(format!(
                "object {} ({:.1} × {:.1} mm) does not fit inside one position area",
                o.id,
                b.width() / 10.0,
                b.height() / 10.0
            )),
        }
    }
    let groups = if design.machine.optimize_hoop_change {
        optimize_hoop_changes(&items)
    } else {
        let mut groups: Vec<(usize, Vec<u64>)> = Vec::new();
        for &(id, p, _) in &items {
            match groups.last_mut() {
                Some((q, ids)) if *q == p => ids.push(id),
                _ => groups.push((p, vec![id])),
            }
        }
        groups
    };
    let sections = groups
        .into_iter()
        .enumerate()
        .map(|(i, (p, ids))| Section {
            kind: SectionKind::Position,
            label: format!("{}{}", i + 1, (b'a' + p as u8) as char),
            rect: areas[p],
            rotate180: false,
            marks_before: Vec::new(),
            marks_after: Vec::new(),
            objects: Some(ids),
        })
        .collect();
    Plan { sections, errors }
}

/// Groups patterns by position with fewer hoop changes: after taking the next pattern, later
/// patterns of the same position join it, unless one would move ahead of a pattern it overlaps
/// (that would change what is sewn on top) or of an earlier pattern of its own position that
/// could not join. Reproduces the guide's example 1a 2b 3a 4c 5a 6c → a(1) b(2) a(3, 5) c(4, 6)
/// when 3 overlaps 2.
pub fn optimize_hoop_changes(items: &[(u64, usize, Rect)]) -> Vec<(usize, Vec<u64>)> {
    let mut left: Vec<(u64, usize, Rect)> = items.to_vec();
    let mut groups = Vec::new();
    while !left.is_empty() {
        let first = left.remove(0);
        let mut ids = vec![first.0];
        let mut skipped: Vec<Rect> = Vec::new();
        let mut blocked = false;
        let mut i = 0;
        while i < left.len() {
            let (id, p, b) = left[i];
            if p == first.1 && !blocked && !skipped.iter().any(|s| overlaps(s, &b)) {
                ids.push(id);
                left.remove(i);
            } else {
                // Patterns of one position keep their order: once one stays behind, the
                // later ones of that position stay behind too.
                blocked |= p == first.1;
                skipped.push(b);
                i += 1;
            }
        }
        groups.push((first.1, ids));
    }
    groups
}

fn jumbo(design: &Design, overlap_mm: f64) -> Plan {
    let (w, h) = (design.page.width, design.page.height);
    let half = h / 2.0;
    let o = overlap_mm * 10.0 / 2.0;
    let side = |label: &str, y0: f64, y1: f64, rotate180| Section {
        kind: SectionKind::JumboSide,
        label: label.into(),
        rect: rect(0.0, y0, w, y1),
        rotate180,
        marks_before: Vec::new(),
        marks_after: Vec::new(),
        objects: None,
    };
    Plan {
        sections: vec![
            side("side1", 0.0, half + o, false),
            side("side2", half - o, h, true),
        ],
        errors: Vec::new(),
    }
}

// MARK: section stitches

/// Thread of the alignment stitching (PE-DESIGN leaves its colour undefined).
pub fn alignment_thread() -> Thread {
    Thread::rgb(0x9E9E9E).with_name("Alignment")
}

/// A running stitch along `a → b` at [`ALIGN_PITCH`], fixed with short stitches at both ends.
pub fn alignment_run(a: Vec2, b: Vec2) -> Vec<Vec2> {
    let len = a.distance(b);
    if len == 0.0 {
        return vec![a];
    }
    let dir = (b - a) * (1.0 / len);
    let tie = ALIGN_TIE.min(len / 2.0);
    let mut run = vec![a, a + dir * tie, a, a + dir * tie];
    let n = ((len - tie) / ALIGN_PITCH).ceil().max(1.0) as usize;
    for k in 1..=n {
        run.push(a + dir * (tie + (len - tie) * k as f64 / n as f64));
    }
    run.extend([b - dir * tie, b]);
    run
}

/// Liang–Barsky: the part of `a → b` inside `r`.
pub fn clip_segment(a: Vec2, b: Vec2, r: &Rect) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-d.x, a.x - r.min_x),
        (d.x, r.max_x - a.x),
        (-d.y, a.y - r.min_y),
        (d.y, r.max_y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then(|| (a + d * t0, a + d * t1))
}

/// Sewn runs per colour block of `pattern` inside `r` (stitches cut at the edge).
fn clipped_blocks(pattern: &StitchPattern, r: &Rect) -> Vec<(Thread, Vec<Vec<Vec2>>)> {
    let mut out = Vec::new();
    let mut prev: Option<Vec2> = None;
    for (thread, stitches) in pattern.color_blocks() {
        let mut runs: Vec<Vec<Vec2>> = Vec::new();
        let mut cut = true;
        for s in &stitches {
            let p = v2(s.x, s.y);
            match s.command {
                Command::Stitch => {
                    let piece = match prev {
                        Some(a) if a != p => clip_segment(a, p, r),
                        _ => (p.x >= r.min_x && p.x <= r.max_x && p.y >= r.min_y && p.y <= r.max_y)
                            .then_some((p, p)),
                    };
                    match piece {
                        Some((c0, c1)) => {
                            let joined = !cut
                                && runs
                                    .last()
                                    .and_then(|run| run.last())
                                    .is_some_and(|&l| l == c0);
                            if !joined {
                                runs.push(vec![c0]);
                            }
                            let run = runs.last_mut().expect("pushed");
                            if run.last() != Some(&c1) {
                                run.push(c1);
                            }
                            cut = false;
                        }
                        None => cut = true,
                    }
                }
                Command::Jump | Command::Trim | Command::ColorChange => cut = true,
                Command::Stop | Command::End => {}
            }
            if matches!(s.command, Command::Stitch | Command::Jump) {
                prev = Some(p);
            }
        }
        runs.retain(|run| run.len() > 1);
        if !runs.is_empty() {
            out.push((thread.cloned().unwrap_or(Thread::rgb(0)), runs));
        }
    }
    out
}

/// Writes colour blocks of runs as a stitch stream: jumps between runs (trimmed when longer
/// than `trim`), a trim and colour change between blocks, a final trim and end.
fn write_blocks(blocks: Vec<(Thread, Vec<Vec<Vec2>>)>, trim: f64) -> StitchPattern {
    let mut out = StitchPattern::new();
    let mut pos: Option<Vec2> = None;
    let push = |out: &mut StitchPattern, p: Vec2, c: Command| {
        out.stitches.push(Stitch::new(p.x, p.y, c));
    };
    for (i, (thread, runs)) in blocks.into_iter().enumerate() {
        if i > 0 {
            let p = pos.unwrap_or_default();
            push(&mut out, p, Command::Trim);
            push(&mut out, p, Command::ColorChange);
        }
        out.threads.push(thread);
        for run in runs {
            let start = run[0];
            if let Some(p) = pos {
                if p.distance(start) > trim {
                    push(&mut out, p, Command::Trim);
                }
                if p != start {
                    push(&mut out, start, Command::Jump);
                }
            } else {
                push(&mut out, start, Command::Jump);
            }
            for &q in &run {
                push(&mut out, q, Command::Stitch);
            }
            pos = run.last().copied();
        }
    }
    if let Some(p) = pos {
        push(&mut out, p, Command::Trim);
        push(&mut out, p, Command::End);
    }
    out
}

/// The machine file content of one hooping: alignment stitching before, the design part,
/// alignment stitching after; turned 180° for jumbo Side 2; moved so the section's centre (the
/// hoop centre) is at (0, 0).
pub fn section_pattern(design: &Design, full: &StitchPattern, section: &Section) -> StitchPattern {
    let compiler = Compiler::for_design(design);
    let mut blocks = Vec::new();
    let marks = |edges: &[(Vec2, Vec2)]| -> Option<(Thread, Vec<Vec<Vec2>>)> {
        (!edges.is_empty()).then(|| {
            (
                alignment_thread(),
                edges.iter().map(|&(a, b)| alignment_run(a, b)).collect(),
            )
        })
    };
    blocks.extend(marks(&section.marks_before));
    match &section.objects {
        Some(ids) => {
            let mut part = design.clone();
            part.objects = ids
                .iter()
                .filter_map(|id| design.index_of(*id).map(|i| design.objects[i].clone()))
                .collect();
            let pattern = compiler.compile(&part);
            let everything = rect(f64::MIN, f64::MIN, f64::MAX, f64::MAX);
            blocks.extend(clipped_blocks(&pattern, &everything));
        }
        None => blocks.extend(clipped_blocks(full, &section.rect)),
    }
    blocks.extend(marks(&section.marks_after));
    let mut out = write_blocks(blocks, compiler.trim_distance);
    let c = section.rect.center();
    for s in &mut out.stitches {
        let (dx, dy) = (s.x - c.x, s.y - c.y);
        (s.x, s.y) = if section.rotate180 {
            (-dx, -dy)
        } else {
            (dx, dy)
        };
    }
    out.set_name(format!("{}_{}", design.name, section.label));
    out
}

/// Whether a section has any design stitches (alignment stitching alone does not count).
pub fn has_design_stitches(pattern: &StitchPattern) -> bool {
    pattern.color_blocks().iter().any(|(t, s)| {
        t.is_none_or(|t| *t != alignment_thread()) && s.iter().any(|s| s.command == Command::Stitch)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Contour;
    use crate::model::{DesignObject, DesignPage, LineSewing, LineType};

    fn page(w_mm: f64, h_mm: f64) -> Design {
        Design::new(
            "Split",
            DesignPage {
                width: w_mm * 10.0,
                height: h_mm * 10.0,
            },
        )
    }

    fn line(d: &mut Design, pts: &[(f64, f64)]) -> u64 {
        let c = Contour::new(
            pts.iter().map(|&(x, y)| v2(x * 10.0, y * 10.0)).collect(),
            false,
        );
        d.add(DesignObject::shape(
            vec![c],
            None,
            Some(LineSewing::new(LineType::Running, Thread::rgb(0xFF0000))),
        ))
    }

    #[test]
    fn even_splits_use_the_fewest_parts() {
        assert_eq!(even_splits(230.0, 120.0), vec![115.0]);
        assert_eq!(even_splits(120.0, 120.0), Vec::<f64>::new());
        assert_eq!(even_splits(361.0, 120.0).len(), 3);
        assert!(check_splits(&[100.0], 230.0, 120.0).is_err(), "130 mm part");
        assert!(check_splits(&[115.0, 115.0], 230.0, 120.0).is_err());
        assert!(check_splits(&[-5.0], 230.0, 120.0).is_err());
        assert!(check_splits(&[115.0], 230.0, 120.0).is_ok());
    }

    #[test]
    fn split_grid_orders_sections_and_alignment_edges_like_the_guide() {
        let mut d = page(230.0, 330.0);
        d.machine.sections = Some(SectionSettings {
            hoop: "130x180".into(),
            ..Default::default()
        });
        let p = plan(&d);
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let labels: Vec<_> = p.sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Aa", "Ab", "Ba", "Bb"]);
        let [ul, ur, ll, lr] = &p.sections[..] else {
            panic!()
        };
        assert_eq!(ul.rect.max_x, 1150.0);
        assert_eq!(ul.rect.max_y, 1650.0);
        // UL: right and bottom after; UR: left before, bottom after; LL: top before, right
        // after; LR: left and top before.
        assert_eq!((ul.marks_before.len(), ul.marks_after.len()), (0, 2));
        assert_eq!((ur.marks_before.len(), ur.marks_after.len()), (1, 1));
        assert_eq!((ll.marks_before.len(), ll.marks_after.len()), (1, 1));
        assert_eq!((lr.marks_before.len(), lr.marks_after.len()), (2, 0));
        assert_eq!(ur.marks_before[0], ul.marks_after[0], "shared edge");

        // Smaller sections: 130 × 180 hoop → 100 × 150 sections → 3 × 3.
        d.machine.sections.as_mut().expect("set").smaller = true;
        assert_eq!(plan(&d).sections.len(), 9);
        // User splits beyond the section size are reported.
        let s = d.machine.sections.as_mut().expect("set");
        s.smaller = false;
        s.splits_x = vec![50.0];
        assert!(!plan(&d).errors.is_empty());
    }

    #[test]
    fn clipping_cuts_stitches_at_section_edges() {
        let r = rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(
            clip_segment(v2(-50.0, 50.0), v2(150.0, 50.0), &r),
            Some((v2(0.0, 50.0), v2(100.0, 50.0)))
        );
        assert_eq!(clip_segment(v2(-50.0, -1.0), v2(150.0, -1.0), &r), None);
        assert_eq!(
            clip_segment(v2(10.0, 10.0), v2(20.0, 20.0), &r),
            Some((v2(10.0, 10.0), v2(20.0, 20.0)))
        );
    }

    #[test]
    fn section_files_hold_their_part_with_alignment_stitching_centred_on_the_hoop() {
        let mut d = page(230.0, 160.0);
        d.machine.sections = Some(SectionSettings {
            hoop: "130x180".into(),
            ..Default::default()
        });
        line(&mut d, &[(20.0, 80.0), (210.0, 80.0)]);
        let full = Compiler::for_design(&d).compile(&d);
        let p = plan(&d);
        assert_eq!(p.sections.len(), 2);
        let a = section_pattern(&d, &full, &p.sections[0]);
        let b = section_pattern(&d, &full, &p.sections[1]);
        for (pat, s) in [(&a, &p.sections[0]), (&b, &p.sections[1])] {
            let (hw, hh) = (s.rect.width() / 2.0, s.rect.height() / 2.0);
            assert!(
                pat.stitches
                    .iter()
                    .all(|st| st.x.abs() <= hw + 1e-6 && st.y.abs() <= hh + 1e-6)
            );
            assert!(has_design_stitches(pat));
        }
        // Section Aa: design, then its right edge; Ab: its left edge, then the design.
        assert_eq!(a.threads.last(), Some(&alignment_thread()));
        assert_eq!(b.threads.first(), Some(&alignment_thread()));
        assert_eq!(a.threads[0], Thread::rgb(0xFF0000));
        // The line is cut at the split (x = 115 mm → 0 mm from Ab's left edge at -57.5 mm).
        let first_design = b.color_blocks()[1]
            .1
            .iter()
            .find(|s| s.command == Command::Stitch)
            .copied()
            .expect("stitch");
        assert!((first_design.x + 575.0).abs() < 1e-6, "{first_design:?}");
    }

    #[test]
    fn alignment_runs_are_fixed_and_at_most_seven_mm() {
        let run = alignment_run(v2(0.0, 0.0), v2(0.0, 1650.0));
        assert_eq!(run[1], v2(0.0, ALIGN_TIE));
        assert_eq!(run.last(), Some(&v2(0.0, 1650.0)));
        assert!(
            run.windows(2)
                .all(|w| w[0].distance(w[1]) <= ALIGN_PITCH + 1e-9)
        );
    }

    #[test]
    fn multi_position_assigns_patterns_and_optimizes_hoop_changes() {
        let r = |y: f64| rect(0.0, y, 100.0, y + 100.0);
        // The guide's example: 1a, 2b, 3a, 4c, 5a, 6c → a(1), b(2), a(3, 5), c(4, 6); pattern
        // 3 overlaps 2, so it cannot join 1.
        let items = [
            (1, 0, r(0.0)),
            (2, 1, rect(0.0, 900.0, 1000.0, 1000.0)),
            (3, 0, rect(0.0, 950.0, 100.0, 1050.0)),
            (4, 2, r(2000.0)),
            (5, 0, r(300.0)),
            (6, 2, r(2500.0)),
        ];
        assert_eq!(
            optimize_hoop_changes(&items),
            vec![(0, vec![1]), (1, vec![2]), (0, vec![3, 5]), (2, vec![4, 6])]
        );

        let mut d = page(130.0, 300.0);
        d.machine.hoop = Some("130x300-multi".into());
        let areas = position_areas(brother_hoop("130x300-multi").expect("hoop"));
        assert_eq!((areas[1].min_y, areas[2].max_y), (600.0, 3000.0));
        line(&mut d, &[(10.0, 10.0), (100.0, 100.0)]); // a
        line(&mut d, &[(10.0, 250.0), (100.0, 290.0)]); // c
        line(&mut d, &[(10.0, 20.0), (60.0, 60.0)]); // a
        line(&mut d, &[(10.0, 10.0), (10.0, 290.0)]); // too tall
        let p = plan(&d);
        assert_eq!(p.errors.len(), 1);
        let labels: Vec<_> = p.sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["1a", "2c", "3a"]);
        d.machine.optimize_hoop_change = true;
        let p = plan(&d);
        let labels: Vec<_> = p.sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["1a", "2c"]);
        assert_eq!(p.sections[0].objects, Some(vec![1, 3]));
        let full = Compiler::for_design(&d).compile(&d);
        let a = section_pattern(&d, &full, &p.sections[0]);
        assert!(has_design_stitches(&a));
        assert!(a.stitches.iter().all(|s| s.y.abs() <= 900.0 + 1e-6));
    }

    #[test]
    fn jumbo_splits_into_two_sides_and_turns_side_two() {
        let mut d = page(350.0, 350.0);
        d.machine.hoop = Some(JUMBO.into());
        d.machine.jumbo_overlap = 20.0;
        line(&mut d, &[(10.0, 340.0), (60.0, 340.0)]);
        let p = plan(&d);
        let [s1, s2] = &p.sections[..] else { panic!() };
        assert_eq!((s1.rect.max_y, s2.rect.min_y), (1850.0, 1650.0));
        assert!(s2.rotate180 && !s1.rotate180);
        let full = Compiler::for_design(&d).compile(&d);
        assert!(!has_design_stitches(&section_pattern(&d, &full, s1)));
        let b = section_pattern(&d, &full, s2);
        // Bottom-left of the page becomes top-right after the 180° turn.
        let st = b
            .stitches
            .iter()
            .find(|s| s.command == Command::Stitch)
            .expect("stitch");
        assert!(st.x > 0.0 && st.y < 0.0, "{st:?}");
    }
}
