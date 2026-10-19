//! Manual Punch: shapes made of several sewing-direction lines (PE-DESIGN's "Using Manual
//! Punch Tool" and "Specifying the straight sewing direction" with more than one line), and
//! satin columns along a skeleton (centre line with per-point width).

use crate::commands::{CommandSpec, Effect, default_fill, opt, req, spec, thread};
use crate::params::{CommandError, Params};
use openstitch_design::column::{self, RungError};
use openstitch_design::skeleton::{self, Skeleton, SkeletonError};
use openstitch_design::{DesignObject, LineSewing, LineType, RegionSewing, RegionType, Vec2};
use serde_json::json;

/// Most direction lines per shape.
pub const MAX_LINES: usize = 1_000;

fn rungs(p: &Params) -> Result<Vec<[Vec2; 2]>, CommandError> {
    let arr = p.array("lines")?;
    if arr.len() < 2 || arr.len() > MAX_LINES {
        return Err(p.invalid("lines", format!("needs 2...{MAX_LINES} lines")));
    }
    let lines = arr
        .iter()
        .map(|v| match v.as_array() {
            Some(a) if a.len() == 2 => Ok([
                p.point_value("lines", &a[0])?,
                p.point_value("lines", &a[1])?,
            ]),
            _ => Err(p.invalid("lines", "each line must be [[x1, y1], [x2, y2]] in mm")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    column::validate(&lines).map_err(|e| match e {
        RungError::TooFew => p.invalid("lines", "needs at least 2 lines"),
        RungError::Degenerate(i) => p.invalid("lines", format!("line {i} is too short")),
        RungError::Crossing(i) => p.invalid(
            "lines",
            format!("lines {i} and {} (or the sides between them) cross", i + 1),
        ),
    })?;
    Ok(lines)
}

/// `points` (mm) with `widths` (mm, one per point) or one `width` (default 3 mm).
fn skeleton_param(p: &Params) -> Result<Skeleton, CommandError> {
    let points = p.points("points", 2)?;
    if points.len() > skeleton::MAX_POINTS {
        return Err(p.invalid("points", format!("at most {} points", skeleton::MAX_POINTS)));
    }
    if p.has("widths") && p.has("width") {
        return Err(p.invalid("widths", "give widths or width, not both"));
    }
    let widths = if p.has("widths") {
        p.array("widths")?
            .iter()
            .map(|v| {
                v.as_f64()
                    .ok_or_else(|| p.invalid("widths", "must be numbers (mm)"))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let w = p.opt("width", skeleton::DEFAULT_WIDTH, 0.1, skeleton::MAX_WIDTH)?;
        vec![w; points.len()]
    };
    let s = Skeleton { points, widths };
    s.validate().map_err(|e| match e {
        SkeletonError::TooFew => p.invalid("points", "needs at least 2 points"),
        SkeletonError::TooMany => {
            p.invalid("points", format!("at most {} points", skeleton::MAX_POINTS))
        }
        SkeletonError::WidthCount => p.invalid("widths", "needs one width per point"),
        SkeletonError::Width(i) => p.invalid(
            "widths",
            format!("width {i} must be 0...{} mm", skeleton::MAX_WIDTH),
        ),
        SkeletonError::Thin => p.invalid("widths", "at least one width must be 0.1 mm or more"),
        SkeletonError::Short => p.invalid("points", "the centre line is too short"),
    })?;
    Ok(s)
}

fn kind(p: &Params) -> Result<RegionType, CommandError> {
    match p.string("type", Some("satin"))?.as_str() {
        "satin" => Ok(RegionType::Satin),
        "fill" => Ok(RegionType::Fill),
        _ => Err(p.invalid("type", "must be satin or fill")),
    }
}

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "shape.column",
            "Add a Manual Punch shape: a column through direction lines, sewn along them",
            Effect::Edit,
            vec![
                req(
                    "lines",
                    "[[[x1, y1], [x2, y2]]] mm",
                    "Direction lines in order (2-1000); first ends make one side, second ends the other",
                ),
                opt("type", "satin|fill", "Region sew type (default satin)"),
                opt("fill", "color", "Region thread (default Brother Blue)"),
                opt(
                    "outline",
                    "color",
                    "Outline running-stitch thread (default none)",
                ),
                opt("name", "string", "Object name"),
            ],
            |s, p| {
                let lines = rungs(p)?;
                let mut region =
                    RegionSewing::new(kind(p)?, thread(p, "fill")?.unwrap_or_else(default_fill));
                region.direction_lines = lines.clone();
                let line = thread(p, "outline")?.map(|t| LineSewing::new(LineType::Running, t));
                let mut o = DesignObject::shape(vec![column::outline(&lines)], Some(region), line);
                o.name = Some(if p.has("name") {
                    p.string("name", None)?
                } else {
                    "Manual punch".into()
                });
                Ok(json!({ "id": s.design.add(o) }))
            },
        ),
        spec(
            "object.setDirectionLines",
            "Sew a satin or fill region along several direction lines (null = back to one direction)",
            Effect::Edit,
            vec![
                req("id", "int", "Object with satin or fill region sewing"),
                req(
                    "lines",
                    "[[[x1, y1], [x2, y2]]] mm | null",
                    "Direction lines in order",
                ),
            ],
            |s, p| {
                let lines = if p.is_null("lines") {
                    Vec::new()
                } else {
                    rungs(p)?
                };
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                let o = &mut s.design.objects[i];
                if o.hole_sewing && !lines.is_empty() {
                    return Err(
                        p.failed("direction lines cannot be added to a shape with hole sewing")
                    );
                }
                let r = o
                    .region
                    .as_mut()
                    .ok_or_else(|| p.failed(format!("object {id} has no region sewing")))?;
                if !lines.is_empty() && !matches!(r.kind, RegionType::Satin | RegionType::Fill) {
                    return Err(p.failed(format!(
                        "object {id}: direction lines work with satin and fill"
                    )));
                }
                r.direction_lines = lines;
                Ok(json!(null))
            },
        ),
        spec(
            "shape.skeletonSatin",
            "Add a satin column along a centre line (skeleton): stitches stay perpendicular to the line, width given per point",
            Effect::Edit,
            vec![
                req("points", "[[x, y]] mm", "Centre line (2-10000 points)"),
                opt("widths", "[mm]", "Column width at each point (0-30)"),
                opt("width", "mm", "One width for every point (default 3)"),
                opt("density", "lines/mm", "Satin density (default 3)"),
                opt("fill", "color", "Satin thread (default Brother Blue)"),
                opt(
                    "outline",
                    "color",
                    "Outline running-stitch thread (default none)",
                ),
                opt("name", "string", "Object name"),
            ],
            |s, p| {
                let sk = skeleton_param(p)?;
                let outline = skeleton::outline(&sk)
                    .ok_or_else(|| p.invalid("points", "the centre line is too short"))?;
                let mut region = RegionSewing::new(
                    RegionType::Satin,
                    thread(p, "fill")?.unwrap_or_else(default_fill),
                );
                region.density = p.opt("density", region.density, 0.5, 10.0)?;
                region.skeleton = Some(sk);
                let line = thread(p, "outline")?.map(|t| LineSewing::new(LineType::Running, t));
                let mut o = DesignObject::shape(vec![outline], Some(region), line);
                o.name = Some(if p.has("name") {
                    p.string("name", None)?
                } else {
                    "Skeleton satin".into()
                });
                Ok(json!({ "id": s.design.add(o) }))
            },
        ),
        spec(
            "object.setSkeleton",
            "Sew a satin region along a centre line with per-point width (points null = back to normal satin)",
            Effect::Edit,
            vec![
                req("id", "int", "Object with satin region sewing"),
                req("points", "[[x, y]] mm | null", "Centre line"),
                opt("widths", "[mm]", "Column width at each point (0-30)"),
                opt("width", "mm", "One width for every point (default 3)"),
                opt(
                    "reshape",
                    "bool",
                    "Replace the outline with the column's (default false)",
                ),
            ],
            |s, p| {
                let sk = if p.is_null("points") {
                    None
                } else {
                    Some(skeleton_param(p)?)
                };
                let reshape = p.bool("reshape", Some(false))?;
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                let o = &mut s.design.objects[i];
                if o.hole_sewing && sk.is_some() {
                    return Err(p.failed("a skeleton cannot be added to a shape with hole sewing"));
                }
                let r = o
                    .region
                    .as_mut()
                    .ok_or_else(|| p.failed(format!("object {id} has no region sewing")))?;
                if sk.is_some() && r.kind != RegionType::Satin {
                    return Err(p.failed(format!("object {id}: a skeleton works with satin")));
                }
                let outline = match (&sk, reshape) {
                    (Some(sk), true) => Some(
                        skeleton::outline(sk)
                            .ok_or_else(|| p.invalid("points", "the centre line is too short"))?,
                    ),
                    _ => None,
                };
                r.skeleton = sk;
                if let Some(c) = outline {
                    o.contours = vec![c];
                }
                Ok(json!(null))
            },
        ),
    ]
}
