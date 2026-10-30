//! Background fill around patterns (PE-DESIGN Tutorial 8, "Specify settings for background
//! fill pattern"): fill a closed shape, or the hoop, around the selected patterns with echo
//! lines, stippling, a decorative fill, crosshatch or stripes, keeping a set distance from the
//! patterns.

use super::{MAX_PIECES, difference, footprint, insert_at, link, selected};
use crate::commands::{
    CommandSpec, Effect, MM, default_fill, default_line, opt, req, spec, thread,
};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_design::boolean;
use openstitch_design::fmath::Det;
use openstitch_design::paths;
use openstitch_design::patterns::{self, PatternKind};
use openstitch_design::{
    ChainShape, Contour, Design, DesignObject, LineSewing, LineType, ObjectKind, Rect,
    RegionSewing, RegionType, RunningStitchPath, Vec2, v2,
};
use serde_json::json;

/// Most echo rings, whatever the area's size.
const MAX_RINGS: usize = 400;

pub fn commands() -> Vec<CommandSpec> {
    vec![spec(
        "wizard.backgroundFill",
        "Background fill: fill a closed shape or the hoop around the patterns, keeping a gap (one new object sewn before them)",
        Effect::Edit,
        vec![
            req("ids", "[int]", "Patterns to fill around"),
            opt(
                "area",
                "int",
                "Closed shape to fill (default: the hoop, or the Design Page without one)",
            ),
            opt(
                "fillType",
                "echo|stippling|decorative|crosshatch|stripes",
                "Fill type (default echo)",
            ),
            opt(
                "spacing",
                "mm",
                "Gap around the patterns; echo: also between lines (echo 1-30, default 5; stippling 1-7, others 0-20, default 2)",
            ),
            opt(
                "echoType",
                "clipped|echo",
                "Echo: copies of the patterns' outline clipped to the area, or rings between both outlines (default clipped)",
            ),
            opt(
                "linkEchoEnds",
                "bool",
                "Echo clipped, crosshatch, stripes: join line ends inside the area (default false)",
            ),
            opt(
                "sew",
                "running|triple|chain",
                "Echo, crosshatch, stripes: line sew type (default running)",
            ),
            opt(
                "pitch",
                "mm",
                "Running / triple stitch length 1-10 (default 2.5)",
            ),
            opt(
                "runTimes",
                "int 1-5",
                "Running stitch passes; chain: times (default 1)",
            ),
            opt(
                "chainShape",
                "triangle|diamond",
                "Chain stitch type (default triangle)",
            ),
            opt("chainSize", "mm", "Chain stitch size 1-10 (default 2)"),
            opt(
                "color",
                "color",
                "Thread (default black for lines, blue for fills)",
            ),
            opt(
                "stippleSpacing",
                "mm",
                "Stippling: distance between stippling lines 0.5-10 (default 2)",
            ),
            opt(
                "pattern",
                "string",
                "Decorative fill pattern id (patterns.list)",
            ),
            opt("patternWidth", "mm", "Decorative pattern width 2-50"),
            opt("patternHeight", "mm", "Decorative pattern height 2-50"),
            opt(
                "connectEdges",
                "bool",
                "Decorative: connect pattern ends with running stitches inside the area (default true)",
            ),
            opt(
                "lineSpacing",
                "mm",
                "Crosshatch / stripes: line spacing 7-30 (default 10)",
            ),
            opt(
                "angle",
                "degrees",
                "Crosshatch / stripes: line angle 45-90, counter-clockwise (default 45)",
            ),
            opt(
                "skipOverlap",
                "bool",
                "Leave the area under the patterns unsewn (default true; false fills it all, sewn first)",
            ),
        ],
        background_fill,
    )]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FillType {
    Echo,
    Stippling,
    Decorative,
    Crosshatch,
    Stripes,
}

fn background_fill(s: &mut EngineState, p: &Params) -> crate::params::CommandResult {
    let idx = selected(p, &s.design)?;
    let fill = match p.string("fillType", Some("echo"))?.as_str() {
        "echo" => FillType::Echo,
        "stippling" => FillType::Stippling,
        "decorative" => FillType::Decorative,
        "crosshatch" => FillType::Crosshatch,
        "stripes" => FillType::Stripes,
        _ => {
            return Err(p.invalid(
                "fillType",
                "must be echo, stippling, decorative, crosshatch or stripes",
            ));
        }
    };
    let spacing = match fill {
        FillType::Echo => p.opt("spacing", 5.0, 1.0, 30.0)?,
        FillType::Stippling => p.opt("spacing", 2.0, 1.0, 7.0)?,
        _ => p.opt("spacing", 2.0, 0.0, 20.0)?,
    } * MM;
    let area = area(s, p, &idx)?;
    let around = footprint(&s.design, &idx);
    if around.is_empty() {
        return Err(p.failed("the patterns have no outline or stitches to fill around"));
    }
    let skip = p.bool("skipOverlap", Some(true))?;
    let color = thread(p, "color")?;
    let (object, lines) = match fill {
        FillType::Echo => {
            let line = line_sewing(p, color.unwrap_or_else(default_line))?;
            let contours = match p.string("echoType", Some("clipped"))?.as_str() {
                "clipped" => echo_clipped(p, &area, &around, spacing)?,
                "echo" => echo_rings(&area, &around, spacing),
                _ => return Err(p.invalid("echoType", "must be clipped or echo")),
            };
            line_object(p, contours, line)?
        }
        FillType::Crosshatch | FillType::Stripes => {
            let line = line_sewing(p, color.unwrap_or_else(default_line))?;
            let gap = p.opt("lineSpacing", 10.0, 7.0, 30.0)? * MM;
            let angle = p.opt("angle", 45.0, 45.0, 90.0)?;
            let region = fill_region(&area, &around, spacing, skip);
            let mut angles = vec![angle];
            if fill == FillType::Crosshatch {
                angles.push(angle + 90.0);
            }
            let mut pieces = Vec::new();
            for a in angles {
                // Counter-clockwise on screen, as region directions are.
                pieces.extend(hatch(
                    &region,
                    openstitch_design::region::geometry_angle(a),
                    gap,
                ));
                if pieces.len() > MAX_PIECES {
                    return Err(p.failed("the line spacing is too small for this area"));
                }
            }
            let contours = if p.bool("linkEchoEnds", Some(false))? {
                link(pieces, &region, 2.0 * gap)
            } else {
                paths::order(pieces, None)
                    .into_iter()
                    .map(|q| Contour::new(q, false))
                    .collect()
            };
            line_object(p, contours, line)?
        }
        FillType::Stippling | FillType::Decorative => {
            let region = fill_region(&area, &around, spacing, skip);
            if region.is_empty() {
                return Err(p.failed("nothing is left of the area to fill"));
            }
            let thread = color.unwrap_or_else(default_fill);
            let sewing = if fill == FillType::Stippling {
                let mut r = RegionSewing::new(RegionType::Stippling, thread);
                r.spacing = Some(p.opt("stippleSpacing", 2.0, 0.5, 10.0)?);
                r
            } else {
                let mut r = RegionSewing::new(RegionType::DecorativeFill, thread);
                if p.has("pattern") {
                    let id = p.string("pattern", None)?;
                    if patterns::find(PatternKind::Decorative, &id).is_none() {
                        return Err(p.invalid("pattern", "is not a decorative fill pattern"));
                    }
                    r.pattern = Some(id);
                }
                if p.has("patternWidth") {
                    r.pattern_width = Some(p.req("patternWidth", 2.0, 50.0)?);
                }
                if p.has("patternHeight") {
                    r.pattern_height = Some(p.req("patternHeight", 2.0, 50.0)?);
                }
                r.running_stitch_path = p
                    .bool("connectEdges", Some(true))?
                    .then_some(RunningStitchPath::Inside);
                r
            };
            let n = region.len();
            (DesignObject::shape(region, Some(sewing), None), n)
        }
    };
    let mut object = object;
    object.name = Some("Background fill".into());
    let first = idx[0];
    let id = insert_at(&mut s.design, first, object);
    Ok(json!({ "id": id, "contours": lines }))
}

/// The closed outline to fill: the `area` shape, or the hoop (Design Page without one).
fn area(s: &EngineState, p: &Params, idx: &[usize]) -> Result<Vec<Contour>, CommandError> {
    if !p.has("area") {
        return Ok(vec![hoop(&s.design)]);
    }
    let id = p.int("area", None, 0, i64::MAX)? as u64;
    let i = s
        .design
        .index_of(id)
        .ok_or_else(|| p.not_found(format!("object {id}")))?;
    if idx.contains(&i) {
        return Err(p.invalid("area", "must not be one of the patterns to fill around"));
    }
    let o = &s.design.objects[i];
    if o.kind == ObjectKind::Stitches || !o.has_closed_contour() {
        return Err(p.failed(format!("object {id} has no closed outline to fill")));
    }
    let closed: Vec<Contour> = o.contours.iter().filter(|c| c.closed).cloned().collect();
    let region = boolean::contours(&boolean::simplify(&closed));
    if region.is_empty() {
        return Err(p.failed(format!("object {id} has no area")));
    }
    Ok(region)
}

/// The selected hoop, centred on the Design Page, or the Design Page itself.
fn hoop(d: &Design) -> Contour {
    let c = d.page.center();
    let (w, h) = d
        .machine
        .hoop_size()
        .map(|(w, h)| (w * MM, h * MM))
        .unwrap_or((d.page.width, d.page.height));
    Contour::rectangle(c.x - w / 2.0, c.y - h / 2.0, w, h)
}

/// The patterns' footprint grown by `gap` (the footprint itself for no gap).
fn grown(around: &[Contour], gap: f64) -> Vec<Contour> {
    if gap <= 0.0 {
        return around.to_vec();
    }
    boolean::contours(&boolean::offset(around, gap, true))
}

/// The area to fill: `area` minus the patterns and their gap, or all of `area`.
fn fill_region(area: &[Contour], around: &[Contour], gap: f64, skip: bool) -> Vec<Contour> {
    if skip {
        difference(area, &grown(around, gap))
    } else {
        difference(area, &[])
    }
}

/// Echo clipped: copies of the patterns' outline at `spacing`, 2 × `spacing`, … clipped to the
/// area, outward until they no longer reach it.
fn echo_clipped(
    p: &Params,
    area: &[Contour],
    around: &[Contour],
    spacing: f64,
) -> Result<Vec<Contour>, CommandError> {
    let reach = Rect::enclosing(area.iter().chain(around).flat_map(Contour::outline_points))
        .map_or(0.0, |b| b.width().dhypot(b.height()));
    let rings = ((reach / spacing).ceil() as usize + 1).min(MAX_RINGS);
    let mut pieces = Vec::new();
    for k in 1..=rings {
        for ring in boolean::contours(&boolean::offset(around, spacing * k as f64, true)) {
            pieces.extend(paths::clip(&ring.path(), area, &[]));
        }
        if pieces.len() > MAX_PIECES {
            return Err(p.failed("the echo lines are too many for this area"));
        }
    }
    if pieces.is_empty() {
        return Err(p.failed("no echo line falls inside the area"));
    }
    if p.bool("linkEchoEnds", Some(false))? {
        let inside = difference(area, &grown(around, spacing / 2.0));
        return Ok(link(pieces, &inside, 2.0 * spacing));
    }
    Ok(paths::order(pieces, None)
        .into_iter()
        .map(|q| Contour::new(q, false))
        .collect())
}

/// Echo: rings between the area's outline and the patterns' outline, each `spacing` in from
/// the last, starting `spacing` away from both.
fn echo_rings(area: &[Contour], around: &[Contour], spacing: f64) -> Vec<Contour> {
    let inset = boolean::contours(&boolean::offset(area, -spacing, true));
    let base = difference(&inset, &grown(around, spacing));
    let mut out = Vec::new();
    for k in 0..MAX_RINGS {
        let ring = if k == 0 {
            base.clone()
        } else {
            boolean::contours(&boolean::offset(&base, -spacing * k as f64, true))
        };
        if ring.is_empty() || out.len() + ring.len() > MAX_PIECES {
            break;
        }
        out.extend(ring);
    }
    out
}

/// Parallel lines `gap` apart at `angle` degrees, clipped to `region`.
fn hatch(region: &[Contour], angle: f64, gap: f64) -> Vec<Vec<Vec2>> {
    let Some(b) = Rect::enclosing(region.iter().flat_map(Contour::outline_points)) else {
        return Vec::new();
    };
    let r = angle.to_radians();
    let (u, n) = (v2(r.dcos(), r.dsin()), v2(-r.dsin(), r.dcos()));
    let corners = [
        v2(b.min_x, b.min_y),
        v2(b.max_x, b.min_y),
        v2(b.min_x, b.max_y),
        v2(b.max_x, b.max_y),
    ];
    let range = |d: Vec2| {
        let v = corners.map(|c| c.x * d.x + c.y * d.y);
        (
            v.iter().copied().fold(f64::INFINITY, f64::min),
            v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let ((ulo, uhi), (nlo, nhi)) = (range(u), range(n));
    let rows = (((nhi - nlo) / gap).floor() as usize).min(MAX_PIECES);
    let offset = (nhi - nlo - rows as f64 * gap) / 2.0;
    let mut out = Vec::new();
    for k in 0..=rows {
        let t = nlo + offset + k as f64 * gap;
        let (a, z) = (u * (ulo - 1.0) + n * t, u * (uhi + 1.0) + n * t);
        let mut row = paths::clip(&[a, z], region, &[]);
        if k % 2 == 1 {
            row.reverse();
            row.iter_mut().for_each(|q| q.reverse());
        }
        out.extend(row);
    }
    out
}

/// Line sewing for echo, crosshatch and stripe lines.
fn line_sewing(p: &Params, thread: openstitch_core::Thread) -> Result<LineSewing, CommandError> {
    let kind = match p.string("sew", Some("running"))?.as_str() {
        "running" => LineType::Running,
        "triple" => LineType::Triple,
        "chain" => LineType::Chain,
        _ => return Err(p.invalid("sew", "must be running, triple or chain")),
    };
    let mut l = LineSewing::new(kind, thread);
    l.pitch = p.opt("pitch", 2.5, 1.0, 10.0)?;
    l.run_times = p.int("runTimes", Some(1), 1, 5)? as u32;
    l.chain_shape = match p.string("chainShape", Some("triangle"))?.as_str() {
        "triangle" => ChainShape::Triangle,
        "diamond" => ChainShape::Diamond,
        _ => return Err(p.invalid("chainShape", "must be triangle or diamond")),
    };
    if kind == LineType::Chain {
        l.width = p.opt("chainSize", 2.0, 1.0, 10.0)?;
    }
    Ok(l)
}

fn line_object(
    p: &Params,
    contours: Vec<Contour>,
    line: LineSewing,
) -> Result<(DesignObject, usize), CommandError> {
    if contours.is_empty() {
        return Err(p.failed("nothing is left of the area to fill"));
    }
    let n = contours.len();
    Ok((DesignObject::shape(contours, None, Some(line)), n))
}
