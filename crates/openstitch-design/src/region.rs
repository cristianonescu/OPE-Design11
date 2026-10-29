//! Region sewing with its options: underlay styles, automatic direction, gradation, feathered
//! edges, split satin stitches and travel inside the region. Plain fill and satin without
//! options go straight to [`generate::fill`] / [`generate::satin`], so their output is
//! unchanged. Lengths are 0.1 mm.

use crate::direction;
use crate::generate::{self, RegionParams, RowSpan, Run, Stagger, subdivide};
use crate::geometry::{Contour, Rect, Span, Vec2, contains, subtract, v2};
use crate::model::{
    Feather, FeatherSide, Gradation, RegionSewing, RegionType, RunningStitchPath, UnderlayParts,
    UnderlayStyle,
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
        return layers_with(&c, &flat, &h, user)
            .into_iter()
            .map(|l| w.back_runs(l))
            .collect();
    }
    // Reference point: the pattern grid starts there. A programmable or motif fill without one
    // starts its grid at the region's top-left corner, as PE-DESIGN 11.50 does
    // (compat/reference/stitches/prog-fill-*.pes, motif-fill-2pt.pes).
    // The shifted copy carries the reference (0, 0), so it is not anchored again.
    let anchor = r
        .reference
        .or_else(|| {
            matches!(r.kind, RegionType::ProgrammableFill | RegionType::MotifFill)
                .then(|| {
                    Rect::enclosing(closed.iter().flat_map(|c| c.outline_points()))
                        .map(|b| v2(b.min_x, b.min_y))
                })
                .flatten()
        })
        .filter(|m| m.x != 0.0 || m.y != 0.0);
    if let Some(at) = anchor
        && direction::REFERENCE_TYPES.contains(&r.kind)
    {
        let mut local = r.clone();
        local.reference = Some(v2(0.0, 0.0));
        local.entry = r.entry.map(|p| p - at);
        local.center = r.center.map(|p| p - at);
        let back = |l: Vec<Run>| -> Vec<Run> {
            l.into_iter()
                .map(|run| run.into_iter().map(|p| p + at).collect())
                .collect()
        };
        return layers_with(
            &direction::shifted(closed, v2(-at.x, -at.y)),
            &local,
            &direction::shifted(holes, v2(-at.x, -at.y)),
            user,
        )
        .into_iter()
        .map(back)
        .collect();
    }
    // Several direction lines: the top stitching follows the column they make.
    if r.direction_lines.len() >= 2 && matches!(r.kind, RegionType::Satin | RegionType::Fill) {
        let spacing = 10.0 / r.density.max(0.1);
        let pull = r.pull_compensation * 10.0;
        let lines = if r.curved_lines {
            crate::column::curved_rungs(&r.direction_lines)
        } else {
            r.direction_lines.clone()
        };
        let mut out = Vec::new();
        if r.underlay {
            // PE-DESIGN's under sewing of a Manual Punch column (see `column::underlay`).
            let start = r.entry.unwrap_or(lines[0][0]);
            let run = crate::column::underlay(
                &lines,
                r.underlay_parts(),
                r.underlay_spacing_mm() * 10.0,
                r.stitch_length * 10.0,
                start,
            );
            if run.len() > 1 {
                out.push(vec![run]);
            }
        }
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
        // OpenStitch's own styles from before PE-DESIGN's types were measured sew as they
        // always did (see `RegionSewing::legacy_underlay`).
        let legacy = r.legacy_underlay();
        if legacy {
            let across = angle + 90.0;
            match r.underlay_style {
                UnderlayStyle::Edge => out.push(edge()),
                UnderlayStyle::Zigzag => out.push(rows(across)),
                UnderlayStyle::EdgeZigzag => {
                    out.push(edge());
                    out.push(rows(across));
                }
                _ => {
                    out.push(rows(angle + 45.0));
                    out.push(rows(angle - 45.0));
                }
            }
        }
        // The components combine (PE-DESIGN's types are combinations of them, see
        // `UnderlayParts`): the edge run first, then the piping, the rows, and the lattice.
        let parts = if legacy {
            UnderlayParts::default()
        } else {
            r.underlay_parts()
        };
        // The edge run belongs to the lattice's own sequence when nothing comes between them
        // (PE-DESIGN's second type: the edge run, then the two layers).
        let edge_with_lattice = parts.edge && parts.lattice && !parts.zigzag && !parts.center;
        let lattice = |with_edge: bool| -> Vec<Vec<Run>> {
            match cross_underlay(closed, r, angle, holes, with_edge) {
                Some(layers) => layers,
                None => {
                    let mut out = Vec::new();
                    if with_edge {
                        out.push(edge());
                    }
                    out.push(rows(angle + 45.0));
                    out.push(rows(angle - 45.0));
                    out
                }
            }
        };
        if r.kind == RegionType::Satin {
            let rows = parts.zigzag.then(satin_rows);
            if parts.edge && !edge_with_lattice {
                out.push(satin_edge(
                    rows.as_ref()
                        .and_then(|r| r.first().and_then(|r| r.first().copied())),
                ));
            }
            if parts.center
                && let Some(run) = center_row(closed, holes, angle, SATIN_UNDERLAY_INSETS[0])
            {
                out.push(vec![run]);
            }
            if let Some(rows) = rows {
                out.push(rows);
            }
            if parts.lattice {
                out.extend(lattice(edge_with_lattice));
            }
        } else {
            if (parts.edge && !edge_with_lattice) || parts.zigzag || parts.center {
                out.extend(fill_underlay(closed, r, angle, holes, parts));
            }
            if parts.lattice {
                out.extend(lattice(edge_with_lattice));
            }
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
            generate::fill_edge_to_edge(closed, &params, holes, r.fill_stagger(), r.entry)
        }
        // A feathered edge alone keeps the sawtooth (PE-DESIGN 11.50, satin-feather); so does
        // a pattern gradation on a satin split at its start point (satin-gradation).
        RegionType::Satin
            if !shaped || (r.sewn_gradation().is_none() && r.split_length.is_none()) =>
        {
            sawtooth_satin(closed, r, &params, holes)
        }
        RegionType::Satin if r.split_length.is_none() && r.sewn_feather().is_none() => {
            let lattice = r
                .sewn_gradation()
                .filter(|g| g.steps.is_some())
                .map(|g| row_positions(closed, params.angle, params.row_spacing, Some(g)));
            match (lattice, r.entry) {
                (Some(lattice), Some(e)) => {
                    split_satin(closed, holes, &params, Some(&lattice), e, None)
                        .map_or_else(|| shaped_rows(closed, r, &params, holes), |run| vec![run])
                }
                _ => shaped_rows(closed, r, &params, holes),
            }
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
    parts: UnderlayParts,
) -> Vec<Vec<Run>> {
    let mut shape: Vec<Contour> = closed.to_vec();
    shape.extend(holes.iter().cloned());
    let inset =
        crate::boolean::contours(&crate::boolean::offset(&shape, -FILL_UNDERLAY_INSET, false));
    let mut edge: Vec<Run> = if parts.edge {
        inset
            .iter()
            .filter(|c| c.closed && c.points.len() >= 3)
            .map(|c| match r.entry {
                Some(p) => generate::running(&c.starting_at(p), FILL_UNDERLAY_PITCH, 1),
                None => generate::running(c, FILL_UNDERLAY_PITCH, 1),
            })
            .collect()
    } else {
        Vec::new()
    };
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
        pull_compensation: 0.0,
    };
    let center = parts
        .center
        .then(|| center_row(closed, holes, angle, FILL_UNDERLAY_INSET))
        .flatten();
    let rows = if parts.zigzag {
        let l = p.stitch_length;
        generate::chain_spans(&inset_row_spans(&inset, &p), &p, |span, row, forward| {
            let mut xs = tatami_points(span, row, l, Stagger::Regular(FILL_UNDERLAY_SHIFT));
            if !forward {
                xs.reverse();
            }
            xs
        })
    } else {
        Vec::new()
    };
    let pieces: Vec<Run> = edge
        .into_iter()
        .chain(center)
        .chain(rows)
        .flat_map(|run| split_outside(run, closed, holes))
        .collect();
    vec![join(
        pieces,
        Some(&mut TravelGraph::new(closed, holes)),
        25.0,
    )]
}

