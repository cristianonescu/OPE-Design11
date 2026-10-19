//! Manual Punch: shapes made of several sewing-direction lines (PE-DESIGN's "Using Manual
//! Punch Tool" and "Specifying the straight sewing direction" with more than one line).

use crate::commands::{CommandSpec, Effect, default_fill, opt, req, spec, thread};
use crate::params::{CommandError, Params};
use openstitch_design::column::{self, RungError};
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
    ]
}
