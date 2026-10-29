//! Sewing-order editing (PE-DESIGN's "Checking and Editing the Sewing Order"): move objects
//! to the start or end or one place earlier or later, sort by colour so same-colour objects
//! sew together, and Optimize Sewing Order in two modes: PE-DESIGN 11.50's (the default,
//! pinned by references) and OpenStitch's shortest-jump arrangement (greedy nearest, then
//! 2-opt), chosen per design (`optimizeOrder`) or per call (`mode`).

use crate::commands::{CommandSpec, Effect, indexes, opt, req, spec};
use openstitch_design::boolean::{self, BoolOp};
use openstitch_design::{Design, DesignObject, ObjectKind, OptimizeOrder, Vec2};
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
            "Optimise the sewing order. peDesign (PE-DESIGN 11.50): same colours together, in drawing order within a colour, but an object never moves before one it overlaps that was sewn earlier; then each shape's entry and exit points move to where it comes closest to the same-colour shapes sewn before and after it. minJump (beyond PE-DESIGN): the same colour blocks, each arranged for the shortest total travel between outlines (greedy nearest, then 2-opt), and every neighbouring pair linked",
            Effect::Edit,
            vec![opt(
                "mode",
                "peDesign|minJump",
                "How to arrange the objects (default: the design's optimizeOrder setting, PE-DESIGN's unless changed with design.setProperties)",
            )],
            |s, p| {
                let mode = if p.has("mode") {
                    let m = p.string("mode", None)?;
                    OptimizeOrder::from_id(&m)
                        .ok_or_else(|| p.invalid("mode", "must be peDesign or minJump"))?
                } else {
                    s.design.optimize_order
                };
                let order = optimized_with(&s.design, mode);
                let mut objects = std::mem::take(&mut s.design.objects);
                let mut slots: Vec<Option<DesignObject>> = objects.drain(..).map(Some).collect();
                s.design.objects = order
                    .iter()
                    .map(|&i| slots[i].take().expect("each index once"))
                    .collect();
                let n = s.design.objects.len();
                link_entry_exit_with(&mut s.design, 0..n, mode == OptimizeOrder::PeDesign);
                Ok(json!({ "order": ids(&s.design), "mode": mode.id() }))
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

/// New order (indexes into `d.objects`): keeps the current colour while an object of it is free
/// to sew, else takes the earliest free object; within a colour the drawing order stays, as in
/// PE-DESIGN 11, which groups colours but does not sort same-colour objects by distance. An
/// object is free once every earlier object it overlaps has been placed.
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
        let next = same.first().copied().unwrap_or(free[0]);
        placed[next] = true;
        out.push(next);
    }
    out
}

/// The order for `mode` (indexes into `d.objects`).
pub fn optimized_with(d: &Design, mode: OptimizeOrder) -> Vec<usize> {
    match mode {
        OptimizeOrder::PeDesign => optimized(d),
        OptimizeOrder::MinJump => min_jump(d, true),
    }
}

/// Above this many objects in one colour block, travel is measured between bounding-box
/// centres instead of outlines, and 2-opt is skipped (greedy only), to stay fast.
const MIN_JUMP_EXACT_LIMIT: usize = 400;

/// Sweeps of the 2-opt search before it gives up (each sweep tries every reversal once).
const MAX_SWEEPS: usize = 500;

/// Shortest-jump order: PE-DESIGN's colour blocks (same colours together in order of first
/// use), each block arranged for the least total travel between the outlines of consecutive
/// objects. The block starts next to the previous block's last object; within it a greedy
/// nearest-free walk gives the first order and, with `two_opt`, 2-opt reversals improve it
/// until none shortens the travel. Overlapping objects keep their drawing order throughout.
/// Without `two_opt` the greedy order is returned (the baseline the search never loses to).
pub fn min_jump(d: &Design, two_opt: bool) -> Vec<usize> {
    let pe = optimized(d);
    let colour = |i: usize| crate::drawing::first_colour(&d.objects[i]);
    let exact = d.objects.len() <= EXACT_OVERLAP_LIMIT;
    let mut out: Vec<usize> = Vec::with_capacity(pe.len());
    let mut start = 0;
    while start < pe.len() {
        let mut end = start + 1;
        while end < pe.len() && colour(pe[end]) == colour(pe[start]) {
            end += 1;
        }
        let prev = out.last().copied();
        out.extend(order_block(d, &pe[start..end], prev, exact, two_opt));
        start = end;
    }
    out
}

/// Travel between two objects: the shortest distance between their outlines (0 when they
/// touch or overlap); between bounding-box centres when one has no outline to link, or
/// when `coarse`.
fn travel_cost(a: &DesignObject, b: &DesignObject, coarse: bool) -> f64 {
    if !coarse && let (Some(ea), Some(eb)) = (ends(a), ends(b)) {
        return closest_pair(&ea.edges, &eb.edges).map_or(0.0, |(p, q)| p.distance(q));
    }
    match (a.bounds(), b.bounds()) {
        (Some(ra), Some(rb)) => ra.center().distance(rb.center()),
        _ => 0.0,
    }
}

/// Total travel of `order` (indexes into `d.objects`) between consecutive objects, 0.1 mm:
/// the sum of the outline-to-outline distances, colour changes included.
pub fn total_travel(d: &Design, order: &[usize]) -> f64 {
    let coarse = order.len() > MIN_JUMP_EXACT_LIMIT;
    order
        .windows(2)
        .map(|w| travel_cost(&d.objects[w[0]], &d.objects[w[1]], coarse))
        .sum()
}