/// Stitch pitch (0.1 mm) of the walks along the inset outline between the lines of
/// PE-DESIGN's cross under sewing: the 2.83 mm gap between lines 2 mm apart is sewn in two,
/// a 1.6 mm walk in one.
const CROSS_JOIN_PITCH: f64 = 20.0;
/// PE-DESIGN sews a cross under sewing line of exactly the stitch length in two stitches (a
/// 4.0 mm line in two of 2.0), so the lines are cut at this much less (0.1 mm).
const CROSS_LINE_SLACK: f64 = 0.2;
/// Pitch of the travel to each half's corner (0.1 mm): equal stitches up to 2.5 mm.
const CROSS_TRAVEL_PITCH: f64 = 25.0;

/// PE-DESIGN 11.50's second under sewing type, measured on its output (sweep-satin-us-type-2,
/// sweep-fill-us-type-2): the running stitch 1 mm inside the outline from the start point
/// (as the first type), then two layers of parallel lines 1 mm inside the outline, the first
/// at the sewing direction minus 45°, the second at plus 45°, `underlay_spacing` apart (2 mm
/// by default), in stitches of the region's stitch length. Each layer is sewn in two halves.
/// A half's lines lie one spacing apart from its far corner (the region's extreme point
/// across the lines) up to the line through the start point; the half travels straight to
/// that corner in 2.5 mm stitches, walks the inset outline to the line end nearer the start
/// point, sews the lines as a serpentine with the walks between them along the outline
/// (2 mm stitches), and walks back to the start point. The half whose corner comes first
/// along the outline from the start point goes first. `None` (the plain crossing rows) for a region whose inset is not a
/// single outline or that a line crosses more than once.
fn cross_underlay(
    closed: &[Contour],
    r: &RegionSewing,
    angle: f64,
    holes: &[Contour],
    with_edge: bool,
) -> Option<Vec<Vec<Run>>> {
    let inset: Vec<Contour> = if r.kind == RegionType::Satin {
        satin_inset(closed, holes)?
    } else {
        let mut shape: Vec<Contour> = closed.to_vec();
        shape.extend(holes.iter().cloned());
        crate::boolean::contours(&crate::boolean::offset(&shape, -FILL_UNDERLAY_INSET, false))
    };
    let outline: Vec<&Contour> = inset
        .iter()
        .filter(|c| c.closed && c.points.len() >= 3)
        .collect();
    let [outline] = outline[..] else {
        return None;
    };
    let entry = r
        .entry
        .or_else(|| closed.first().and_then(|c| c.points.first().copied()))?;
    let start = crate::route::snap(std::slice::from_ref(outline), entry);
    let mut pieces = Vec::new();
    if with_edge {
        let mut edge = generate::running(&outline.starting_at(start), FILL_UNDERLAY_PITCH, 1);
        if r.kind != RegionType::Satin {
            // The fill's edge run is entered from the start point on the outline.
            edge.insert(0, crate::route::snap(closed, entry));
        }
        pieces.push(edge);
    }
    let spacing = r.underlay_spacing_mm() * 10.0;
    let stitch = r.stitch_length * 10.0;
    for a in [angle + 45.0, angle - 45.0] {
        pieces.extend(cross_layer(outline, a, spacing, stitch, start)?);
    }
    Some(vec![join(
        pieces,
        Some(&mut TravelGraph::new(closed, holes)),
        CROSS_TRAVEL_PITCH,
    )])
}

