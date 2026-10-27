//! Region sewing with its options: underlay styles, automatic direction, gradation, feathered
//! edges, split satin stitches and travel inside the region. Plain fill and satin without
//! options go straight to [`generate::fill`] / [`generate::satin`], so their output is
//! unchanged. Lengths are 0.1 mm.

use crate::direction;
use crate::generate::{self, RegionParams, RowSpan, Run, subdivide};
use crate::geometry::{Contour, Rect, Span, Vec2, contains, subtract, v2};
use crate::model::{
    Feather, FeatherSide, Gradation, RegionSewing, RegionType, RunningStitchPath, UnderlayStyle,
};
use crate::paths::{OutlineTravel, Rng, TravelGraph, join};
use crate::patterns::Pattern;
use crate::regions;

/// Inset of underlay rows from the outline (0.1 mm).
const UNDERLAY_INSET: f64 = 5.0;

/// Inset of satin under sewing (outline and rows) from the outline (0.1 mm), as PE-DESIGN 11
/// sews it; on a column too narrow for it, the outline is sewn 0.9 mm in.
const SATIN_UNDERLAY_INSETS: [f64; 2] = [10.0, 9.0];

/// Running stitch of the travel from satin under sewing to the top stitching (0.1 mm).
const SATIN_TRAVEL_PITCH: f64 = 20.0;

/// The layers of a region in sewing order (underlay first, then the top stitching); each layer
/// is a list of runs.
pub fn layers(closed: &[Contour], r: &RegionSewing, holes: &[Contour]) -> Vec<Vec<Run>> {
    layers_with(closed, r, holes, &[])
}

/// [`layers`] with the design's own (user) patterns available by id.
pub fn layers_with(
    closed: &[Contour],
    r: &RegionSewing,
    holes: &[Contour],
    user: &[Pattern],
) -> Vec<Vec<Run>> {
    let mut out = layers_inner(closed, r, holes, user);
    if r.kind == RegionType::Satin && r.underlay {
        link_underlay(&mut out, closed, holes);
    }
    out
}

/// Satin: the under sewing layers become one run that travels inside the region from layer
/// to layer and on to the first needle point of the top stitching, so the object is sewn
/// without a jump (PE-DESIGN 11 sews satin this way).
fn link_underlay(out: &mut Vec<Vec<Run>>, closed: &[Contour], holes: &[Contour]) {
    if out.len() < 2 {
        return;
    }
    let top = out.pop().expect("two layers");
    let mut pieces: Vec<Run> = out.drain(..).flatten().filter(|r| !r.is_empty()).collect();
    // Between under sewing runs the travel follows the inset outline, where PE-DESIGN sews
    // it too; the rest (and the way to the top stitching) is the shortest path inside.
    if let Some(inner) = satin_inset(closed, holes) {
        // Along the inset outline only where that is not a long way round (between the U's
        // bar and a leg the outline goes round the whole leg).
        let mut graph = TravelGraph::new(&inner, &[]);
        graph.along_edges = true;
        pieces = join(pieces, Some(&mut graph), SATIN_TRAVEL_PITCH);
    }
    if let (false, Some(start)) = (
        pieces.is_empty(),
        top.iter().find_map(|r| r.first().copied()),
    ) {
        pieces.push(vec![start]);
    }
    let mut graph = TravelGraph::new(closed, holes);
    let mut linked: Vec<Run> = join(pieces, Some(&mut graph), SATIN_TRAVEL_PITCH)
        .into_iter()
        .filter(|r| r.len() > 1)
        .collect();
    // Subdividing the travel can miss its end by a rounding error; the top stitching must
    // begin exactly where the under sewing stops, or the writer moves there with a jump.
    if let (Some(end), Some(start)) = (
        linked.last_mut().and_then(|r| r.last_mut()),
        top.iter().find_map(|r| r.first().copied()),
    ) && end.distance(start) < 1e-6
    {
        *end = start;
    }
    out.push(linked);
    out.push(top);
}

/// The outline of satin under sewing: the region (minus its holes) moved 1 mm inwards, or
/// 0.9 mm where 1 mm leaves nothing; `None` for a region narrower than that.
fn satin_inset(closed: &[Contour], holes: &[Contour]) -> Option<Vec<Contour>> {
    let all: Vec<Contour> = closed.iter().chain(holes).cloned().collect();
    SATIN_UNDERLAY_INSETS
        .iter()
        .map(|&d| crate::boolean::contours(&crate::boolean::offset(&all, -d, false)))
        .find(|c| !c.is_empty())
}

