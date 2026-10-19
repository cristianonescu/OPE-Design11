//! Sewing-order editing (PE-DESIGN's "Checking and Editing the Sewing Order"): move objects
//! to the start or end or one place earlier or later, and sort by colour so same-colour
//! objects sew together.

use crate::commands::{CommandSpec, Effect, indexes, opt, req, spec};
use openstitch_design::boolean::{self, BoolOp};
use openstitch_design::{Design, DesignObject, ObjectKind, Rect, Vec2};
use serde_json::json;

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "order.move",
            "Move objects in the sewing order: to the start or end, one place earlier or later, or as a block before / after another object (they keep their relative order)",
            Effect::Edit,
            vec![
                req("ids", "[int] | id: int", "Objects to move"),
                req("to", "first|last|earlier|later|before|after", "Where to"),
                opt(
                    "target",
                    "int",
                    "before / after: the object to move next to (not one of ids)",
                ),
            ],
            |s, p| {
                let mut idx = indexes(p, &s.design)?;
                idx.sort_unstable();
                idx.dedup();
                let to = p.string("to", None)?;
                let d = &mut s.design;
                match to.as_str() {
                    "before" | "after" => {
                        let target = p.int("target", None, 0, i64::MAX)? as u64;
                        let t = d
                            .index_of(target)
                            .ok_or_else(|| p.not_found(format!("object {target}")))?;
                        if idx.contains(&t) {
                            return Err(p.invalid("target", "must not be one of the moved objects"));
                        }
                        let (mut moved, mut rest) = (Vec::new(), Vec::new());
                        for (i, o) in std::mem::take(&mut d.objects).into_iter().enumerate() {
                            if idx.contains(&i) {
                                moved.push(o);
                            } else {
                                rest.push(o);
                            }
                        }
                        let at = rest.iter().position(|o| o.id == target).expect("kept")
                            + usize::from(to == "after");
                        rest.splice(at..at, moved);
                        d.objects = rest;
                    }
                    "first" | "last" => {
                        let (mut moved, mut rest) = (Vec::new(), Vec::new());
                        for (i, o) in std::mem::take(&mut d.objects).into_iter().enumerate() {
                            if idx.contains(&i) {
                                moved.push(o);
                            } else {
                                rest.push(o);
                            }
                        }
                        d.objects = if to == "first" {
                            moved.into_iter().chain(rest).collect()
                        } else {
                            rest.into_iter().chain(moved).collect()
                        };
                    }
                    "earlier" => {
                        // Each selected object swaps with the unselected one before it.
                        for k in 0..idx.len() {
                            let i = idx[k];
                            if i > 0 && !idx[..k].contains(&(i - 1)) {
                                d.objects.swap(i - 1, i);
                                idx[k] = i - 1;
                            }
                        }
                    }
                    "later" => {
                        let n = d.objects.len();
                        for k in (0..idx.len()).rev() {
                            let i = idx[k];
                            if i + 1 < n && !idx[k + 1..].contains(&(i + 1)) {
                                d.objects.swap(i, i + 1);
                                idx[k] = i + 1;
                            }
                        }
                    }
                    _ => {
                        return Err(p.invalid(
                            "to",
                            "must be first, last, earlier, later, before or after",
                        ));
                    }
                }
                Ok(json!({ "order": ids(d) }))
            },
        ),
        spec(
            "order.optimize",
            "Optimise the sewing order: same colours together, nearest object next, but an object never moves before one it overlaps that was sewn earlier; then each shape's entry and exit points move to where it comes closest to the shapes sewn before and after it",
            Effect::Edit,
            vec![],
            |s, _| {
                let order = optimized(&s.design);
                let mut objects = std::mem::take(&mut s.design.objects);
                let mut slots: Vec<Option<DesignObject>> = objects.drain(..).map(Some).collect();
                s.design.objects = order
                    .iter()
                    .map(|&i| slots[i].take().expect("each index once"))
                    .collect();
                let n = s.design.objects.len();
                link_entry_exit(&mut s.design, 0..n);
                Ok(json!({ "order": ids(&s.design) }))
            },
        ),
        spec(
            "order.byColor",
            "Sort by colour so same-colour objects sew together (colours in order of first use; stable)",
            Effect::Edit,
            vec![opt(
                "ids",
                "[int]",
                "Only reorder these objects among their own places (default all)",
            )],
            |s, p| {
                let chosen: Vec<u64> = if p.has("ids") || p.has("id") {
                    indexes(p, &s.design)?
                        .into_iter()
                        .map(|i| s.design.objects[i].id)
                        .collect()
                } else {
                    s.design.objects.iter().map(|o| o.id).collect()
                };
                crate::drawing::group_by_colour(&mut s.design, &chosen);
                Ok(json!({ "order": ids(&s.design) }))
            },
        ),
    ]
}

