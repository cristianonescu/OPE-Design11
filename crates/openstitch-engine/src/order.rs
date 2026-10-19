//! Sewing-order editing (PE-DESIGN's "Checking and Editing the Sewing Order"): move objects
//! to the start or end or one place earlier or later, and sort by colour so same-colour
//! objects sew together.

use crate::commands::{CommandSpec, Effect, indexes, opt, req, spec};
use openstitch_design::boolean::{self, BoolOp};
use openstitch_design::{Design, DesignObject, Rect};
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
            "Optimise the sewing order: same colours together, nearest object next, but an object never moves before one it overlaps that was sewn earlier",
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