/// One layer of [`cross_underlay`]: the lines along geometry angle `a`, in two halves.
fn cross_layer(
    outline: &Contour,
    a: f64,
    spacing: f64,
    stitch: f64,
    start: Vec2,
) -> Option<Vec<Run>> {
    let rad = a.to_radians();
    let points = outline.outline_points();
    let rotated: Vec<Vec2> = points.iter().map(|p| p.rotated(-rad)).collect();
    let s = start.rotated(-rad);
    let imin = (0..rotated.len()).min_by(|&i, &j| rotated[i].y.total_cmp(&rotated[j].y))?;
    let imax = (0..rotated.len()).max_by(|&i, &j| rotated[i].y.total_cmp(&rotated[j].y))?;
    let line = |y: f64| -> Option<Option<(Vec2, Vec2)>> {
        match spans_at(std::slice::from_ref(outline), &[], a, y).as_slice() {
            [] => Some(None),
            [span] => Some(Some((
                unrotate(*span.start(), y, a),
                unrotate(*span.end(), y, a),
            ))),
            _ => None,
        }
    };
    let mut halves: Vec<(Vec2, Vec<(Vec2, Vec2)>)> = Vec::new();
    for (corner, from, step) in [
        (points[imin], rotated[imin].y, spacing),
        (points[imax], rotated[imax].y, -spacing),
    ] {
        let mut lines = Vec::new();
        let mut y = from + step;
        while (s.y - y) * step.signum() > 1e-6 && lines.len() < 100_000 {
            if let Some(l) = line(y)? {
                lines.push(l);
            }
            y += step;
        }
        halves.push((corner, lines));
    }
    // The half whose corner comes first along the outline from the start point goes first
    // (PE-DESIGN: the left end of a rectangle started at the middle of its bottom edge, for
    // both layers).
    let total: f64 = outline
        .path()
        .windows(2)
        .map(|w| w[0].distance(w[1]))
        .sum::<f64>()
        .max(1e-9);
    let from_start = |p: Vec2| {
        (outline_position(outline, p) - outline_position(outline, start)).rem_euclid(total)
    };
    halves.sort_by(|a, b| from_start(a.0).total_cmp(&from_start(b.0)));
    let mut runs = Vec::new();
    for (corner, lines) in halves {
        if lines.is_empty() {
            continue;
        }
        let mut run = vec![corner];
        let mut at = corner;
        for (e0, e1) in lines {
            let (d0, d1) = (
                outline_distance(outline, at, e0),
                outline_distance(outline, at, e1),
            );
            let (near, far) = if (d0 - d1).abs() < 0.5 {
                // The first line from a corner: the end nearer the start point along the outline.
                if outline_distance(outline, e0, start) <= outline_distance(outline, e1, start) {
                    (e0, e1)
                } else {
                    (e1, e0)
                }
            } else if d0 <= d1 {
                (e0, e1)
            } else {
                (e1, e0)
            };
            run.extend(
                subdivide(&outline_walk(outline, at, near), CROSS_JOIN_PITCH)
                    .into_iter()
                    .skip(1),
            );
            run.extend(
                subdivide(&[near, far], (stitch - CROSS_LINE_SLACK).max(1.0))
                    .into_iter()
                    .skip(1),
            );
            at = far;
        }
        run.extend(
            subdivide(&outline_walk(outline, at, start), CROSS_JOIN_PITCH)
                .into_iter()
                .skip(1),
        );
        runs.push(run);
    }
    Some(runs)
}