fn layers_inner(
    closed: &[Contour],
    r: &RegionSewing,
    holes: &[Contour],
    user: &[Pattern],
) -> Vec<Vec<Run>> {
    // Skeleton: the top stitching is a satin column along the centre line.
    if let Some(skeleton) = &r.skeleton
        && r.kind == RegionType::Satin
    {
        let mut plain = r.clone();
        plain.skeleton = None;
        let mut out = layers_inner(closed, &plain, holes, user);
        out.pop();
        let spacing = 10.0 / r.density.max(0.1);
        out.push(vec![crate::skeleton::satin(
            skeleton,
            spacing,
            r.pull_compensation * 10.0,
        )]);
        return out;
    }
    // Curved direction: sew the straightened region with horizontal rows, then bend back.
    if let Some(curve) = &r.direction_curve
        && direction::CURVE_TYPES.contains(&r.kind)
        && let Ok(w) = direction::Warp::new(curve)
    {
        let mut flat = r.clone();
        flat.direction_curve = None;
        flat.angle = 0.0;
        flat.auto_direction = false;
        flat.entry = r.entry.map(|p| w.forward(p));
        flat.reference = r.reference.map(|p| w.forward(p));
        let (c, h) = (w.forward_contours(closed), w.forward_contours(holes));
        return layers(&c, &flat, &h)
            .into_iter()
            .map(|l| w.back_runs(l))
            .collect();
    }
    // Reference point: the pattern grid starts there.
    if let Some(at) = r.reference
        && direction::REFERENCE_TYPES.contains(&r.kind)
    {
        let mut local = r.clone();
        local.reference = None;
        local.entry = r.entry.map(|p| p - at);
        local.center = r.center.map(|p| p - at);
        let back = |l: Vec<Run>| -> Vec<Run> {
            l.into_iter()
                .map(|run| run.into_iter().map(|p| p + at).collect())
                .collect()
        };
        return layers(
            &direction::shifted(closed, v2(-at.x, -at.y)),
            &local,
            &direction::shifted(holes, v2(-at.x, -at.y)),
        )
        .into_iter()
        .map(back)
        .collect();
    }
    // Several direction lines: the top stitching follows the column they make.
    if r.direction_lines.len() >= 2 && matches!(r.kind, RegionType::Satin | RegionType::Fill) {
        let mut plain = r.clone();
        plain.direction_lines.clear();
        let mut out = layers_inner(closed, &plain, holes, &[]);
        out.pop();
        let spacing = 10.0 / r.density.max(0.1);
        let pull = r.pull_compensation * 10.0;
        let lines = if r.curved_lines {
            crate::column::curved_rungs(&r.direction_lines)
        } else {
            r.direction_lines.clone()
        };
        out.push(vec![match r.kind {
            RegionType::Satin => crate::column::satin(&lines, spacing, pull),
            _ => crate::column::fill(&lines, spacing, r.stitch_length * 10.0, pull),
        }]);
        return out;
    }
    // Auto direction, satin: the rows follow a skeleton derived from the region, so they stay
    // across the local axis on curved and bent regions. Travel (Running Stitch Path) does not
    // apply with Auto satin. The fixed direction across the narrow side remains for fill, for
    // regions with holes or several parts, with gradation, feathered edges or split stitches,
    // and where the skeleton column does not cover the region (a branch, a blob).
    if r.auto_direction
        && r.kind == RegionType::Satin
        && let Some(top) = auto_satin(closed, r, holes)
    {
        // PE-DESIGN 11 sews the under sewing rows of Auto satin at 90° (across a bent band's
        // horizontal leg, along its vertical one): the rows of a 0° satin.
        let mut plain = r.clone();
        plain.auto_direction = false;
        plain.angle = 0.0;
        let mut out = layers_inner(closed, &plain, holes, user);
        out.pop();
        out.push(vec![top]);
        return out;
    }
    let mut out = Vec::new();
    let angle = if r.auto_direction {
        auto_angle(closed)
    } else {
        geometry_angle(r.angle)
    };
    let params = RegionParams {
        angle,
        row_spacing: 10.0 / r.density.max(0.1),
        stitch_length: r.stitch_length * 10.0,
        pull_compensation: r.pull_compensation * 10.0,
    };
    let row_based = matches!(r.kind, RegionType::Fill | RegionType::Satin);
    if r.underlay {
        let edge = || -> Vec<Run> {
            closed
                .iter()
                .map(|c| match r.entry {
                    // The underlay begins (and ends) at the entry point.
                    Some(p) => generate::running(&c.starting_at(p), 25.0, 1),
                    None => generate::running(c, 25.0, 1),
                })
                .collect()
        };
        let rows = |a: f64| -> Vec<Run> {
            let p = RegionParams {
                angle: a,
                row_spacing: r.underlay_spacing_mm() * 10.0,
                stitch_length: params.stitch_length,
                pull_compensation: -UNDERLAY_INSET,
            };
            generate::satin(closed, &p, holes)
        };
        let across = if r.kind == RegionType::Satin {
            angle
        } else {
            angle + 90.0
        };
        // Satin as PE-DESIGN 11 sews it: the outline 1 mm in, then rows across the satin
        // (perpendicular to its stitches) 2 mm apart with stitches of the fill length.
        // PE-DESIGN starts satin at its start point, by default the outline's first point.
        let satin_start = r
            .entry
            .or_else(|| closed.first().and_then(|c| c.points.first().copied()));
        let satin_rows = || -> Vec<Run> {
            let p = RegionParams {
                angle: angle + 90.0,
                row_spacing: r.underlay_spacing_mm() * 10.0,
                stitch_length: params.stitch_length,
                pull_compensation: -SATIN_UNDERLAY_INSETS[0],
            };
            satin_underlay_rows(closed, &p, holes, satin_start)
        };
        // The outline starts at the start point, or else where the rows begin.
        let satin_edge = |start: Option<Vec2>| -> Vec<Run> {
            let Some(inner) = satin_inset(closed, holes) else {
                return edge();
            };
            inner
                .iter()
                .map(|c| match satin_start.or(start) {
                    Some(p) => generate::running(&c.starting_at(p), 25.0, 1),
                    None => generate::running(c, 25.0, 1),
                })
                .collect()
        };
        match r.underlay_style {
            UnderlayStyle::Zigzag if r.kind == RegionType::Satin => out.push(satin_rows()),
            UnderlayStyle::EdgeZigzag if r.kind == RegionType::Satin => {
                let rows = satin_rows();
                out.push(satin_edge(rows.first().and_then(|r| r.first().copied())));
                out.push(rows);
            }
            UnderlayStyle::Edge => out.push(edge()),
            UnderlayStyle::Zigzag => out.push(rows(across)),
            UnderlayStyle::EdgeZigzag => {
                out.push(edge());
                out.push(rows(across));
            }
            UnderlayStyle::Double => {
                out.push(rows(angle + 45.0));
                out.push(rows(angle - 45.0));
            }
            UnderlayStyle::EdgeFill => out.extend(fill_underlay(closed, r, angle, holes)),
        }
    }
    let shaped =
        r.sewn_gradation().is_some() || r.sewn_feather().is_some() || r.split_length.is_some();
    if !row_based {
        out.extend(regions::generate(closed, r, angle, holes, user));
        return out;
    }
    let mut runs = match r.kind {
        RegionType::Fill if !shaped => {
            generate::fill_edge_to_edge(closed, &params, holes, r.fill_shift(), r.entry)
        }
        // A feathered edge alone keeps the sawtooth (PE-DESIGN 11.50, satin-feather).
        RegionType::Satin
            if !shaped || (r.sewn_gradation().is_none() && r.split_length.is_none()) =>
        {
            sawtooth_satin(closed, r, &params, holes)
        }
        _ => shaped_rows(closed, r, &params, holes),
    };
    if let Some(path) = r.travel_path() {
        runs = match r.kind {
            // PE-DESIGN's satin travels in 1.8-2.0 mm stitches (satin-sections-h, the U).
            RegionType::Satin => {
                let pieces: Vec<Run> = runs
                    .into_iter()
                    .flat_map(|run| split_outside(run, closed, holes))
                    .collect();
                let mut graph = travel_graph(closed, holes, path, params.pull_compensation);
                join(pieces, Some(&mut graph), SATIN_TRAVEL_PITCH)
            }
            _ => reroute_with(runs, closed, holes, path, params.pull_compensation),
        };
    }
    out.push(runs);
    out
}