/// Symmetric travel costs between the members of a block (the upper triangle, row-major).
struct Costs {
    k: usize,
    upper: Vec<f64>,
}

impl Costs {
    fn new(k: usize, cost: impl Fn(usize, usize) -> f64) -> Self {
        let upper = (0..k)
            .flat_map(|i| (0..k).map(move |j| (i, j)))
            .map(|(i, j)| if i < j { cost(i, j) } else { 0.0 })
            .collect();
        Costs { k, upper }
    }

    fn get(&self, i: usize, j: usize) -> f64 {
        let (a, b) = if i < j { (i, j) } else { (j, i) };
        self.upper[a * self.k + b]
    }
}

/// Arranges one same-colour block (indexes into `d.objects`, in PE-DESIGN order) for the
/// least travel, starting from object `prev` when there is one.
fn order_block(
    d: &Design,
    block: &[usize],
    prev: Option<usize>,
    exact_overlap: bool,
    two_opt: bool,
) -> Vec<usize> {
    let k = block.len();
    if k <= 1 {
        return block.to_vec();
    }
    let coarse = k > MIN_JUMP_EXACT_LIMIT;
    let obj = |i: usize| &d.objects[block[i]];
    let cost = Costs::new(k, |i, j| travel_cost(obj(i), obj(j), coarse));
    let from_prev: Vec<f64> = match prev {
        Some(p) => (0..k)
            .map(|i| travel_cost(&d.objects[p], obj(i), coarse))
            .collect(),
        None => vec![0.0; k],
    };
    // `before[j]` lists the block members that must sew before member `j` (drawn earlier and
    // overlapping it); `related[i][j]` says the two may not change their relative order.
    let mut related = vec![vec![false; k]; k];
    let mut before: Vec<Vec<usize>> = vec![Vec::new(); k];
    for i in 0..k {
        for j in 0..k {
            if block[i] < block[j] && overlaps(obj(i), obj(j), exact_overlap) {
                before[j].push(i);
                related[i][j] = true;
                related[j][i] = true;
            }
        }
    }
    // Greedy: from the previous object, the nearest member free to sew.
    let mut placed = vec![false; k];
    let mut tour: Vec<usize> = Vec::with_capacity(k);
    while tour.len() < k {
        let last = tour.last().copied();
        let c = |j: usize| last.map_or(from_prev[j], |l| cost.get(l, j));
        let next = (0..k)
            .filter(|&j| !placed[j] && before[j].iter().all(|&i| placed[i]))
            .min_by(|&a, &b| c(a).total_cmp(&c(b)))
            .expect("a free member exists: constraints follow the drawing order");
        placed[next] = true;
        tour.push(next);
    }
    if two_opt && !coarse {
        two_opt_improve(&mut tour, &cost, &from_prev, &related);
    }
    tour.into_iter().map(|i| block[i]).collect()
}

/// 2-opt on an open path with a fixed start: reversing `tour[i..=j]` replaces the travel into
/// `tour[i]` and out of `tour[j]`; the first reversal that shortens the total is taken and
/// the sweep restarts, until a whole sweep finds none (or `MAX_SWEEPS` pass). A reversal is
/// allowed only when no two members of the segment must keep their order.
fn two_opt_improve(tour: &mut [usize], cost: &Costs, from_prev: &[f64], related: &[Vec<bool>]) {
    let k = tour.len();
    let into = |prev: Option<usize>, j: usize| prev.map_or(from_prev[j], |p| cost.get(p, j));
    for _ in 0..MAX_SWEEPS {
        let mut improved = false;
        'sweep: for i in 0..k - 1 {
            let prev = (i > 0).then(|| tour[i - 1]);
            for j in i + 1..k {
                if (i..j).any(|m| related[tour[m]][tour[j]]) {
                    // Nothing longer starting at `i` can be reversed either.
                    break;
                }
                let after = tour.get(j + 1).copied();
                let old = into(prev, tour[i]) + after.map_or(0.0, |a| cost.get(tour[j], a));
                let new = into(prev, tour[j]) + after.map_or(0.0, |a| cost.get(tour[i], a));
                if new < old - 1e-9 {
                    tour[i..=j].reverse();
                    improved = true;
                    break 'sweep;
                }
            }
        }
        if !improved {
            return;
        }
    }
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
/// sewn after it in the same colour, the exit of `k` and the entry of the next move to where
/// the two outlines come closest, so the travel between them is the gap between their
/// outlines. A colour change between them leaves both points alone, as in PE-DESIGN 11. Text,
/// stitch patterns and hidden objects are left alone.
pub fn link_entry_exit(d: &mut Design, pairs: std::ops::Range<usize>) {
    link_entry_exit_with(d, pairs, true);
}

/// [`link_entry_exit`], linking across colour changes too when `same_colour_only` is off
/// (the shortest-jump mode).
pub fn link_entry_exit_with(d: &mut Design, pairs: std::ops::Range<usize>, same_colour_only: bool) {
    let n = d.objects.len();
    for k in pairs {
        if k + 1 >= n {
            break;
        }
        if same_colour_only
            && crate::drawing::first_colour(&d.objects[k])
                != crate::drawing::first_colour(&d.objects[k + 1])
        {
            continue;
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