fn ids(d: &Design) -> Vec<u64> {
    d.objects.iter().map(|o| o.id).collect()
}

/// Above this many objects, overlap is judged by bounding boxes only.
const EXACT_OVERLAP_LIMIT: usize = 1_000;

/// Whether `a` and `b` overlap: bounding boxes, refined with the regions when both are closed
/// outlines.
fn overlaps(a: &DesignObject, b: &DesignObject, exact: bool) -> bool {
    let (Some(ra), Some(rb)) = (a.bounds(), b.bounds()) else {
        return false;
    };
    let boxes = ra.min_x <= rb.max_x
        && rb.min_x <= ra.max_x
        && ra.min_y <= rb.max_y
        && rb.min_y <= ra.max_y;
    if !boxes || !exact {
        return boxes;
    }
    let closed = |o: &DesignObject| {
        o.is_outline() && o.region.is_some() && o.contours.iter().all(|c| c.closed)
    };
    if !closed(a) || !closed(b) {
        return true;
    }
    !boolean::combine(&a.contours, &b.contours, BoolOp::Intersection).is_empty()
}

fn centre(o: &DesignObject) -> Option<openstitch_design::Vec2> {
    o.bounds().map(|b: Rect| b.center())
}

/// New order (indexes into `d.objects`): greedy, keeping the current colour while an object of
/// it is free to sew (nearest first), else the earliest free object. An object is free once
/// every earlier object it overlaps has been placed.
pub fn optimized(d: &Design) -> Vec<usize> {
    let n = d.objects.len();
    let exact = n <= EXACT_OVERLAP_LIMIT;
    let before: Vec<Vec<usize>> = (0..n)
        .map(|j| {
            (0..j)
                .filter(|&i| overlaps(&d.objects[i], &d.objects[j], exact))
                .collect()
        })
        .collect();
    let colour = |i: usize| crate::drawing::first_colour(&d.objects[i]);
    let mut placed = vec![false; n];
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let free: Vec<usize> = (0..n)
            .filter(|&j| !placed[j] && before[j].iter().all(|&i| placed[i]))
            .collect();
        let last = out.last().copied();
        let same: Vec<usize> = match last {
            Some(l) => free
                .iter()
                .copied()
                .filter(|&j| colour(j) == colour(l))
                .collect(),
            None => Vec::new(),
        };
        let next = match (last.and_then(|l| centre(&d.objects[l])), same.is_empty()) {
            (Some(at), false) => same
                .iter()
                .copied()
                .min_by(|&a, &b| {
                    let da = centre(&d.objects[a]).map_or(f64::MAX, |c| c.distance(at));
                    let db = centre(&d.objects[b]).map_or(f64::MAX, |c| c.distance(at));
                    da.total_cmp(&db).then(a.cmp(&b))
                })
                .expect("non-empty"),
            (None, false) => same[0],
            _ => free[0],
        };
        placed[next] = true;
        out.push(next);
    }
    out
}

/// Where a shape can be entered and left: outline segments (each a pair of points), and
/// whether its exit can be chosen freely. A region can be left anywhere on its outline (its
/// exit point, R2, and the start of an outline sewn after it); a line alone ends where its
/// entry point (L1) puts the end. `None` for text, stitch patterns and hidden objects.
struct Ends {
    edges: Vec<(Vec2, Vec2)>,
    free_exit: bool,
}

fn ends(o: &DesignObject) -> Option<Ends> {
    if o.kind != ObjectKind::Shape || !o.visible {
        return None;
    }
    if o.region.is_some() && o.has_closed_contour() {
        let edges = o
            .contours
            .iter()
            .filter(|c| c.closed)
            .flat_map(|c| c.edges())
            .collect();
        return Some(Ends {
            edges,
            free_exit: true,
        });
    }
    o.line.as_ref()?;
    // An open line is entered at one of its ends.
    let edges = o
        .contours
        .iter()
        .flat_map(|c| {
            if c.closed {
                c.edges()
            } else {
                let p = c.path();
                match (p.first(), p.last()) {
                    (Some(&a), Some(&b)) => vec![(a, a), (b, b)],
                    _ => Vec::new(),
                }
            }
        })
        .collect();
    Some(Ends {
        edges,
        free_exit: false,
    })
}

