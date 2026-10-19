//! Shape operations on closed outlines: PE-DESIGN's "Changing shapes of overlapped patterns"
//! (merge, remove overlap), general boolean combination, "Splitting Shape pattern" and
//! "Creating an offset line pattern". Geometry lives in `openstitch_design::boolean`.

use crate::commands::{CommandSpec, Effect, IDS, MM, default_line, indexes, opt, req, spec};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_design::boolean::{self, BoolOp, Region};
use openstitch_design::{Contour, Design, DesignObject, LineSewing, LineType, ObjectKind, Vec2};
use serde_json::json;

/// Most vertices a cut outline may have.
const MAX_CUT_POINTS: usize = 2_000;

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "shape.merge",
            "Merge overlapping shapes into one (takes the sewing attributes and place of the last-sewn shape)",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let idx = ordered(s, p, 2)?;
                let merged =
                    boolean::union_all(idx.iter().map(|&i| &s.design.objects[i].contours[..]));
                if merged.len() != 1 {
                    return Err(p.failed("the shapes do not all overlap; they cannot be merged"));
                }
                let top = *idx.last().expect("2 or more");
                set_region(&mut s.design.objects[top], &merged);
                let keep = s.design.objects[top].id;
                remove(&mut s.design, &idx[..idx.len() - 1]);
                Ok(json!({ "id": keep }))
            },
        ),
        spec(
            "shape.removeOverlap",
            "Cut away the parts of each shape covered by selected shapes sewn after it",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let idx = ordered(s, p, 2)?;
                let mut changed = Vec::new();
                let mut emptied = Vec::new();
                for (k, &i) in idx.iter().enumerate() {
                    let above = boolean::contours(&boolean::union_all(
                        idx[k + 1..]
                            .iter()
                            .map(|&j| &s.design.objects[j].contours[..]),
                    ));
                    if above.is_empty() {
                        continue;
                    }
                    let o = &s.design.objects[i];
                    let before: f64 = boolean::simplify(&o.contours)
                        .iter()
                        .map(|r| boolean::region_area(r))
                        .sum();
                    let rest = boolean::combine(&o.contours, &above, BoolOp::Difference);
                    let after: f64 = rest.iter().map(|r| boolean::region_area(r)).sum();
                    if (before - after).abs() < 1.0 {
                        continue;
                    }
                    if rest.is_empty() {
                        emptied.push(i);
                    } else {
                        changed.push(o.id);
                        set_region(&mut s.design.objects[i], &rest);
                    }
                }
                if changed.is_empty() && emptied.is_empty() {
                    return Err(p.failed("the shapes do not overlap"));
                }
                let removed: Vec<u64> = emptied.iter().map(|&i| s.design.objects[i].id).collect();
                remove(&mut s.design, &emptied);
                Ok(json!({ "changed": changed, "removed": removed }))
            },
        ),
        spec(
            "shape.combine",
            "Boolean combination: the first listed shape op the union of the others; the result replaces the first",
            Effect::Edit,
            vec![
                req("ids", "[int]", "Subject first, then the operands"),
                req("op", "union|difference|intersection|xor", "Operation"),
                opt(
                    "keepOperands",
                    "bool",
                    "Keep the other shapes instead of deleting them (default false)",
                ),
            ],
            |s, p| {
                let op = match p.string("op", None)?.as_str() {
                    "union" => BoolOp::Union,
                    "difference" => BoolOp::Difference,
                    "intersection" => BoolOp::Intersection,
                    "xor" => BoolOp::Xor,
                    _ => {
                        return Err(
                            p.invalid("op", "must be union, difference, intersection or xor")
                        );
                    }
                };
                let idx = indexes(p, &s.design)?;
                let mut seen = idx.clone();
                seen.sort_unstable();
                seen.dedup();
                if seen.len() != idx.len() || idx.len() < 2 {
                    return Err(p.invalid("ids", "needs at least 2 different objects"));
                }
                for &i in &idx {
                    closed_shape(&s.design, p, i)?;
                }
                let (subject, operands) = (idx[0], &idx[1..]);
                let clip = boolean::contours(&boolean::union_all(
                    operands.iter().map(|&i| &s.design.objects[i].contours[..]),
                ));
                let result = boolean::combine(&s.design.objects[subject].contours, &clip, op);
                if result.is_empty() {
                    return Err(p.failed("the result is empty"));
                }
                set_region(&mut s.design.objects[subject], &result);
                let id = s.design.objects[subject].id;
                if !p.bool("keepOperands", Some(false))? {
                    remove(&mut s.design, operands);
                }
                Ok(json!({ "id": id, "pieces": result.len() }))
            },
        ),
        spec(
            "shape.split",
            "Split a shape along a closed cut outline: the part inside becomes a new shape sewn right after",
            Effect::Edit,
            vec![
                req("id", "int", "Closed shape without hole sewing"),
                req(
                    "points",
                    "[[x, y]] mm",
                    "Cut outline vertices (closed, not self-crossing)",
                ),
            ],
            |s, p| {
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                closed_shape(&s.design, p, i)?;
                if s.design.objects[i].hole_sewing {
                    return Err(p.failed("shapes with hole sewing cannot be split"));
                }
                let points = p.points("points", 3)?;
                if points.len() > MAX_CUT_POINTS {
                    return Err(p.invalid("points", format!("at most {MAX_CUT_POINTS} points")));
                }
                if self_crossing(&points) {
                    return Err(p.invalid("points", "the cut outline must not cross itself"));
                }
                let cut = [Contour::new(points, true)];
                let o = &s.design.objects[i];
                let inside = boolean::combine(&o.contours, &cut, BoolOp::Intersection);
                let outside = boolean::combine(&o.contours, &cut, BoolOp::Difference);
                if inside.is_empty() || outside.is_empty() {
                    return Err(p.failed("the cut outline does not divide the shape"));
                }
                let mut piece = s.design.objects[i].clone();
                piece.name = None;
                set_region(&mut piece, &inside);
                set_region(&mut s.design.objects[i], &outside);
                // Splitting cancels text fitted to this outline.
                for t in 0..s.design.objects.len() {
                    if s.design.objects[t].text.as_ref().and_then(|x| x.path) == Some(id) {
                        s.design.detach_text(t);
                    }
                }
                let new = insert_at(&mut s.design, i + 1, piece);
                Ok(json!({ "id": id, "piece": new }))
            },
        ),
        spec(
            "shape.offsetLines",
            "Create concentric offset lines around (or inside) a closed outline, as a new line shape",
            Effect::Edit,
            vec![
                req("id", "int", "Shape or text with closed outlines"),
                opt(
                    "direction",
                    "outward|inward",
                    "Offset direction (default outward)",
                ),
                opt("spacing", "mm", "Distance between lines (default 2)"),
                opt("count", "int 1-20", "Number of lines (default 1)"),
                opt(
                    "roundedCorners",
                    "bool",
                    "Round the corners (default false)",
                ),
            ],
            |s, p| {
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                let sign = match p.string("direction", Some("outward"))?.as_str() {
                    "outward" => 1.0,
                    "inward" => -1.0,
                    _ => return Err(p.invalid("direction", "must be outward or inward")),
                };
                let spacing = p.opt("spacing", 2.0, 0.1, 100.0)? * MM;
                let count = p.int("count", Some(1), 1, 20)? as usize;
                let rounded = p.bool("roundedCorners", Some(false))?;
                let o = &s.design.objects[i];
                if !o.is_outline() || !o.has_closed_contour() {
                    return Err(p.failed(format!("object {id} has no closed outline")));
                }
                let mut lines = Vec::new();
                for k in 1..=count {
                    let ring = boolean::offset(&o.contours, sign * spacing * k as f64, rounded);
                    if ring.is_empty() {
                        break;
                    }
                    lines.extend(boolean::contours(&ring));
                }
                if lines.is_empty() {
                    return Err(p.failed("the shape is too small for an inward offset"));
                }
                let line = o
                    .line
                    .clone()
                    .unwrap_or_else(|| LineSewing::new(LineType::Running, default_line()));
                let mut object = DesignObject::shape(lines, None, Some(line));
                object.visible = o.visible;
                let new = insert_at(&mut s.design, i + 1, object);
                Ok(json!({ "id": new }))
            },
        ),
    ]
}

