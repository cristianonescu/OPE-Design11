//! PE-DESIGN's pattern wizards as engine commands: background fill around patterns (Tutorial 8),
//! appliqué (9-1), embroidered patch (9-2) and cutwork (9-3). Each wizard builds ordinary shape
//! objects with line and region sewing, so the result stays editable, and is one undo step.
//!
//! Machine stops between stages are colour changes: each stage gets its own thread, named after
//! the stage (see [`stage_thread`]), so the compiler starts a new colour block there and the
//! machine stops, as PE-DESIGN does with its appliqué and cutwork colours. The behaviour comes
//! from the public PE-DESIGN guide; the geometry is this project's own.

pub(crate) mod applique;
mod background;
mod cutwork;
mod patch;

use crate::commands::CommandSpec;
use crate::params::{CommandError, Params};
use openstitch_core::Thread;
use openstitch_design::boolean::{self, BoolOp, Region};
use openstitch_design::geometry::contains;
use openstitch_design::paths;
use openstitch_design::{Contour, Design, DesignObject, LineType, ObjectKind, Rect, Vec2};

pub fn commands() -> Vec<CommandSpec> {
    let mut v = Vec::new();
    v.extend(background::commands());
    v.extend(applique::commands());
    v.extend(patch::commands());
    v.extend(cutwork::commands());
    v
}

/// Most lines or pieces a wizard generates, so hostile sizes cannot exhaust memory.
pub(crate) const MAX_PIECES: usize = 20_000;

/// `thread` renamed after a wizard stage, so that stage is its own colour block (a machine stop).
/// Catalogue number and brand are cleared: the colour is the user's, the name is the stage's.
pub(crate) fn stage_thread(thread: &Thread, name: &'static str) -> Thread {
    let mut t = thread.clone();
    t.name = Some(name.into());
    t.catalog_number = None;
    t.brand = None;
    t
}

/// Shape and text objects among `idx` whose outlines are all closed, or an error naming the
/// first that is not.
pub(crate) fn closed_outlines(p: &Params, d: &Design, idx: &[usize]) -> Result<(), CommandError> {
    for &i in idx {
        let o = &d.objects[i];
        if !o.is_outline() {
            return Err(p.failed(format!(
                "object {} is a stitch pattern, not an outline",
                o.id
            )));
        }
        if o.contours.is_empty() || o.contours.iter().any(|c| !c.closed) {
            return Err(p.failed(format!(
                "object {} has open lines; the wizard needs closed outlines",
                o.id
            )));
        }
    }
    Ok(())
}

/// The union of the closed outlines of the objects at `idx`, as connected regions (each object
/// even-odd within itself, so letter counters stay holes).
pub(crate) fn outline_regions(d: &Design, idx: &[usize]) -> Vec<Region> {
    let parts: Vec<Vec<Contour>> = idx
        .iter()
        .map(|&i| {
            d.objects[i]
                .contours
                .iter()
                .filter(|c| c.closed)
                .cloned()
                .collect()
        })
        .collect();
    boolean::union_all(parts.iter().map(|p| &p[..]))
}

/// Inserts the wizard's stage objects at sewing position `at`, in order, as one group (when
/// there are two or more). Returns their ids.
pub(crate) fn insert_stages(d: &mut Design, at: usize, stages: Vec<DesignObject>) -> Vec<u64> {
    let group = (stages.len() >= 2).then(|| d.new_group_id());
    stages
        .into_iter()
        .enumerate()
        .map(|(k, mut o)| {
            o.group = group;
            insert_at(d, at + k, o)
        })
        .collect()
}

/// A line-only shape object named `name` over `contours`.
pub(crate) fn stage(
    name: &'static str,
    contours: Vec<Contour>,
    line: openstitch_design::LineSewing,
) -> DesignObject {
    let mut o = DesignObject::shape(contours, None, Some(line));
    o.name = Some(name.into());
    o
}

/// The objects named by `ids` (deduplicated, in sewing order).
pub(crate) fn selected(p: &Params, d: &Design) -> Result<Vec<usize>, CommandError> {
    let mut idx = crate::commands::indexes(p, d)?;
    idx.sort_unstable();
    idx.dedup();
    Ok(idx)
}

/// How far a line's stitches reach to each side of its outline (0.1 mm).
fn line_extent(o: &DesignObject) -> f64 {
    match &o.line {
        Some(l) => match l.kind {
            LineType::Running | LineType::Triple | LineType::Cutting => 0.0,
            LineType::Motif => l.width.max(l.height.unwrap_or(3.0)) * 5.0,
            _ => l.width * 5.0,
        },
        None => 0.0,
    }
}

