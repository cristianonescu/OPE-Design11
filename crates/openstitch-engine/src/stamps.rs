//! Embossing/engraving from a shape and stamps (PE-DESIGN's Stamp/Emboss tools).

use crate::commands::{CommandSpec, Effect, MM, opt, req, spec};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_design::stamp::{self, PATTERNS, STAMP_TYPES, Stamp, StampEffect};
use openstitch_design::{ObjectKind, RegionSewing, Transform, v2};
use serde_json::json;

/// Most stamps one region may carry.
pub const MAX_STAMPS: usize = 200;

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "shape.emboss",
            "Turn a shape on top into an engraved line, engraved region or embossed region of the shape below (the tool shape is used up)",
            Effect::Edit,
            vec![
                req(
                    "id",
                    "int",
                    "Target: fill, satin or programmable-fill region",
                ),
                req("tool", "int", "Closed shape giving the effect outline"),
                req(
                    "effect",
                    "engravedLine|engravedRegion|embossedRegion",
                    "Effect",
                ),
            ],
            |s, p| {
                let effect = effect(p)?;
                let tool = p.int("tool", None, 0, i64::MAX)? as u64;
                let ti = s
                    .design
                    .index_of(tool)
                    .ok_or_else(|| p.not_found(format!("object {tool}")))?;
                let t = &s.design.objects[ti];
                if t.kind != ObjectKind::Shape
                    || t.contours.is_empty()
                    || t.contours.iter().any(|c| !c.closed)
                {
                    return Err(p.failed(format!(
                        "object {tool} must be a shape made of closed outlines"
                    )));
                }
                let contours = t.contours.clone();
                let tool_box = t.bounds();
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                if id == tool {
                    return Err(p.invalid("tool", "must be a different object"));
                }
                let target_box = {
                    let i = s
                        .design
                        .index_of(id)
                        .ok_or_else(|| p.not_found(format!("object {id}")))?;
                    s.design.objects[i].bounds()
                };
                let overlap = matches!((tool_box, target_box), (Some(a), Some(b))
                    if a.min_x < b.max_x && b.min_x < a.max_x && a.min_y < b.max_y && b.min_y < a.max_y);
                if !overlap {
                    return Err(p.failed("the shapes do not overlap"));
                }
                let r = region(s, p)?;
                r.stamps.push(Stamp {
                    effect,
                    contours,
                    pattern: None,
                });
                let index = r.stamps.len() - 1;
                s.design.objects.remove(ti);
                Ok(json!({ "id": id, "index": index }))
            },
        ),
        spec(
            "object.addStamp",
            "Place a stamp in a fill, satin or programmable-fill region",
            Effect::Edit,
            vec![
                req("id", "int", "Object with region sewing"),
                req(
                    "pattern",
                    "circle|heart|star|diamond|teardrop|crescent|cross",
                    "Stamp pattern",
                ),
                req(
                    "effect",
                    "engravedLine|engravedRegion|embossedRegion",
                    "Effect",
                ),
                req("x", "mm", "Centre x"),
                req("y", "mm", "Centre y"),
                opt("size", "mm", "Size (default 10)"),
                opt("rotation", "degrees", "Clockwise rotation (default 0)"),
            ],
            |s, p| {
                let effect = effect(p)?;
                let pattern = p.string("pattern", None)?;
                if !PATTERNS.contains(&pattern.as_str()) {
                    return Err(p.invalid("pattern", format!("must be one of {PATTERNS:?}")));
                }
                let at = v2(
                    p.req("x", -5000.0, 5000.0)? * MM,
                    p.req("y", -5000.0, 5000.0)? * MM,
                );
                let size = p.opt("size", 10.0, 0.5, 500.0)? * MM;
                let rotation = p.opt("rotation", 0.0, -3600.0, 3600.0)?;
                let st = stamp::stamp(&pattern, effect, at, size, rotation)
                    .ok_or_else(|| p.invalid("pattern", "unknown"))?;
                let r = region(s, p)?;
                if r.stamps.len() >= MAX_STAMPS {
                    return Err(p.failed(format!("at most {MAX_STAMPS} stamps per region")));
                }
                r.stamps.push(st);
                Ok(json!({ "index": r.stamps.len() - 1 }))
            },
        ),
        spec(
            "object.editStamp",
            "Move, resize, rotate a stamp or change its effect",
            Effect::Edit,
            vec![
                req("id", "int", "Object"),
                req("index", "int", "Stamp index"),
                opt("x", "mm", "New centre x (with y)"),
                opt("y", "mm", "New centre y (with x)"),
                opt("size", "mm", "New size (larger side)"),
                opt("rotation", "degrees", "Turn by this much, clockwise"),
                opt(
                    "effect",
                    "engravedLine|engravedRegion|embossedRegion",
                    "New effect",
                ),
            ],
            |s, p| {
                let new_effect = if p.has("effect") {
                    Some(effect(p)?)
                } else {
                    None
                };
                let to = if p.has("x") || p.has("y") {
                    Some(v2(
                        p.req("x", -5000.0, 5000.0)? * MM,
                        p.req("y", -5000.0, 5000.0)? * MM,
                    ))
                } else {
                    None
                };
                let size = if p.has("size") {
                    Some(p.req("size", 0.5, 500.0)? * MM)
                } else {
                    None
                };
                let rotation = p.opt("rotation", 0.0, -3600.0, 3600.0)?;
                let r = region(s, p)?;
                let n = r.stamps.len();
                if n == 0 {
                    return Err(p.failed("the region has no stamps"));
                }
                let index = p.int("index", None, 0, n as i64 - 1)? as usize;
                let st = &mut r.stamps[index];
                let b = st.bounds().ok_or_else(|| p.failed("empty stamp"))?;
                let c = b.center();
                let k = size.map_or(1.0, |sz| sz / b.width().max(b.height()).max(1e-9));
                let to = to.unwrap_or(c);
                let t = Transform::scaling(k, k, c)
                    .then(&Transform::rotation(rotation, c))
                    .then(&Transform::translation(to.x - c.x, to.y - c.y));
                *st = st.transformed(&t);
                if let Some(e) = new_effect {
                    st.effect = e;
                }
                Ok(json!(null))
            },
        ),
        spec(
            "object.deleteStamp",
            "Remove one stamp (index) or all of them (all: true)",
            Effect::Edit,
            vec![
                req("id", "int", "Object"),
                opt("index", "int", "Stamp index"),
                opt("all", "bool", "Remove every stamp"),
            ],
            |s, p| {
                let all = p.bool("all", Some(false))?;
                let r = region(s, p)?;
                if all {
                    r.stamps.clear();
                    return Ok(json!(null));
                }
                let n = r.stamps.len();
                if n == 0 {
                    return Err(p.failed("the region has no stamps"));
                }
                let index = p.int("index", None, 0, n as i64 - 1)? as usize;
                r.stamps.remove(index);
                Ok(json!(null))
            },
        ),
    ]
}

fn effect(p: &Params) -> Result<StampEffect, CommandError> {
    StampEffect::from_id(&p.string("effect", None)?).ok_or_else(|| {
        p.invalid(
            "effect",
            "must be engravedLine, engravedRegion or embossedRegion",
        )
    })
}

/// The stamp-capable region of object `id`.
fn region<'a>(s: &'a mut EngineState, p: &Params) -> Result<&'a mut RegionSewing, CommandError> {
    let id = p.int("id", None, 0, i64::MAX)? as u64;
    let i = s
        .design
        .index_of(id)
        .ok_or_else(|| p.not_found(format!("object {id}")))?;
    let r = s.design.objects[i]
        .region
        .as_mut()
        .ok_or_else(|| p.failed(format!("object {id} has no region sewing")))?;
    if !STAMP_TYPES.contains(&r.kind) {
        return Err(p.failed(format!(
            "object {id}: stamps work on fill, satin and programmable fill only"
        )));
    }
    Ok(r)
}
