//! Manual Punch: shapes made of several sewing-direction lines (PE-DESIGN's "Using Manual
//! Punch Tool" and "Specifying the straight sewing direction" with more than one line), and
//! satin columns along a skeleton (centre line with per-point width). Curved Block columns,
//! Feed patterns and Reverse Direction Line.

use crate::commands::{
    CommandSpec, Effect, IDS, default_fill, default_line, indexes, opt, req, spec, thread,
};
use crate::params::{CommandError, Params};
use openstitch_core::{Command, Stitch};
use openstitch_design::column::{self, RungError};
use openstitch_design::skeleton::{self, Skeleton, SkeletonError};
use openstitch_design::{
    DesignObject, LineSewing, LineType, RegionSewing, RegionType, StitchBlock, Vec2,
};
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

/// The open curve `c` from its other end.
fn reversed(c: &openstitch_design::Contour) -> openstitch_design::Contour {
    let mut points = c.points.clone();
    points.reverse();
    let mut out = openstitch_design::Contour::new(points, c.closed);
    if c.has_curves() {
        let n = c.segment_count();
        out.curves = (0..n)
            .rev()
            .map(|i| {
                c.curve(i)
                    .map(|z| openstitch_design::Bezier { c1: z.c2, c2: z.c1 })
            })
            .collect();
    }
    out
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
                opt(
                    "curved",
                    "bool",
                    "Curved Block: the sides curve through the line ends (default false, Straight Block)",
                ),
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
                let curved = p.bool("curved", Some(false))?;
                if curved && column::validate(&column::curved_rungs(&lines)).is_err() {
                    return Err(p.invalid("lines", "the curved sides cross"));
                }
                let mut region =
                    RegionSewing::new(kind(p)?, thread(p, "fill")?.unwrap_or_else(default_fill));
                region.direction_lines = lines.clone();
                region.curved_lines = curved;
                let line = thread(p, "outline")?.map(|t| LineSewing::new(LineType::Running, t));
                let outline = if curved {
                    column::curved_outline(&lines)
                } else {
                    column::outline(&lines)
                };
                let mut o = DesignObject::shape(vec![outline], Some(region), line);
                o.name = Some(if p.has("name") {
                    p.string("name", None)?
                } else if curved {
                    "Curved block".into()
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
                if lines.is_empty() {
                    r.curved_lines = false;
                } else if r.curved_lines && column::validate(&column::curved_rungs(&lines)).is_err()
                {
                    return Err(p.invalid("lines", "the curved sides cross"));
                }
                r.direction_lines = lines;
                Ok(json!(null))
            },
        ),
        spec(
            "shape.feed",
            "Add a Manual Punch feed: jump stitches through the points, nothing sewn (moves the frame without cutting the thread)",
            Effect::Edit,
            vec![
                req("points", "[[x, y]] mm", "Points in order (2-1000)"),
                opt("name", "string", "Object name"),
            ],
            |s, p| {
                let points = p.points("points", 2)?;
                if points.len() > MAX_LINES {
                    return Err(p.invalid("points", format!("at most {MAX_LINES} points")));
                }
                if points.windows(2).all(|w| w[0] == w[1]) {
                    return Err(p.invalid("points", "needs two different points"));
                }
                let block = StitchBlock {
                    stitches: points
                        .iter()
                        .map(|q| Stitch::new(q.x, q.y, Command::Jump))
                        .collect(),
                    threads: vec![default_line()],
                };
                let mut o = DesignObject::stitches(block);
                o.name = Some(if p.has("name") {
                    p.string("name", None)?
                } else {
                    "Feed".into()
                });
                Ok(json!({ "id": s.design.add(o) }))
            },
        ),
        spec(
            "object.reverseDirection",
            "Reverse Direction Line: sew the region's sewing direction the other way (every direction line, the curved direction, or the angle turned 180°)",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let idx = indexes(p, &s.design)?;
                if let Some(&i) = idx.iter().find(|&&i| s.design.objects[i].region.is_none()) {
                    let id = s.design.objects[i].id;
                    return Err(p.failed(format!("object {id} has no region sewing")));
                }
                for i in idx {
                    let r = s.design.objects[i].region.as_mut().expect("checked");
                    for l in &mut r.direction_lines {
                        l.swap(0, 1);
                    }
                    if let Some(c) = &mut r.direction_curve {
                        *c = reversed(c);
                    }
                    r.angle = (r.angle + 180.0).rem_euclid(360.0);
                }
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
        spec(
            "object.deriveSkeleton",
            "Derive a satin skeleton from the shape's closed outline (an approximation of its medial axis): the region becomes satin sewn along it",
            Effect::Edit,
            vec![
                req("id", "int", "Shape object with region sewing"),
                opt(
                    "reshape",
                    "bool",
                    "Replace the outline with the column's (default false)",
                ),
            ],
            |s, p| {
                let reshape = p.bool("reshape", Some(false))?;
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                let o = &mut s.design.objects[i];
                if o.kind != openstitch_design::ObjectKind::Shape {
                    return Err(p.failed(format!("object {id} is not a shape")));
                }
                if o.hole_sewing {
                    return Err(p.failed("a skeleton cannot be added to a shape with hole sewing"));
                }
                if o.region.is_none() {
                    return Err(p.failed(format!("object {id} has no region sewing")));
                }
                let sk = skeleton::derive(&o.contours).ok_or_else(|| {
                    p.failed(format!(
                        "object {id}: no centre line found (needs a closed outline wider than 0.2 mm)"
                    ))
                })?;
                let outline = if reshape {
                    Some(
                        skeleton::outline(&sk)
                            .ok_or_else(|| p.failed("the derived centre line is too short"))?,
                    )
                } else {
                    None
                };
                let result = json!({
                    "points": sk.points.iter().map(|q| [q.x / 10.0, q.y / 10.0]).collect::<Vec<_>>(),
                    "widths": sk.widths,
                });
                let r = o.region.as_mut().expect("checked");
                r.kind = RegionType::Satin;
                r.skeleton = Some(sk);
                if let Some(c) = outline {
                    o.contours = vec![c];
                }
                Ok(result)
            },
        ),
    ]
}