/// The area the objects at `idx` cover: the outer boundaries of their closed outlines (holes
/// such as letter counters are covered too), widened by the reach of their line sewing.
/// Open lines and stitch patterns count as their bounding rectangle.
pub(crate) fn footprint(d: &Design, idx: &[usize]) -> Vec<Contour> {
    let mut parts: Vec<Vec<Contour>> = Vec::new();
    for &i in idx {
        let o = &d.objects[i];
        let mut part: Vec<Contour> = Vec::new();
        if o.kind == ObjectKind::Stitches {
            part.extend(o.bounds().map(|b| rect(b, 5.0)));
        } else {
            let closed: Vec<Contour> = o.contours.iter().filter(|c| c.closed).cloned().collect();
            part.extend(outer(&boolean::simplify(&closed)));
            let open = o.contours.iter().filter(|c| !c.closed);
            part.extend(
                Rect::enclosing(open.flat_map(Contour::outline_points)).map(|b| rect(b, 5.0)),
            );
        }
        let extent = line_extent(o);
        if extent > 0.0 && !part.is_empty() {
            part = boolean::contours(&boolean::offset(&part, extent, true));
        }
        if !part.is_empty() {
            parts.push(part);
        }
    }
    outer(&boolean::union_all(parts.iter().map(|p| &p[..])))
}

/// The outer boundary of each region (holes dropped).
pub(crate) fn outer(regions: &[Region]) -> Vec<Contour> {
    regions.iter().filter_map(|r| r.first().cloned()).collect()
}

/// `b` grown by `margin` on every side, at least 1 mm wide and high, as a closed contour.
fn rect(b: Rect, margin: f64) -> Contour {
    Contour::rectangle(
        b.min_x - margin,
        b.min_y - margin,
        b.width() + 2.0 * margin,
        b.height() + 2.0 * margin,
    )
}

/// Inserts `object` with a fresh id at sewing position `at`.
pub(crate) fn insert_at(d: &mut Design, at: usize, mut object: DesignObject) -> u64 {
    object.id = d.next_object_id;
    d.next_object_id += 1;
    let id = object.id;
    d.objects.insert(at.min(d.objects.len()), object);
    id
}

/// Whether the straight segment `a`–`b` stays inside `region` (even-odd contours).
fn segment_inside(a: Vec2, b: Vec2, region: &[Contour]) -> bool {
    let pieces = paths::clip(&[a, b], region, &[]);
    pieces.len() == 1 && pieces[0].len() == 2 && {
        let p = &pieces[0];
        p[0].distance(a) < 0.5 && p[1].distance(b) < 0.5
    } || (a.distance(b) < 0.5 && contains(a, region))
}

/// Orders `pieces` greedily and joins each to the next with a straight run where that run is no
/// longer than `max_join` and stays inside `region`, so the lines sew without jumps; pieces that
/// cannot be joined start a new open contour.
pub(crate) fn link(pieces: Vec<Vec<Vec2>>, region: &[Contour], max_join: f64) -> Vec<Contour> {
    let mut out: Vec<Vec<Vec2>> = Vec::new();
    for piece in paths::order(pieces, None) {
        match out.last_mut() {
            Some(last)
                if last.last().expect("non-empty").distance(piece[0]) <= max_join
                    && segment_inside(*last.last().expect("non-empty"), piece[0], region) =>
            {
                last.extend(piece);
            }
            _ => out.push(piece),
        }
    }
    out.into_iter()
        .filter(|p| p.len() >= 2)
        .map(|p| Contour::new(p, false))
        .collect()
}

/// `region` minus `minus`, or `region` itself when `minus` is empty.
pub(crate) fn difference(region: &[Contour], minus: &[Contour]) -> Vec<Contour> {
    if minus.is_empty() {
        return boolean::contours(&boolean::simplify(region));
    }
    boolean::contours(&boolean::combine(region, minus, BoolOp::Difference))
}

/// The colour of a pattern: its region thread, else its line thread.
pub(crate) fn pattern_colour(o: &DesignObject) -> Thread {
    o.region
        .as_ref()
        .map(|r| r.thread.clone())
        .or_else(|| o.line.as_ref().map(|l| l.thread.clone()))
        .unwrap_or_else(crate::commands::default_line)
}