/// Inset of PE-DESIGN's fill under sewing (edge run and rows) from the outline, 0.1 mm.
pub const FILL_UNDERLAY_INSET: f64 = 10.0;
/// Edge run pitch of the fill under sewing (each side split into equal stitches up to this).
const FILL_UNDERLAY_PITCH: f64 = 25.0;
/// Needle shift per row of the under sewing rows, fraction of a stitch.
const FILL_UNDERLAY_SHIFT: f64 = 0.3;

/// PE-DESIGN's fill under sewing (measured in its 11.50 output, fill-analysis): a running
/// stitch 1 mm inside the outline (holes included) with stitches up to 2.5 mm, then tatami rows
/// across the sewing direction `underlay_spacing` apart, 1 mm inside the outline, with the top
/// layer's stitch length and a 30 % needle shift. Both layers start at the entry point.
fn fill_underlay(
    closed: &[Contour],
    r: &RegionSewing,
    angle: f64,
    holes: &[Contour],
) -> Vec<Vec<Run>> {
    let mut shape: Vec<Contour> = closed.to_vec();
    shape.extend(holes.iter().cloned());
    let inset =
        crate::boolean::contours(&crate::boolean::offset(&shape, -FILL_UNDERLAY_INSET, false));
    let mut edge: Vec<Run> = inset
        .iter()
        .filter(|c| c.closed && c.points.len() >= 3)
        .map(|c| match r.entry {
            Some(p) => generate::running(&c.starting_at(p), FILL_UNDERLAY_PITCH, 1),
            None => generate::running(c, FILL_UNDERLAY_PITCH, 1),
        })
        .collect();
    // With an entry point the region starts there: the edge run nearest to it goes first and
    // is entered from the entry on the outline.
    if let Some(p) = r.entry {
        let at = crate::route::snap(closed, p);
        let d = |run: &Run| run.first().map_or(f64::INFINITY, |q| q.distance(at));
        if let Some(i) = (0..edge.len()).min_by(|&a, &b| d(&edge[a]).total_cmp(&d(&edge[b]))) {
            let mut first = edge.remove(i);
            first.insert(0, at);
            edge.insert(0, first);
        }
    }
    let p = RegionParams {
        angle: angle + 90.0,
        row_spacing: r.underlay_spacing_mm() * 10.0,
        stitch_length: r.stitch_length * 10.0,
        pull_compensation: -FILL_UNDERLAY_INSET,
    };
    let rows = generate::fill_shifted(closed, &p, holes, Some(FILL_UNDERLAY_SHIFT));
    let pieces: Vec<Run> = edge
        .into_iter()
        .chain(rows)
        .flat_map(|run| split_outside(run, closed, holes))
        .collect();
    vec![join(
        pieces,
        Some(&mut TravelGraph::new(closed, holes)),
        25.0,
    )]
}

/// A stored direction angle (degrees counter-clockwise on screen, as PE-DESIGN's Sewing
/// Attributes give it) as the angle the geometry code rotates by (from +x towards +y, the
/// design's y axis pointing down).
pub fn geometry_angle(stored: f64) -> f64 {
    -stored
}

/// The stored direction angle (counter-clockwise on screen, 0 ≤ a < 360) of a geometry angle
/// (+x towards +y, y down), for directions derived from a shape.
pub fn stored_angle(geometry: f64) -> f64 {
    let a = (-geometry).rem_euclid(360.0);
    if a >= 360.0 - 1e-9 { 0.0 } else { a + 0.0 }
}

/// Joins a region's layers (under sewing, then the top stitching) with running stitches, as
/// PE-DESIGN sews them: no jump or trim from one layer to the next. The move follows `path`
/// (Running Stitch Path), as the travel between parts does: PE's Inside goes straight across,
/// Along / On outline follow the outline. Runs inside a layer stay as they are.
pub fn join_layers(
    layers: Vec<Vec<Run>>,
    closed: &[Contour],
    holes: &[Contour],
    path: RunningStitchPath,
) -> Vec<Run> {
    let mut graph = travel_graph(closed, holes, path, 0.0);
    let mut out: Vec<Run> = Vec::new();
    for layer in layers {
        for (i, run) in layer.into_iter().filter(|r| !r.is_empty()).enumerate() {
            match out.pop() {
                Some(last) if i == 0 => out.extend(join(vec![last, run], Some(&mut graph), 25.0)),
                Some(last) => out.extend([last, run]),
                None => out.push(run),
            }
        }
    }
    out
}

/// Row connections that cut across a gap (a hole, a notch) are split off, then every gap is
/// bridged by a running path inside the region.
pub fn reroute(runs: Vec<Run>, closed: &[Contour], holes: &[Contour]) -> Vec<Run> {
    reroute_with(runs, closed, holes, RunningStitchPath::Inside, 0.0)
}

/// Inset of Along outline travel from the outline (0.1 mm). A placeholder until measured.
pub const ALONG_OUTLINE_INSET: f64 = 5.0;

/// [`reroute`] with the travel of `path`. Row ends stand `pull` (0.1 mm) beyond the outline,
/// and still count as on it for the outline variants.
pub fn reroute_with(
    runs: Vec<Run>,
    closed: &[Contour],
    holes: &[Contour],
    path: RunningStitchPath,
    pull: f64,
) -> Vec<Run> {
    let pieces: Vec<Run> = runs
        .into_iter()
        .flat_map(|run| split_outside(run, closed, holes))
        .collect();
    let mut graph = travel_graph(closed, holes, path, pull);
    join(pieces, Some(&mut graph), 25.0)
}

/// The travel graph of `path`; row ends `pull` (0.1 mm) beyond the outline count as on it.
fn travel_graph(
    closed: &[Contour],
    holes: &[Contour],
    path: RunningStitchPath,
    pull: f64,
) -> TravelGraph {
    let mut graph = TravelGraph::new(closed, holes);
    graph.outline = match path {
        RunningStitchPath::Inside => None,
        RunningStitchPath::AlongOutline => Some(OutlineTravel::Inset(ALONG_OUTLINE_INSET)),
        RunningStitchPath::OnOutline => Some(OutlineTravel::On),
    };
    graph.outline_tolerance = pull.max(0.0) + 1.0;
    graph
}