/// Where `p` lies along `c`'s outline (flattened, closed): the distance along it from the
/// first point to the nearest point of the outline to `p`.
fn outline_position(c: &Contour, p: Vec2) -> f64 {
    let path = c.path();
    let (mut best, mut at, mut along) = (f64::INFINITY, 0.0, 0.0);
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (ab, ap) = (b - a, p - a);
        let len2 = ab.x * ab.x + ab.y * ab.y;
        let t = if len2 < 1e-18 {
            0.0
        } else {
            ((ap.x * ab.x + ap.y * ab.y) / len2).clamp(0.0, 1.0)
        };
        let d = p.distance(a.lerp(b, t));
        if d < best {
            (best, at) = (d, along + t * len2.sqrt());
        }
        along += len2.sqrt();
    }
    at
}

/// The shorter way along `c`'s outline from `a` to `b` (both on it), as a path through the
/// vertices between them.
fn outline_walk(c: &Contour, a: Vec2, b: Vec2) -> Vec<Vec2> {
    let path = c.path();
    let total: f64 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    if total < 1e-9 {
        return vec![a, b];
    }
    let (pa, pb) = (outline_position(c, a), outline_position(c, b));
    // Vertex positions along the outline, with the vertex itself.
    let mut vertices: Vec<(f64, Vec2)> = Vec::new();
    let mut along = 0.0;
    for w in path.windows(2) {
        along += w[0].distance(w[1]);
        vertices.push((along, w[1]));
    }
    // Forward: the vertices strictly between pa and pb going round in drawing order.
    let between = |from: f64, to: f64, v: f64| {
        if from <= to {
            v > from && v < to
        } else {
            v > from || v < to
        }
    };
    let forward: Vec<Vec2> = vertices
        .iter()
        .filter(|(v, _)| between(pa, pb, *v % total))
        .map(|(_, p)| *p)
        .collect();
    let f = (pb - pa).rem_euclid(total);
    let mut out = vec![a];
    if f <= total - f {
        out.extend(forward);
    } else {
        out.extend(
            vertices
                .iter()
                .filter(|(v, _)| between(pb, pa, *v % total))
                .map(|(_, p)| *p)
                .rev(),
        );
    }
    out.push(b);
    out
}