/// The objects named by `ids` in sewing order, each a closed shape; at least `minimum`.
fn ordered(s: &EngineState, p: &Params, minimum: usize) -> Result<Vec<usize>, CommandError> {
    let mut idx = indexes(p, &s.design)?;
    idx.sort_unstable();
    idx.dedup();
    if idx.len() < minimum {
        return Err(p.invalid("ids", format!("needs at least {minimum} different objects")));
    }
    for &i in &idx {
        closed_shape(&s.design, p, i)?;
    }
    Ok(idx)
}

/// Shape operations work on shape objects made only of closed outlines.
fn closed_shape(d: &Design, p: &Params, i: usize) -> Result<(), CommandError> {
    let o = &d.objects[i];
    match o.kind {
        ObjectKind::Shape => {}
        ObjectKind::Text => {
            return Err(p.failed(format!(
                "object {} is text; convert it to shapes first (text.toShapes)",
                o.id
            )));
        }
        ObjectKind::Stitches => {
            return Err(p.failed(format!("object {} is a stitch pattern", o.id)));
        }
    }
    if o.contours.is_empty() || o.contours.iter().any(|c| !c.closed) {
        return Err(p.failed(format!(
            "object {} is not made of closed outlines only",
            o.id
        )));
    }
    Ok(())
}