/// Splits `run` wherever a stitch leaves the region (its midpoint is outside and more than
/// 0.05 mm from the outline).
fn split_outside(run: Run, closed: &[Contour], holes: &[Contour]) -> Vec<Run> {
    let edges: Vec<(Vec2, Vec2)> = closed
        .iter()
        .chain(holes)
        .flat_map(Contour::edges)
        .collect();
    let near = |p: Vec2| {
        edges.iter().any(|&(a, b)| {
            let d = b - a;
            let l2 = d.x * d.x + d.y * d.y;
            let t = if l2 > 0.0 {
                (((p - a).x * d.x + (p - a).y * d.y) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            p.distance(a + d * t) <= 0.5
        })
    };
    let inside = |p: Vec2| contains(p, closed) && !contains(p, holes);
    let mut out = Vec::new();
    let mut current: Run = Vec::new();
    for p in run {
        if let Some(&last) = current.last() {
            let m = last.lerp(p, 0.5);
            if !inside(m) && !near(m) {
                out.push(std::mem::take(&mut current));
            }
        }
        current.push(p);
    }
    out.push(current);
    out.retain(|r| !r.is_empty());
    out
}

/// Rows of satin under sewing (`p` gives their direction, spacing, stitch and inset as a
/// negative pull compensation). The rows lie on the region's grid but are cut from its
/// under sewing outline ([`satin_inset`]), so none runs along the region's own edge. Where no
/// row is cut in two, they are sewn one after the other across the region, each from its end
/// nearer the last needle point, as PE-DESIGN 11 does; otherwise as fill rows (nearest row
/// next).
fn satin_underlay_rows(
    closed: &[Contour],
    p: &RegionParams,
    holes: &[Contour],
    start: Option<Vec2>,
) -> Vec<Run> {
    let Some(inner) = satin_inset(closed, holes) else {
        return generate::fill(closed, p, holes);
    };
    let rows: Vec<(usize, f64, Vec<Span>)> = row_positions(closed, p.angle, p.row_spacing, None)
        .into_iter()
        .enumerate()
        .map(|(i, y)| {
            // Spans that touch (a row along an edge between two parts) are one span. A row on
            // an edge of the inset region takes the side where it is longer, so a row on a
            // leg's inner edge runs the leg's length (PE-DESIGN does so on one leg of
            // region-travel-u-satin's U; on the other it sews only the bar part).
            let side = |d: f64| spans_at(&inner, &[], p.angle, y + d);
            let length = |s: &[Span]| s.iter().map(|s| s.end() - s.start()).sum::<f64>();
            let (lo, hi) = (side(-EDGE_ROW_INSET), side(EDGE_ROW_INSET));
            let mut merged: Vec<Span> = Vec::new();
            for s in if length(&hi) > length(&lo) { hi } else { lo } {
                match merged.last_mut() {
                    Some(m) if s.start() - m.end() < 1e-6 => *m = *m.start()..=*s.end(),
                    _ => merged.push(s),
                }
            }
            let spans = merged.into_iter().filter(|s| s.end() > s.start()).collect();
            (i, y, spans)
        })
        .collect();
    if rows.iter().any(|(_, _, s)| s.len() > 1) {
        let p = RegionParams {
            pull_compensation: 0.0,
            ..*p
        };
        return generate::fill(&inner, &p, &[]);
    }
    // From the side away from the start point towards it, so the under sewing ends near where
    // the top stitching starts; the first row from its end nearer the start point (where the
    // outline ends), as PE-DESIGN sews the U and the bent band.
    let mut rows = rows;
    let near = |row: &(usize, f64, Vec<Span>)| -> Option<f64> {
        let s = start?.rotated(-p.angle.to_radians());
        row.2.first().map(|_| (row.1 - s.y).abs())
    };
    let (first, last) = (rows.iter().find_map(near), rows.iter().rev().find_map(near));
    if let (Some(f), Some(l)) = (first, last)
        && f < l
    {
        rows.reverse();
    }
    let mut at: Option<Vec2> = start;
    let mut runs = Vec::new();
    for (i, y, spans) in rows {
        let Some(span) = spans.first() else {
            continue;
        };
        let mut xs = tatami_points(span, i, p.stitch_length, None);
        let run: Run = xs.iter().map(|&x| unrotate(x, y, p.angle)).collect();
        let reverse = at.is_some_and(|a| a.distance(run[run.len() - 1]) < a.distance(run[0]));
        if reverse {
            xs.reverse();
        }
        let mut run: Run = xs.iter().map(|&x| unrotate(x, y, p.angle)).collect();
        if runs.is_empty() {
            // PE-DESIGN sews the first row there and back.
            let back: Run = run.iter().rev().skip(1).copied().collect();
            run.extend(back);
        }
        at = run.last().copied();
        runs.push(run);
    }
    runs
}

/// Rows of a sawtooth satin lie this far (0.1 mm) inside the region's first and last edge, so
/// a row is never cut along an edge.
const EDGE_ROW_INSET: f64 = 0.01;

/// Where satin splits at its start point, the last row of each half stops this far (0.1 mm)
/// short of the start point (PE-DESIGN 11: 0.2 mm).
const SPLIT_GAP: f64 = 2.0;

/// Running stitch of the travel along the edge to each half of a split satin (0.1 mm).
const SPLIT_TRAVEL_PITCH: f64 = 20.0;

/// Fixed-direction satin as PE-DESIGN 11 sews it: rows two crossings apart from the region's
/// first edge to its last, each row sewn whole in one direction and the stitch to the next row
/// slanting back (a sawtooth: one crossing along the sewing direction, the next slanted). With
/// a start point inside the column it is sewn in two halves ([`split_satin`]).
fn sawtooth_satin(
    closed: &[Contour],
    r: &RegionSewing,
    p: &RegionParams,
    holes: &[Contour],
) -> Vec<Run> {
    let pitch = 2.0 * p.row_spacing;
    if let Some(run) = r
        .entry
        .and_then(|e| split_satin(closed, holes, p, pitch, e, r.sewn_feather()))
    {
        return vec![run];
    }
    let rows: Vec<(f64, Vec<Span>)> = edge_rows(closed, p.angle, pitch)
        .into_iter()
        .map(|y| (y, pulled_spans(closed, holes, p, y)))
        .collect();
    // PE-DESIGN starts (and ends) a region at its start point, by default the outline's first
    // point: the sections are sewn from there and travel to the first one.
    let first = closed.first().and_then(|c| c.points.first().copied());
    let start = r.entry.or(first);
    let exit = r.exit.or(start);
    let rad = p.angle.to_radians();
    let mut runs: Vec<Run> = Vec::new();
    if let Some(s) = start.map(|s| crate::route::snap(closed, s)) {
        // PE-DESIGN first sews across the start point's edge row and back (from the row end
        // nearer the start point, the same needle points as that row's satin).
        let at = s.rotated(-rad);
        let row = rows
            .iter()
            .min_by(|a, b| (a.0 - at.y).abs().total_cmp(&(b.0 - at.y).abs()));
        let ends = row.and_then(|(y, spans)| {
            spans
                .iter()
                .find(|r| at.x >= r.start() - 1.0 && at.x <= r.end() + 1.0)
                .map(|r| {
                    let (near, far) = if (at.x - r.start()).abs() <= (at.x - r.end()).abs() {
                        (*r.start(), *r.end())
                    } else {
                        (*r.end(), *r.start())
                    };
                    (unrotate(near, *y, p.angle), unrotate(far, *y, p.angle))
                })
        });
        runs.push(match ends {
            Some((near, far)) if near.distance(far) > 1.0 => vec![near, far, near],
            _ => vec![s],
        });
    }
    // Outline corners in order, rotated: a leg is entered at the tip corner that comes first.
    let corners: Vec<Vec2> = closed
        .first()
        .map(|c| c.points.iter().map(|q| q.rotated(-rad)).collect())
        .unwrap_or_default();
    runs.extend(
        sections_in_order(
            &rows,
            start.map(|s| s.rotated(-rad)),
            exit.map(|e| e.rotated(-rad)),
            &corners,
            pitch,
        )
        .into_iter()
        .map(|rows| {
            let n = rows.len();
            rows.into_iter()
                .enumerate()
                .flat_map(|(k, (y, from, to))| {
                    let (from, to) = feathered(from, to, k, k + 1 == n, r.sewn_feather());
                    [unrotate(from, y, p.angle), unrotate(to, y, p.angle)]
                })
                .collect()
        }),
    );
    runs
}

/// A run of consecutive rows whose spans continue one another (rotated space): one leg, bar or
/// block of a satin region, with the sections it touches at its first and last row.
struct Section {
    rows: Vec<(f64, Span)>,
    before: Vec<usize>,
    after: Vec<usize>,
}

/// Splits rows of spans (rotated space, in row order) into sections: a section continues while
/// exactly one span of the next row overlaps its last span and that span overlaps nothing else.
fn sections(rows: &[(f64, Vec<Span>)]) -> Vec<Section> {
    let overlap = |a: &Span, b: &Span| a.start() < b.end() && b.start() < a.end();
    let mut out: Vec<Section> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    for (y, spans) in rows {
        let mut next = Vec::new();
        let mut taken = vec![false; spans.len()];
        for &s in &open {
            let last = &out[s].rows[out[s].rows.len() - 1].1;
            let hits: Vec<usize> = (0..spans.len())
                .filter(|&i| overlap(last, &spans[i]))
                .collect();
            if let [i] = hits[..]
                && open
                    .iter()
                    .filter(|&&o| overlap(&out[o].rows[out[o].rows.len() - 1].1, &spans[i]))
                    .count()
                    == 1
            {
                out[s].rows.push((*y, spans[i].clone()));
                taken[i] = true;
                next.push(s);
            }
        }
        for (_, span) in spans.iter().enumerate().filter(|(i, _)| !taken[*i]) {
            let id = out.len();
            let before: Vec<usize> = open
                .iter()
                .copied()
                .filter(|&o| !next.contains(&o))
                .filter(|&o| overlap(&out[o].rows[out[o].rows.len() - 1].1, span))
                .collect();
            for &b in &before {
                out[b].after.push(id);
            }
            out.push(Section {
                rows: vec![(*y, span.clone())],
                before,
                after: Vec::new(),
            });
            next.push(id);
        }
        open = next;
    }
    out
}

/// The rows of a sawtooth satin as PE-DESIGN 11.50 orders them (region-travel-u-satin,
/// satin-sections-h): section by section from the far end of the rows back (the last row's
/// side first, and across a row from its far end), every leg from its tip towards the section
/// it joins, a section between others from the side already sewn; the section holding the
/// exit point (rotated space) comes last and ends there. Each row is (y, from, to), every row
/// of a section sewn in the same direction.
fn sections_in_order(
    rows: &[(f64, Vec<Span>)],
    start: Option<Vec2>,
    exit: Option<Vec2>,
    corners: &[Vec2],
    pitch: f64,
) -> Vec<Vec<(f64, f64, f64)>> {
    let all = sections(rows);
    if all.is_empty() {
        return Vec::new();
    }
    let middle = |s: &Section| {
        let (y0, y1) = (s.rows[0].0, s.rows[s.rows.len() - 1].0);
        let span = &s.rows[s.rows.len() / 2].1;
        ((y0 + y1) / 2.0, (span.start() + span.end()) / 2.0)
    };
    let holds = |s: &Section, e: Vec2| {
        let (y0, y1) = (s.rows[0].0, s.rows[s.rows.len() - 1].0);
        let pad = if s.rows.len() > 1 {
            (y1 - y0) / (s.rows.len() - 1) as f64
        } else {
            1.0
        };
        e.y >= y0 - pad
            && e.y <= y1 + pad
            && s.rows
                .iter()
                .any(|(_, r)| e.x >= r.start() - pad && e.x <= r.end() + pad)
    };
    // (A region of one section is simply sewn from the start point's side.)
    let last = exit.filter(|_| all.len() > 1).and_then(|e| {
        (0..all.len())
            .filter(|&i| holds(&all[i], e))
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    let (y, x) = middle(&all[i]);
                    (y - e.y).hypot(x - e.x)
                };
                d(a).total_cmp(&d(b))
            })
    });
    let mut order: Vec<usize> = (0..all.len()).filter(|&i| Some(i) != last).collect();
    order.sort_by(|&a, &b| {
        let ((ya, xa), (yb, xb)) = (middle(&all[a]), middle(&all[b]));
        yb.total_cmp(&ya).then(xb.total_cmp(&xa))
    });
    order.extend(last);
    let mut done = vec![false; all.len()];
    let mut at: Option<Vec2> = start;
    let mut out = Vec::new();
    for &i in &order {
        let s = &all[i];
        let sewn = |n: &[usize]| n.iter().any(|&j| done[j]);
        // Rows in their order (first row first) or reversed.
        let forward = if let Some(e) = exit.filter(|_| Some(i) == last) {
            (e.y - s.rows[s.rows.len() - 1].0).abs() <= (e.y - s.rows[0].0).abs()
        } else if s.before.is_empty() != s.after.is_empty() {
            // A leg: from its tip, the end that touches nothing.
            s.before.is_empty()
        } else if sewn(&s.before) != sewn(&s.after) {
            sewn(&s.before)
        } else {
            at.is_none_or(|a| (a.y - s.rows[0].0).abs() <= (a.y - s.rows[s.rows.len() - 1].0).abs())
        };
        let rows: Vec<&(f64, Span)> = if forward {
            s.rows.iter().collect()
        } else {
            s.rows.iter().rev().collect()
        };
        // Across each row from the end nearer the needle (the far end for the first section).
        let (y0, first) = (rows[0].0, &rows[0].1);
        // The outline corner (index) at a row end, if one is within a row of it.
        let corner = |x: f64| {
            corners
                .iter()
                .position(|c| (c.x - x).abs() <= pitch && (c.y - y0).abs() <= pitch)
        };
        let leg = s.before.is_empty() != s.after.is_empty() && Some(i) != last;
        let from_start = match (leg, corner(*first.start()), corner(*first.end())) {
            (true, Some(a), Some(b)) => a < b,
            _ => match at {
                Some(a) => (a.x - first.start()).abs() <= (a.x - first.end()).abs(),
                None => false,
            },
        };
        let n = rows.len();
        let sewn_rows: Vec<(f64, f64, f64)> = rows
            .iter()
            .enumerate()
            .map(|(k, (y, r))| {
                // A last row closer than a row's pitch is reached along the far edge and sewn
                // back across (PE: the H's top-right leg).
                let partial = k + 1 == n && n > 1 && (y - rows[k - 1].0).abs() < 0.75 * pitch;
                if from_start != partial {
                    (*y, *r.start(), *r.end())
                } else {
                    (*y, *r.end(), *r.start())
                }
            })
            .collect();
        at = sewn_rows.last().map(|&(y, _, to)| v2(to, y));
        done[i] = true;
        out.push(sewn_rows);
    }
    out
}