/// The shorter distance along `c`'s outline between `a` and `b` (both on it).
fn outline_distance(c: &Contour, a: Vec2, b: Vec2) -> f64 {
    outline_walk(c, a, b)
        .windows(2)
        .map(|w| w[0].distance(w[1]))
        .sum()
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
    let rows: Vec<(usize, f64, Vec<Span>)> = inset_rows(&inner, p.angle, p.row_spacing)
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
        let mut xs = tatami_points(span, i, p.stitch_length, Stagger::Third);
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

/// Row positions (rotated space) of PE-DESIGN's under sewing across the inset region
/// `inner`: a row on each inset edge and a whole number of equal gaps between them, the count
/// nearest to the height over `spacing` (its 11.50 output, sweep-fill-us-density-*: 38 mm of
/// inset square hold 19 gaps of 2.0 mm at Medium, 27 of 1.407 at Dense (1.4 nominal) and 11
/// of 3.455 at Light (3.5 nominal)). The edge rows are sampled just inside the edges.
fn inset_rows(inner: &[Contour], angle: f64, spacing: f64) -> Vec<f64> {
    let Some(b) = rotated_bounds(inner, angle) else {
        return Vec::new();
    };
    let h = b.height();
    if !(h > 0.0 && spacing > 0.0 && spacing.is_finite()) {
        return Vec::new();
    }
    let n = (h / spacing).round().clamp(1.0, 100_000.0) as usize;
    let inside = EDGE_ROW_INSET.min(h / 4.0);
    (0..=n)
        .map(|k| (b.min_y + h * k as f64 / n as f64).clamp(b.min_y + inside, b.max_y - inside))
        .collect()
}

/// The spans of [`inset_rows`] across `inner`, as rows for [`generate::chain_spans`].
fn inset_row_spans(inner: &[Contour], p: &RegionParams) -> Vec<RowSpan> {
    inset_rows(inner, p.angle, p.row_spacing)
        .into_iter()
        .enumerate()
        .flat_map(|(row, y)| {
            spans_at(inner, &[], p.angle, y)
                .into_iter()
                .map(move |range| RowSpan { row, y, range })
        })
        .collect()
}

/// The piping of an under sewing: a running stitch along the region's middle row (rotated
/// space, across the sewing direction) `inset` inside the outline, in stitches up to 2.5 mm
/// (PE-DESIGN 11.50 sews a Manual Punch column's piping in equal stitches of at most 2.5 mm).
/// `None` where the inset leaves no row.
fn center_row(closed: &[Contour], holes: &[Contour], angle: f64, inset: f64) -> Option<Run> {
    let mut shape: Vec<Contour> = closed.to_vec();
    shape.extend(holes.iter().cloned());
    let inner = crate::boolean::contours(&crate::boolean::offset(&shape, -inset, false));
    let b = rotated_bounds(&inner, angle + 90.0)?;
    let y = (b.min_y + b.max_y) / 2.0;
    let span = spans_at(&inner, &[], angle + 90.0, y)
        .into_iter()
        .max_by(|a, b| (a.end() - a.start()).total_cmp(&(b.end() - b.start())))?;
    let ends = [
        unrotate(*span.start(), y, angle + 90.0),
        unrotate(*span.end(), y, angle + 90.0),
    ];
    Some(subdivide(&ends, FILL_UNDERLAY_PITCH))
}

/// Running stitch of the travel along the edge to each half of a split satin (0.1 mm).
const SPLIT_TRAVEL_PITCH: f64 = 20.0;

/// How close to a split satin's start point a lattice line is the start point's own line
/// (0.1 mm): 0.05 mm, PE-DESIGN's file resolution. Such a line is the second half's hop; the
/// first half hops to the line before it (see [`split_satin`]).
const START_LINE_SLACK: f64 = 0.5;

/// The shortest single-line gap a split satin crosses next to its far end (0.1 mm): 1 mm.
/// PE-DESIGN 11.50 crosses a 2 mm one (the round-3 column at 0.5 lines/mm: lines 0 and 1,
/// then every other line) and not a 0.33 mm one (satin-feather at 3 lines/mm: the crossings
/// spread evenly instead); the bound lies between, pinned by nothing closer (the 40 x 6 mm
/// column at 1, 1.5 and 2 lines/mm would).
const CROSSING_MERGE_GAP: f64 = 10.0;

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
        .and_then(|e| split_satin(closed, holes, p, None, e, r.sewn_feather()))
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

/// Satin split at its start point, as PE-DESIGN 11.50 sews it (satin-zigzag-rect,
/// satin-gradation, sweep-satin-density-min): from the start point along the edge to one end
/// of the region, a sawtooth back towards the start point, a hop of one line along the far edge
/// and a crossing, home along the near edge; then the same from the other end. The satin's
/// lines lie on a lattice: `lattice` (rotated row positions from the region's first edge to its
/// last; the cumulative-line-count rows of a pattern gradation) or, without one, a line every
/// line spacing from each end. Each half hops to the last lattice line at or before the start
/// point coming from its end and sets its crossings on every other line counting back from the
/// hop, plus the far end's line. PE-DESIGN does that at every density: 0.3 mm hops at
/// 3 lines/mm; at 0.1 to 0.5 lines/mm (the round-3 column) a crossing on the line next to the
/// far end, then every other line up to the hop on the start point's own line; on the
/// gradation rows likewise, where a row within [`START_LINE_SLACK`] of the start point is the
/// second half's hop only. A single-line gap shorter than [`CROSSING_MERGE_GAP`] next to the
/// far end is not crossed: those crossings spread evenly (satin-feather at 3 lines/mm). `None`
/// where the split does not apply (holes, several contours, a start point within a line of
/// either end).
fn split_satin(
    closed: &[Contour],
    holes: &[Contour],
    p: &RegionParams,
    lattice: Option<&[f64]>,
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
    let line = p.row_spacing;
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
    // The lines of each half, from its own end towards the start point.
    let lines = |from_low: bool| -> Vec<f64> {
        match lattice {
            Some(l) if l.len() >= 2 => {
                if from_low {
                    l.to_vec()
                } else {
                    l.iter().rev().copied().collect()
                }
            }
            _ => {
                let (from, step) = if from_low { (lo, line) } else { (hi, -line) };
                (0..100_000)
                    .map(|k| from + step * k as f64)
                    .take_while(|y| (y - from).abs() <= hi - lo + 1e-9)
                    .collect()
            }
        }
    };
    let half = |from_low: bool| -> Option<Vec<(f64, Span)>> {
        let lines = lines(from_low);
        let sign = if from_low { 1.0 } else { -1.0 };
        // The hop line: the last lattice line at or before the start point coming from this
        // half's end. A line on the start point (the uniform lattice with the start on a line:
        // its edge rows sit [`EDGE_ROW_INSET`] inside the region, so the line lies that far
        // past the point) is the hop of both halves. A line within [`START_LINE_SLACK`] of the
        // start point without lying on it is the second half's hop only (satin-gradation:
        // row 12 lies 0.02 mm from the start point; PE-DESIGN's left half hops to row 11, its
        // right half crosses at the start point).
        let h = lines.iter().rposition(|&y| {
            let d = (at.y - y) * sign;
            if d.abs() <= EDGE_ROW_INSET + 1e-6 {
                true
            } else if d.abs() <= START_LINE_SLACK {
                !from_low
            } else {
                d > 0.0
            }
        })?;
        if h == 0 {
            return None;
        }
        // Crossings on every other line counting back from the hop, and on the far end's line.
        let mut idx: Vec<usize> = (1..h).rev().step_by(2).collect();
        if idx.last() != Some(&0) {
            idx.push(0);
        }
        idx.reverse();
        let mut ys: Vec<f64> = idx.iter().map(|&k| lines[k]).collect();
        // A single line between the far end's crossing and the next one, shorter than
        // [`CROSSING_MERGE_GAP`], is not crossed: the crossings before the hop spread evenly
        // from the far end's line to the last one instead (satin-feather at 3 lines/mm: 15
        // crossings 0.69 mm apart over 29 lines, then the 0.33 mm hop).
        if idx.get(1) == Some(&1) && ys.len() >= 3 && (ys[1] - ys[0]).abs() < CROSSING_MERGE_GAP {
            let (first, last, n) = (ys[0], ys[ys.len() - 1], ys.len() - 2);
            ys = (0..=n)
                .map(|k| first + (last - first) * k as f64 / n as f64)
                .collect();
        }
        let mut rows = Vec::new();
        for y in ys {
            rows.push((y, one(y)?));
        }
        // The hop on the start point's own line crosses at the start point itself (PE-DESIGN's
        // last crossing lands on it; the end lock steps back along that crossing).
        let hop = if (at.y - lines[h]).abs() <= START_LINE_SLACK {
            at.y
        } else {
            lines[h]
        };
        rows.push((hop, one(hop)?));
        Some(rows)
    };
    let pt = |x: f64, y: f64| unrotate(x, y, p.angle);
    let mut run = vec![e];
    for rows in [half(true)?, half(false)?] {
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
    let mut s = crate::skeleton::derive(closed).filter(|s| s.validate().is_ok())?;
    let spacing = 10.0 / r.density.max(0.1);
    let pull = r.pull_compensation * 10.0;
    // PE-DESIGN starts Auto satin at the start point (by default the outline's first point),
    // travels along the outline to the far end and sews the satin back to it.
    let start = r
        .entry
        .or_else(|| closed[0].points.first().copied())
        .map(|s| crate::route::snap(closed, s));
    // The skeleton begins at the start point's end.
    if let Some(st) = start
        && let (Some(&a), Some(&b)) = (s.points.first(), s.points.last())
        && b.distance(st) < a.distance(st)
    {
        s.points.reverse();
        if s.widths.len() == s.points.len() {
            s.widths.reverse();
        }
    }
    let edges: Vec<(Vec2, Vec2)> = closed[0].edges();
    // PE-DESIGN counts its rungs in whole spacings from the far end it sews first
    // (satin-auto-bend-90-nohalf: the outer edge's needle points lie at 0.667k mm from the
    // far edge), so the rungs take the lattice anchored on the far edge.
    let n = s.points.len();
    let beyond = match (s.points.get(n.wrapping_sub(2)), s.points.last()) {
        (Some(&p0), Some(&p1)) if start.is_some() && n >= 2 => {
            ray_to(p1, crate::paths::unit(p1 - p0), &edges)
        }
        _ => None,
    };
    let rungs = crate::skeleton::rungs_within_from(&s, closed, spacing, pull, beyond);
    if rungs.len() < 2 {
        return None;
    }
    let mut column: Vec<Vec2> = rungs.iter().map(|r| r[0]).collect();
    column.extend(rungs.iter().rev().map(|r| r[1]));
    if area(&column) < AUTO_COVERAGE * area(&closed[0].outline_points()) {
        return None;
    }
    let rungs = square_rungs(rungs, &edges);
    let mut rungs = fan_corners(reach_ends(rungs, closed, spacing), &closed[0]);
    // Sewn from the far end back to the start point: one needle point per rung, on the start
    // point's side for every other rung counted from the start, so the last needle point is
    // the start point itself (PE-DESIGN, satin-auto-bend-60 and -90-nohalf).
    rungs.reverse();
    // PE-DESIGN sews an odd number of needle points (every reference bend: 139, 141, 177),
    // so the first lies on the outer end of the far rung and the last on the start point;
    // an even count loses the rung after the far edge's.
    if rungs.len().is_multiple_of(2) && rungs.len() > 2 {
        rungs.remove(1);
    }
    let n = rungs.len();
    let last = rungs[n - 1];
    let start_side = match start {
        Some(st) if last[1].distance(st) < last[0].distance(st) => 1,
        _ => 0,
    };
    let mut run: Run = rungs
        .iter()
        .enumerate()
        .map(|(i, g)| {
            g[if (n - 1 - i).is_multiple_of(2) {
                start_side
            } else {
                1 - start_side
            }]
        })
        .collect();
    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    if let (Some(s), Some(&first)) = (start, run.first()) {
        let mut graph = TravelGraph::new(closed, holes);
        graph.outline = Some(OutlineTravel::Inset(AUTO_TRAVEL_INSET));
        graph.outline_tolerance = AUTO_TRAVEL_INSET + 1.0;
        // PE-DESIGN travels forward along the outline from the start point (over a bent
        // band's outer edge), not the shorter way round.
        graph.outline_forward = true;
        let path = graph.path(s, first).unwrap_or(vec![s, first]);
        let mut lead = subdivide(&path, SATIN_TRAVEL_PITCH);
        lead.pop();
        lead.extend(run);
        run = lead;
    }
    Some(run)
}

/// Auto satin's travel to its far end runs this far inside the outline (0.1 mm; PE-DESIGN
/// 11.50: 0.4-0.5 mm on the bent band).
const AUTO_TRAVEL_INSET: f64 = 5.0;

/// Rungs whose two ends lie on parallel straight edges are set square to those edges, from
/// the rung's middle: a derived skeleton wobbles by a grid cell, and PE-DESIGN's rungs on a
/// straight leg are square to it (satin-auto-bend-60: the needle points on the two edges
/// alternate at exactly half a spacing).
fn square_rungs(mut rungs: Vec<[Vec2; 2]>, edges: &[(Vec2, Vec2)]) -> Vec<[Vec2; 2]> {
    let edge_at = |p: Vec2| {
        edges
            .iter()
            .find(|&&(a, b)| crate::skeleton::segment_distance(p, a, b) <= 1.0)
            .map(|&(a, b)| crate::paths::unit(b - a))
    };
    for g in &mut rungs {
        let (Some(ua), Some(ub)) = (edge_at(g[0]), edge_at(g[1])) else {
            continue;
        };
        if crate::paths::cross(ua, ub).abs() > 0.035 {
            continue;
        }
        let m = g[0].lerp(g[1], 0.5);
        let n = v2(-ua.y, ua.x);
        let towards = |end: Vec2| {
            if crate::paths::dot(end - m, n) >= 0.0 {
                n
            } else {
                n * -1.0
            }
        };
        let (da, db) = (towards(g[0]), towards(g[1]));
        if let (Some(ta), Some(tb)) = (ray_to(m, da, edges), ray_to(m, db, edges)) {
            *g = [m + da * ta, m + db * tb];
        }
    }
    rungs
}

/// Distance from `p` along the unit `dir` to the nearest edge more than 0.1 mm away.
fn ray_to(p: Vec2, dir: Vec2, edges: &[(Vec2, Vec2)]) -> Option<f64> {
    edges
        .iter()
        .filter_map(|&(a, b)| {
            let e = b - a;
            let den = dir.x * e.y - dir.y * e.x;
            if den.abs() < 1e-12 {
                return None;
            }
            let w = a - p;
            let t = (w.x * e.y - w.y * e.x) / den;
            let u = (w.x * dir.y - w.y * dir.x) / den;
            (t > 1.0 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t)
        })
        .min_by(f64::total_cmp)
}

/// A derived column carried on to the outline at both ends, rows `spacing` apart and one on the
/// edge: the skeleton stops short of a band's square end, PE-DESIGN's satin reaches it.
fn reach_ends(rungs: Vec<[Vec2; 2]>, closed: &[Contour], spacing: f64) -> Vec<[Vec2; 2]> {
    if rungs.len() < 2 {
        return rungs;
    }
    let edges: Vec<(Vec2, Vec2)> = closed.iter().flat_map(Contour::edges).collect();
    let centre = |g: &[Vec2; 2]| g[0].lerp(g[1], 0.5);
    let beyond = |end: [Vec2; 2], next: [Vec2; 2]| -> Vec<[Vec2; 2]> {
        let out = centre(&end) - centre(&next);
        if out.length() < 1e-9 {
            return vec![end];
        }
        let dir = out * (1.0 / out.length());
        let Some(gap) = ray_to(centre(&end), dir, &edges) else {
            return vec![end];
        };
        // Rows a spacing apart, and one more on the edge.
        let mut steps: Vec<f64> = vec![0.0];
        let mut d = spacing;
        while d < gap - spacing * 0.5 {
            steps.push(d);
            d += spacing;
        }
        if gap > spacing * 0.25 {
            steps.push(gap - 0.1);
        }
        steps
            .into_iter()
            .map(|d| [end[0] + dir * d, end[1] + dir * d])
            .collect()
    };
    let n = rungs.len();
    let head = beyond(rungs[0], rungs[1]);
    let tail = beyond(rungs[n - 1], rungs[n - 2]);
    let mut out: Vec<[Vec2; 2]> = head.into_iter().rev().collect();
    out.extend(rungs[1..n - 1].iter().copied());
    out.extend(tail);
    out
}

/// PE-DESIGN 11.50's corner fan on Auto satin (satin-auto-bend-90-nohalf, satin-auto-bend-60):
/// near each outer corner of the outline the rungs keep their outer ends, and their direction
/// turns evenly with the distance along the outer edge, from square to one leg to square to the
/// other, over three band widths for a 90° turn (in proportion for other turns), centred on the
/// corner; each rung then reaches across to the outline. The inner ends crowd towards the inner
/// corner, as PE's do.
fn fan_corners(mut rungs: Vec<[Vec2; 2]>, outline: &Contour) -> Vec<[Vec2; 2]> {
    let pts = outline.outline_points();
    let n = pts.len();
    if n < 3 || rungs.is_empty() {
        return rungs;
    }
    let edges: Vec<(Vec2, Vec2)> = outline.edges();
    let ccw = (0..n)
        .map(|i| crate::paths::cross(pts[i], pts[(i + 1) % n]))
        .sum::<f64>()
        > 0.0;
    let inward = |d: Vec2| if ccw { v2(-d.y, d.x) } else { v2(d.y, -d.x) };
    let mut widths: Vec<f64> = rungs.iter().map(|g| g[0].distance(g[1])).collect();
    widths.sort_by(f64::total_cmp);
    let width = widths[widths.len() / 2];
    let corner = |k: usize| {
        let (a, v, b) = (pts[(k + n - 1) % n], pts[k], pts[(k + 1) % n]);
        let (din, dout) = (crate::paths::unit(v - a), crate::paths::unit(b - v));
        let turn = crate::paths::dot(din, dout).clamp(-1.0, 1.0).acos();
        let convex = (crate::paths::cross(din, dout) > 0.0) == ccw;
        (a, v, b, din, dout, turn, convex)
    };
    // Inner corners of bends; a band's square end has outer corners but no inner one near.
    let inner: Vec<Vec2> = (0..n)
        .filter(|&k| {
            let (_, _, _, _, _, turn, convex) = corner(k);
            !convex && turn >= 15f64.to_radians()
        })
        .map(|k| pts[k])
        .collect();
    let mut done = vec![false; rungs.len()];
    for k in 0..n {
        let (a, v, b, din, dout, turn, convex) = corner(k);
        if !convex
            || turn < 15f64.to_radians()
            || !inner.iter().any(|&q| q.distance(v) <= 2.0 * width)
        {
            continue;
        }
        let zone = 3.0 * width * turn / std::f64::consts::FRAC_PI_2;
        let (nin, nout) = (inward(din), inward(dout));
        let (ain, aout) = (nin.y.atan2(nin.x), nout.y.atan2(nout.x));
        let mut sweep = aout - ain;
        while sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        }
        while sweep < -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }
        let on = |p: Vec2, s: Vec2, e: Vec2| crate::skeleton::segment_distance(p, s, e) <= 1.0;
        for (i, g) in rungs.iter_mut().enumerate() {
            if done[i] {
                continue;
            }
            let Some((side, u)) = (0..2).find_map(|s| {
                let e = g[s];
                if on(e, a, v) {
                    Some((s, -e.distance(v)))
                } else if on(e, v, b) {
                    Some((s, e.distance(v)))
                } else {
                    None
                }
            }) else {
                continue;
            };
            if u.abs() > zone / 2.0 {
                continue;
            }
            let angle = ain + sweep * (u + zone / 2.0) / zone;
            let dir = v2(angle.cos(), angle.sin());
            if let Some(d) = ray_to(g[side], dir, &edges) {
                g[1 - side] = g[side] + dir * d;
                done[i] = true;
            }
        }
    }
    rungs
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
    let stagger = r.fill_stagger();
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
            tatami_points(&span, row, l, stagger)
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
fn tatami_points(span: &Span, row: usize, l: f64, stagger: Stagger) -> Vec<f64> {
    let phase = generate::row_phase(row, stagger) * l;
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
/// A pattern (PE-DESIGN) gradation sews rows at equal steps of the cumulative line count: the
/// density falls linearly from Max to Min across the region, the region holds a whole number
/// of gaps (the line count the profile integrates to, rounded down), and row k lies where the
/// density integrated from the start edge reaches k gaps' worth, so the first row is on the
/// start edge and the last on the far edge. This is PE-DESIGN 11.50's placement
/// (region-travel-u-gradation: 40 rows over 30 mm at 4.5 lines/mm, all within 0.12 mm,
/// 0.04 mm on average; placing each gap at its midpoint density drifted 0.4 mm by the far
/// edge). A continuous one keeps the placement designs saved before patterns had: rows from
/// half a spacing in.
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
    if let Some(g) = gradation.filter(|g| g.steps.is_some()) {
        // Rows on an edge are sampled just inside it.
        let inside = 1e-3_f64.min(h / 4.0);
        // Density fraction a at the start edge, b at the far edge; the line count integrated
        // up to y (from the start edge) is (a·y + (b − a)·y²/2h) / spacing.
        let (a, b_far) = (g.fraction(0.0), g.fraction(1.0));
        let total = h * (a + b_far) / 2.0 / spacing;
        let n = (total + 1e-9).floor().clamp(1.0, 100_000.0);
        let q = (b_far - a) / (2.0 * h);
        for k in 0..=n as usize {
            let target = total * k as f64 / n;
            // Solve q·y² + a·y − target·spacing = 0 for y ≥ 0.
            let c = -target * spacing;
            let y = if q.abs() < 1e-12 {
                -c / a
            } else {
                (-a + (a * a - 4.0 * q * c).max(0.0).sqrt()) / (2.0 * q)
            };
            let y = if k == n as usize { h } else { y };
            ys.push((b.min_y + y).clamp(b.min_y + inside, b.max_y - inside));
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