/// Merges overlapping closed shapes among `ids` (each merged pair keeps the later-sewn
/// object). Returns the ids that were merged away.
pub(crate) fn merge_overlapping(d: &mut Design, ids: &[u64]) -> Vec<u64> {
    let mut gone = Vec::new();
    loop {
        let live: Vec<usize> = ids
            .iter()
            .filter_map(|&id| d.index_of(id))
            .filter(|&i| {
                let o = &d.objects[i];
                o.kind == ObjectKind::Shape
                    && !o.contours.is_empty()
                    && o.contours.iter().all(|c| c.closed)
            })
            .collect();
        let mut pair = None;
        'search: for (a, &i) in live.iter().enumerate() {
            for &j in &live[a + 1..] {
                let (lo, hi) = (i.min(j), i.max(j));
                let (x, y) = (&d.objects[lo].contours, &d.objects[hi].contours);
                if !boolean::combine(x, y, BoolOp::Intersection).is_empty() {
                    pair = Some((lo, hi));
                    break 'search;
                }
            }
        }
        let Some((lo, hi)) = pair else {
            return gone;
        };
        let merged = boolean::union_all([&d.objects[lo].contours[..], &d.objects[hi].contours[..]]);
        set_region(&mut d.objects[hi], &merged);
        gone.push(d.objects[lo].id);
        remove(d, &[lo]);
    }
}

/// New outline for `o`. The sewing direction returns to the default, as in PE-DESIGN.
pub(crate) fn set_region(o: &mut DesignObject, regions: &[Region]) {
    o.contours = boolean::contours(regions);
    if let Some(r) = &mut o.region {
        r.angle = 0.0;
    }
}

/// Removes the objects at `idx` (any order).
pub(crate) fn remove(d: &mut Design, idx: &[usize]) {
    let ids: Vec<u64> = idx.iter().map(|&i| d.objects[i].id).collect();
    d.objects.retain(|o| !ids.contains(&o.id));
}

/// Inserts `object` with a fresh id at sewing position `at`.
fn insert_at(d: &mut Design, at: usize, mut object: DesignObject) -> u64 {
    object.id = d.next_object_id;
    d.next_object_id += 1;
    let id = object.id;
    d.objects.insert(at.min(d.objects.len()), object);
    id
}

/// Whether two non-adjacent edges of the closed polygon `pts` cross or touch.
pub(crate) fn self_crossing(pts: &[Vec2]) -> bool {
    let n = pts.len();
    let edge = |i: usize| (pts[i], pts[(i + 1) % n]);
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let ((a, b), (c, d)) = (edge(i), edge(j));
            if segments_meet(a, b, c, d) {
                return true;
            }
        }
    }
    false
}

fn segments_meet(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let cross = |o: Vec2, p: Vec2, q: Vec2| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    let on = |o: Vec2, p: Vec2, q: Vec2| {
        q.x >= o.x.min(p.x) && q.x <= o.x.max(p.x) && q.y >= o.y.min(p.y) && q.y <= o.y.max(p.y)
    };
    ((d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0) && d1 != 0.0 && d2 != 0.0)
        || (d1 == 0.0 && on(c, d, a))
        || (d2 == 0.0 && on(c, d, b))
        || (d3 == 0.0 && on(a, b, c))
        || (d4 == 0.0 && on(a, b, d))
}