/// PE-DESIGN 11.50's feathered edge on satin: the rows' ends move in by these shares of the
/// feather width, row after row, the pattern restarting with each run of rows; the first and
/// last row keep their ends (satin-feather: 3 mm, rows 0.7 mm apart).
const FEATHER_PATTERN: [f64; 12] = [0.9, 0.2, 0.5, 1.0, 0.5, 0.1, 0.6, 0.3, 0.8, 0.0, 0.7, 0.4];

/// A satin row sewn from `from` to `to` (rotated space), its ends feathered: `Start` moves the
/// end the row starts from, `End` the one it ends at, by [`FEATHER_PATTERN`]'s share for row
/// `k` of the feather width, at most 90 % of the row.
fn feathered(from: f64, to: f64, k: usize, last: bool, feather: Option<Feather>) -> (f64, f64) {
    let Some(f) = feather.filter(|_| k > 0 && !last) else {
        return (from, to);
    };
    let len = to - from;
    let d = (FEATHER_PATTERN[(k - 1) % FEATHER_PATTERN.len()] * f.width * 10.0)
        .min(len.abs() * 0.9)
        * len.signum();
    match f.side {
        FeatherSide::Start => (from + d, to),
        FeatherSide::End => (from, to - d),
        FeatherSide::Both => (from + d, to - d),
    }
}