fn nearest_on(p: Vec2, (a, b): (Vec2, Vec2)) -> Vec2 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 == 0.0 {
        return a;
    }
    let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
    a.lerp(b, t)
}

/// Where segments `a` and `b` cross, if they do.
fn crossing((a, b): (Vec2, Vec2), (c, d): (Vec2, Vec2)) -> Option<Vec2> {
    let (r, s) = (b - a, d - c);
    let den = r.x * s.y - r.y * s.x;
    if den == 0.0 {
        return None;
    }
    let (ca_x, ca_y) = (c.x - a.x, c.y - a.y);
    let t = (ca_x * s.y - ca_y * s.x) / den;
    let u = (ca_x * r.y - ca_y * r.x) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a.lerp(b, t))
}

/// The closest pair of points between two sets of segments: (on `a`, on `b`).
fn closest_pair(a: &[(Vec2, Vec2)], b: &[(Vec2, Vec2)]) -> Option<(Vec2, Vec2)> {
    let mut best: Option<(f64, Vec2, Vec2)> = None;
    let mut consider = |pa: Vec2, pb: Vec2| {
        let d = pa.distance(pb);
        if best.is_none_or(|(e, _, _)| d < e) {
            best = Some((d, pa, pb));
        }
    };
    for &ea in a {
        for &eb in b {
            if let Some(x) = crossing(ea, eb) {
                consider(x, x);
                continue;
            }
            consider(ea.0, nearest_on(ea.0, eb));
            consider(ea.1, nearest_on(ea.1, eb));
            consider(nearest_on(eb.0, ea), eb.0);
            consider(nearest_on(eb.1, ea), eb.1);
        }
    }
    best.map(|(_, pa, pb)| (pa, pb))
}

/// The point of `edges` nearest to `p`.
fn nearest_point(edges: &[(Vec2, Vec2)], p: Vec2) -> Option<Vec2> {
    edges
        .iter()
        .map(|&e| nearest_on(p, e))
        .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
}

/// Where a line-only shape ends: a closed outline where it starts, an open line at the end
/// away from its entry point.
fn line_end(o: &DesignObject) -> Option<Vec2> {
    let entry = o.line.as_ref()?.entry;
    let c = o.contours.last()?;
    let p = c.path();
    let (first, last) = (*p.first()?, *p.last()?);
    if c.closed {
        return Some(entry.and_then(|e| c.nearest(e)).map_or(first, |n| n.point));
    }
    Some(match entry {
        Some(e) if last.distance(e) < first.distance(e) => first,
        _ => last,
    })
}

fn set_entry(o: &mut DesignObject, p: Vec2) {
    match (&mut o.region, &mut o.line) {
        (Some(r), _) => r.entry = Some(p),
        (None, Some(l)) => l.entry = Some(p),
        (None, None) => {}
    }
}

/// A region's exit point; an outline sewn after the region starts (and ends) there too.
fn set_exit(o: &mut DesignObject, p: Vec2) {
    if let Some(r) = &mut o.region {
        r.exit = Some(p);
    }
    if let Some(l) = &mut o.line {
        l.entry = Some(p);
    }
}

/// PE-DESIGN's optimised entry and exit points: for each object `k` in `pairs` and the one
/// sewn after it, the exit of `k` and the entry of the next move to where the two outlines
/// come closest, so the travel between them is the gap between their outlines. Text, stitch
/// patterns and hidden objects are left alone.
pub fn link_entry_exit(d: &mut Design, pairs: std::ops::Range<usize>) {
    let n = d.objects.len();
    for k in pairs {
        if k + 1 >= n {
            break;
        }
        let (Some(a), Some(b)) = (ends(&d.objects[k]), ends(&d.objects[k + 1])) else {
            continue;
        };
        let (exit, entry) = if a.free_exit {
            match closest_pair(&a.edges, &b.edges) {
                Some(pair) => pair,
                None => continue,
            }
        } else {
            let Some(end) = line_end(&d.objects[k]) else {
                continue;
            };
            match nearest_point(&b.edges, end) {
                Some(entry) => (end, entry),
                None => continue,
            }
        };
        if a.free_exit {
            set_exit(&mut d.objects[k], exit);
        }
        set_entry(&mut d.objects[k + 1], entry);
    }
}