/// Row positions (rotated space) `pitch` apart from just inside the region's first edge, and
/// one more just inside its last edge.
fn edge_rows(contours: &[Contour], angle: f64, pitch: f64) -> Vec<f64> {
    let Some(b) = rotated_bounds(contours, angle) else {
        return Vec::new();
    };
    let (lo, hi) = (b.min_y + EDGE_ROW_INSET, b.max_y - EDGE_ROW_INSET);
    if pitch.is_nan() || pitch <= 0.0 || hi.is_nan() || hi <= lo {
        return Vec::new();
    }
    let mut ys = Vec::new();
    let mut y = lo;
    while y < hi - pitch * 0.25 && ys.len() < 100_000 {
        ys.push(y);
        y += pitch;
    }
    ys.push(hi);
    ys
}

/// Inside intervals of the row at rotated `y`, lengthened by the pull compensation.
fn pulled_spans(closed: &[Contour], holes: &[Contour], p: &RegionParams, y: f64) -> Vec<Span> {
    spans_at(closed, holes, p.angle, y)
        .into_iter()
        .filter_map(|s| {
            let (lo, hi) = (
                s.start() - p.pull_compensation,
                s.end() + p.pull_compensation,
            );
            (hi > lo).then_some(lo..=hi)
        })
        .collect()
}

/// Satin whose start point `entry` lies inside the column, as PE-DESIGN 11 sews a rectangle
/// (its default start point is the middle of the bottom edge): from the start point along its
/// edge to one end of the column, the sawtooth back towards the start point, the last row
/// [`SPLIT_GAP`] short of it reached along the far edge, then the same for the other half,
/// ending at the start point again. `None` (sewn whole) for several parts, holes, a row cut in
/// two, or a start point within a row of either end.
fn split_satin(
    closed: &[Contour],
    holes: &[Contour],
    p: &RegionParams,
    pitch: f64,
    entry: Vec2,
    feather: Option<Feather>,
) -> Option<Run> {
    if closed.len() != 1 || !holes.is_empty() {
        return None;
    }
    let e = crate::route::snap(closed, entry);
    let at = e.rotated(-p.angle.to_radians());
    let b = rotated_bounds(closed, p.angle)?;
    let (lo, hi) = (b.min_y + EDGE_ROW_INSET, b.max_y - EDGE_ROW_INSET);
    if at.y < lo + pitch || at.y > hi - pitch {
        return None;
    }
    let one = |y: f64| -> Option<Span> {
        match pulled_spans(closed, holes, p, y).as_slice() {
            [s] => Some(s.clone()),
            _ => None,
        }
    };
    let middle = one(at.y)?;
    // The start point's edge: the start or the end of the rows.
    let near_start = (at.x - middle.start()).abs() <= (at.x - middle.end()).abs();
    let ends = |s: &Span| {
        if near_start {
            (*s.start(), *s.end())
        } else {
            (*s.end(), *s.start())
        }
    };
    let half = |from: f64, step: f64| -> Option<Vec<(f64, Span)>> {
        let stop = at.y - step.signum() * (SPLIT_GAP + pitch.abs() * 0.25);
        let mut rows = Vec::new();
        let mut y = from;
        while (stop - y) * step.signum() > 0.0 && rows.len() < 100_000 {
            rows.push((y, one(y)?));
            y += step;
        }
        let last = at.y - step.signum() * SPLIT_GAP;
        rows.push((last, one(last)?));
        Some(rows)
    };
    let pt = |x: f64, y: f64| unrotate(x, y, p.angle);
    let mut run = vec![e];
    for rows in [half(lo, pitch)?, half(hi, -pitch)?] {
        let (near, _) = ends(&rows[0].1);
        let first = pt(near, rows[0].0);
        let path = crate::paths::inside_path(e, first, closed, holes).unwrap_or(vec![e, first]);
        run.extend(subdivide(&path, SPLIT_TRAVEL_PITCH).into_iter().skip(1));
        let n = rows.len();
        for (k, (y, s)) in rows.iter().enumerate() {
            let (near, far) = ends(s);
            let (near, far) = feathered(near, far, k, k + 1 == n, feather);
            if k > 0 && k + 1 < n {
                run.push(pt(near, *y));
            }
            run.push(pt(far, *y));
            if k + 1 == n {
                // The last row: along the far edge to it, across, along the near edge home.
                run.extend([pt(near, *y), e]);
            }
        }
    }
    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    Some(run)
}

/// Tie-in and end lock of a satin object, as PE-DESIGN 11.50 sews them. From the start point
/// the needle steps 0.3 mm along the first stitch (at most its length) and back, then sews on
/// over that stitch; after a first stitch longer than 3 mm it steps out three times (0.9 mm)
/// and comes back in stitches of at most 0.5 mm. At the end it steps back along the last
/// stitch, 0.3 mm at a time, and returns the same way: twice after a last stitch longer than
/// 4.25 mm, once otherwise. (PE: one tie-in step after 1.2–2.3 mm first stitches, three after
/// 3.5 mm; one lock step after 0.2–3.5 mm last stitches, two after 5 and 30 mm ones.)
pub fn tie_and_lock(layers: &mut [Vec<Run>]) {
    const STEP: f64 = 3.0;
    if let Some(run) = layers.iter_mut().flatten().find(|r| r.len() >= 2) {
        let (a, b) = (run[0], run[1]);
        let len = a.distance(b);
        if len > 1e-9 {
            let unit = (b - a) * (1.0 / len);
            let steps = if len > 30.0 { 3 } else { 1 };
            let out = (STEP * steps as f64).min(len);
            let mut tie: Vec<Vec2> = (1..=steps)
                .map(|k| a + unit * (out * k as f64 / steps as f64))
                .collect();
            let tip = a + unit * out;
            tie.extend(subdivide(&[tip, a], 5.0).into_iter().skip(1));
            run.splice(1..1, tie);
        }
    }
    if let Some(run) = layers
        .iter_mut()
        .rev()
        .flatten()
        .rev()
        .find(|r| r.len() >= 2)
    {
        let (a, b) = (run[run.len() - 2], run[run.len() - 1]);
        let len = a.distance(b);
        if len > 1e-9 {
            let back = (a - b) * (STEP.min(len) / len);
            let steps = if len > 42.5 { 2 } else { 1 };
            run.extend((1..=steps).map(|k| b + back * k as f64));
            run.extend((0..steps).rev().map(|k| b + back * k as f64));
        }
    }
}

/// Least share of the region a derived skeleton's column must cover for Auto satin to follow
/// it.
const AUTO_COVERAGE: f64 = 0.85;

/// Auto-direction satin along a skeleton derived from the single closed contour `closed`;
/// `None` where it does not apply (see [`layers_with`]).
fn auto_satin(closed: &[Contour], r: &RegionSewing, holes: &[Contour]) -> Option<Run> {
    let shaped =
        r.sewn_gradation().is_some() || r.sewn_feather().is_some() || r.split_length.is_some();
    if closed.len() != 1 || !holes.is_empty() || shaped {
        return None;
    }
    let s = crate::skeleton::derive(closed).filter(|s| s.validate().is_ok())?;
    let spacing = 10.0 / r.density.max(0.1);
    let rungs = crate::skeleton::rungs_within(&s, closed, spacing, r.pull_compensation * 10.0);
    let mut column: Vec<Vec2> = rungs.iter().map(|r| r[0]).collect();
    column.extend(rungs.iter().rev().map(|r| r[1]));
    if area(&column) < AUTO_COVERAGE * area(&closed[0].outline_points()) {
        return None;
    }
    let mut run = crate::column::zigzag(rungs);
    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    Some(run)
}

/// Area enclosed by the polygon `pts` (shoelace, unsigned).
fn area(pts: &[Vec2]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

/// Direction (degrees) whose rows run across the region's narrow side: perpendicular to the
/// principal axis of the outline (second moments of the edges, weighted by length).
pub fn auto_angle(contours: &[Contour]) -> f64 {
    let mut w = 0.0;
    let (mut mx, mut my) = (0.0, 0.0);
    let edges: Vec<(Vec2, f64)> = contours
        .iter()
        .flat_map(Contour::edges)
        .map(|(a, b)| (a.lerp(b, 0.5), a.distance(b)))
        .collect();
    for &(m, l) in &edges {
        w += l;
        mx += m.x * l;
        my += m.y * l;
    }
    if w <= 0.0 {
        return 0.0;
    }
    let (cx, cy) = (mx / w, my / w);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for &(m, l) in &edges {
        let (dx, dy) = (m.x - cx, m.y - cy);
        sxx += dx * dx * l;
        syy += dy * dy * l;
        sxy += dx * dy * l;
    }
    let major = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let deg = major.to_degrees() + 90.0;
    // Normalise to (-90, 90] and round to 0.01° so output does not depend on noise.
    let mut a = deg.rem_euclid(180.0);
    if a > 90.0 {
        a -= 180.0;
    }
    (a * 100.0).round() / 100.0
}

/// Rows with gradation, feathered ends or split satin stitches.
fn shaped_rows(
    closed: &[Contour],
    r: &RegionSewing,
    p: &RegionParams,
    holes: &[Contour],
) -> Vec<Run> {
    let ys = row_positions(closed, p.angle, p.row_spacing, r.sewn_gradation());
    let mut spans = Vec::new();
    for (row, &y) in ys.iter().enumerate() {
        for s in spans_at(closed, holes, p.angle, y) {
            let lo = s.start() - p.pull_compensation;
            let hi = s.end() + p.pull_compensation;
            if hi > lo {
                spans.push(RowSpan {
                    row,
                    y,
                    range: lo..=hi,
                });
            }
        }
    }
    let l = p.stitch_length;
    let split = r.split_length.map(|s| s * 10.0);
    let satin = r.kind == RegionType::Satin;
    let feather = r.sewn_feather();
    let shift = r.fill_shift();
    // Rows are ordered on their full spans; feathering then shortens the ends, so the
    // sewing order does not depend on the random ends.
    let chain = if satin {
        generate::chain_satin_spans
    } else {
        generate::chain_spans
    };
    chain(&spans, p, &|span: &Span, row: usize, forward: bool| {
        let (mut lo, mut hi) = (*span.start(), *span.end());
        if let Some(f) = feather {
            let mut rng =
                Rng::new(0x5EED_F00D ^ (row as u64).wrapping_mul(0x9E37_79B9) ^ lo.to_bits());
            let w = (f.width * 10.0).min((hi - lo) * 0.45);
            if matches!(f.side, FeatherSide::Start | FeatherSide::Both) {
                lo += rng.next_f64() * w;
            }
            if matches!(f.side, FeatherSide::End | FeatherSide::Both) {
                hi -= rng.next_f64() * w;
            }
        }
        let span = lo..=hi;
        let mut xs = if satin {
            satin_points(&span, row, split)
        } else {
            tatami_points(&span, row, l, shift)
        };
        if !forward {
            xs.reverse();
        }
        xs
    })
}

/// Satin row: one stitch, or with `split` equal stitches no longer than `split`, staggered by
/// half a stitch on odd rows so the split points do not line up.
fn satin_points(span: &Span, row: usize, split: Option<f64>) -> Vec<f64> {
    let (a, b) = (*span.start(), *span.end());
    let len = b - a;
    let Some(split) = split.filter(|&s| s > 0.0 && len > s) else {
        return vec![a, b];
    };
    let n = (len / split).ceil();
    let step = len / n;
    let mut xs = vec![a];
    if row % 2 == 1 {
        xs.extend((0..n as usize).map(|k| a + (k as f64 + 0.5) * step));
    } else {
        xs.extend((1..n as usize).map(|k| a + k as f64 * step));
    }
    xs.push(b);
    xs
}

/// Tatami needle positions, as [`generate::fill`] places them.
fn tatami_points(span: &Span, row: usize, l: f64, shift: Option<f64>) -> Vec<f64> {
    let phase = generate::row_phase(row, shift) * l;
    let mut xs = vec![*span.start()];
    let mut k = ((span.start() - phase) / l).floor() + 1.0;
    while phase + k * l < *span.end() {
        let x = phase + k * l;
        if x - span.start() > l * 0.25 && span.end() - x > l * 0.25 {
            xs.push(x);
        }
        k += 1.0;
    }
    xs.push(*span.end());
    xs
}

/// Bounds of `contours` rotated by −`angle`.
pub fn rotated_bounds(contours: &[Contour], angle: f64) -> Option<Rect> {
    let rad = -angle.to_radians();
    Rect::enclosing(
        contours
            .iter()
            .flat_map(|c| c.outline_points())
            .map(|p| p.rotated(rad)),
    )
}

/// Row positions (rotated space): `spacing` apart, or with gradation `spacing` divided by the
/// gradation's density fraction where the row is (see [`Gradation::fraction`]).
///
/// A pattern (PE-DESIGN) gradation places its first row on the start edge and its last on the
/// far edge, takes each gap's density halfway across the gap, and splits what is left before
/// the far edge evenly, as PE-DESIGN 11.50 does (region-travel-u-gradation reference). A continuous one keeps the placement designs saved
/// before patterns had: rows from half a spacing in.
pub fn row_positions(
    contours: &[Contour],
    angle: f64,
    spacing: f64,
    gradation: Option<&Gradation>,
) -> Vec<f64> {
    let Some(b) = rotated_bounds(contours, angle) else {
        return Vec::new();
    };
    if spacing.is_nan() || spacing <= 0.0 || b.height().is_nan() || b.height() <= 0.0 {
        return Vec::new();
    }
    let h = b.height();
    let gap = |y: f64| spacing / gradation.map_or(1.0, |g| g.fraction((y - b.min_y) / h));
    let mut ys = Vec::new();
    if gradation.is_some_and(|g| g.steps.is_some()) {
        // Rows on an edge are sampled just inside it.
        let inside = 1e-3_f64.min(h / 4.0);
        // Each gap takes the density halfway across it, as PE-DESIGN's rows do (within 0.07 mm
        // on its default pattern, against 0.31 mm for the density at the row).
        let mid_gap = |y: f64| {
            let mut g = gap(y);
            for _ in 0..12 {
                g = gap(y + g / 2.0);
            }
            g
        };
        let mut y = b.min_y;
        let mut last_gap = mid_gap(y);
        while ys.len() < 100_000 {
            ys.push(y.clamp(b.min_y + inside, b.max_y - inside));
            let g = mid_gap(y);
            if y + g >= b.max_y - 1e-9 {
                // The rest up to the far edge, in equal steps no longer than the last gap.
                let rest = b.max_y - y;
                let n = (rest / last_gap.max(1e-9) - 1e-9).ceil().max(1.0) as usize;
                for k in 1..=n {
                    let yk = y + rest * k as f64 / n as f64;
                    ys.push(yk.clamp(b.min_y + inside, b.max_y - inside));
                }
                break;
            }
            last_gap = g;
            y += g;
        }
        return ys;
    }
    let mut y = b.min_y + spacing / 2.0;
    while y < b.max_y && ys.len() < 100_000 {
        ys.push(y);
        y += gap(y);
    }
    ys
}

/// Inside intervals of the row at rotated `y` (even-odd), minus the holes.
pub fn spans_at(contours: &[Contour], holes: &[Contour], angle: f64, y: f64) -> Vec<Span> {
    let rad = -angle.to_radians();
    let xs_at = |cs: &[Contour]| -> Vec<Span> {
        let mut xs: Vec<f64> = cs
            .iter()
            .filter(|c| c.closed)
            .flat_map(Contour::edges)
            .map(|(p, q)| (p.rotated(rad), q.rotated(rad)))
            .filter(|(p, q)| (p.y <= y && q.y > y) || (q.y <= y && p.y > y))
            .map(|(p, q)| p.x + (y - p.y) * (q.x - p.x) / (q.y - p.y))
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.as_chunks::<2>()
            .0
            .iter()
            .filter(|w| w[1] > w[0])
            .map(|w| w[0]..=w[1])
            .collect()
    };
    let spans = xs_at(contours);
    if holes.is_empty() {
        spans
    } else {
        subtract(&xs_at(holes), &spans)
    }
}

/// Maps a rotated-space point back to design space.
pub fn unrotate(x: f64, y: f64, angle: f64) -> Vec2 {
    v2(x, y).rotated(angle.to_radians())
}
