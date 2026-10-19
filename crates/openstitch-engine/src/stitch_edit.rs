//! Stitch pattern editing commands (PE-DESIGN guide, "Editing a Stitch Pattern"): needle
//! points are addressed by their index in the pattern's stitch list (`stitches.points` lists
//! them). Logic in `openstitch_design::stitch_edit`.

use crate::commands::{CommandSpec, Effect, IDS, MM, indexes, opt, req, spec};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_core::Command;
use openstitch_design::geometry::contains;
use openstitch_design::stitch_edit::{self, EditError};
use openstitch_design::{Contour, DesignObject, ObjectKind, StitchBlock, v2};
use serde_json::{Value, json};

const ID: crate::commands::ParamDoc = req("id", "int", "Stitch pattern object");
/// Most needle points one command may list.
const MAX_INDEXES: usize = 100_000;

/// Index of stitch pattern `id` in the design.
fn target(s: &EngineState, p: &Params, key: &str) -> Result<usize, CommandError> {
    let id = p.int(key, None, 0, i64::MAX)? as u64;
    let i = s
        .design
        .index_of(id)
        .ok_or_else(|| p.not_found(format!("object {id}")))?;
    match &s.design.objects[i] {
        o if o.kind == ObjectKind::Stitches && o.stitch_block.is_some() => Ok(i),
        _ => Err(p.invalid(key, format!("object {id} is not a stitch pattern"))),
    }
}

fn block(s: &mut EngineState, i: usize) -> &mut StitchBlock {
    s.design.objects[i]
        .stitch_block
        .as_mut()
        .expect("checked by target")
}

fn index_list(p: &Params) -> Result<Vec<usize>, CommandError> {
    let a = p.array("indexes")?;
    if a.is_empty() || a.len() > MAX_INDEXES {
        return Err(p.invalid(
            "indexes",
            format!("needs 1...{MAX_INDEXES} needle point indexes"),
        ));
    }
    a.iter()
        .map(|v| {
            v.as_u64()
                .map(|n| n as usize)
                .ok_or_else(|| p.invalid("indexes", "must contain non-negative integers"))
        })
        .collect()
}

fn edit_error(p: &Params, e: EditError) -> CommandError {
    p.failed(e.0)
}

/// A new stitch pattern after `like` in sewing order (same name stem, visibility and group).
fn insert_after(
    s: &mut EngineState,
    at: usize,
    like: &DesignObject,
    b: StitchBlock,
    suffix: &str,
) -> u64 {
    let mut o = DesignObject::stitches(b);
    o.name = Some(format!(
        "{} {suffix}",
        like.name
            .clone()
            .unwrap_or_else(|| format!("Pattern {}", like.id))
    ));
    o.visible = like.visible;
    o.group = like.group;
    let id = s.design.add(o);
    let o = s.design.objects.pop().expect("just added");
    s.design.objects.insert(at.min(s.design.objects.len()), o);
    id
}

fn command_name(c: Command) -> &'static str {
    match c {
        Command::Stitch => "stitch",
        Command::Jump => "jump",
        Command::Trim => "trim",
        Command::ColorChange => "colorChange",
        Command::Stop => "stop",
        Command::End => "end",
    }
}

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "stitches.setThread",
            "Change Color: give one colour block of a stitch pattern another thread",
            Effect::Edit,
            vec![
                ID,
                req(
                    "index",
                    "int",
                    "Colour block (0-based, as stitches.points lists threads)",
                ),
                req(
                    "color",
                    "color",
                    "#rrggbb, a Brother thread name or chart number 1-64",
                ),
            ],
            |s, p| {
                let i = target(s, p, "id")?;
                let n = s.design.objects[i]
                    .stitch_block
                    .as_ref()
                    .map_or(0, |b| b.threads.len());
                if n == 0 {
                    return Err(p.failed("the stitch pattern has no threads"));
                }
                let k = p.int("index", None, 0, n as i64 - 1)? as usize;
                let t = crate::commands::thread(p, "color")?
                    .ok_or_else(|| p.invalid("color", "must be a thread colour"))?;
                block(s, i).threads[k] = t;
                Ok(json!(null))
            },
        ),
        spec(
            "stitches.points",
            "List a stitch pattern's entries: index, position (mm), command and colour block",
            Effect::Read,
            vec![ID],
            |s, p| {
                let i = target(s, p, "id")?;
                let b = s.design.objects[i].stitch_block.as_ref().expect("checked");
                let colors = stitch_edit::colors(b);
                let r = |v: f64| (v / MM * 100.0).round() / 100.0;
                Ok(json!({
                    "threads": b.threads.iter().map(|t| t.hex()).collect::<Vec<_>>(),
                    "points": b.stitches.iter().enumerate().map(|(k, st)| json!({
                        "index": k, "x": r(st.x), "y": r(st.y),
                        "command": command_name(st.command), "color": colors[k],
                    })).collect::<Vec<Value>>(),
                }))
            },
        ),
        spec(
            "stitches.movePoints",
            "Move needle points of a stitch pattern",
            Effect::Edit,
            vec![
                ID,
                req(
                    "indexes",
                    "[int]",
                    "Needle point indexes (stitch or jump entries)",
                ),
                opt("dx", "mm", "Horizontal move"),
                opt("dy", "mm", "Vertical move"),
            ],
            |s, p| {
                let i = target(s, p, "id")?;
                let idx = index_list(p)?;
                let d = v2(
                    p.opt("dx", 0.0, -2000.0, 2000.0)? * MM,
                    p.opt("dy", 0.0, -2000.0, 2000.0)? * MM,
                );
                stitch_edit::move_points(block(s, i), &idx, d).map_err(|e| edit_error(p, e))?;
                Ok(Value::Null)
            },
        ),
        spec(
            "stitches.setPoint",
            "Place one needle point",
            Effect::Edit,
            vec![
                ID,
                req("index", "int", "Needle point index"),
                req("x", "mm", "X"),
                req("y", "mm", "Y"),
            ],
            |s, p| {
                let i = target(s, p, "id")?;
                let k = p.int("index", None, 0, i64::MAX)? as usize;
                let at = v2(
                    p.req("x", -5000.0, 5000.0)? * MM,
                    p.req("y", -5000.0, 5000.0)? * MM,
                );
                stitch_edit::set_point(block(s, i), k, at).map_err(|e| edit_error(p, e))?;
                Ok(Value::Null)
            },
        ),
        spec(
            "stitches.insertPoint",
            "Insert a needle point on the line into point `index` (midpoint unless x/y given); jump makes the line up to it a jump",
            Effect::Edit,
            vec![
                ID,
                req("index", "int", "The needle point the line runs into"),
                opt("x", "mm", "X (with y)"),
                opt("y", "mm", "Y (with x)"),
                opt(
                    "jump",
                    "bool",
                    "Make the line up to the new point a jump (default false)",
                ),
            ],
            |s, p| {
                let i = target(s, p, "id")?;
                let k = p.int("index", None, 0, i64::MAX)? as usize;
                let at = match (p.has("x"), p.has("y")) {
                    (true, true) => Some(v2(
                        p.req("x", -5000.0, 5000.0)? * MM,
                        p.req("y", -5000.0, 5000.0)? * MM,
                    )),
                    (false, false) => None,
                    _ => return Err(p.invalid("x", "x and y go together")),
                };
                let jump = p.bool("jump", Some(false))?;
                let new = stitch_edit::insert_point(block(s, i), k, at, jump)
                    .map_err(|e| edit_error(p, e))?;
                Ok(json!({ "index": new }))
            },
        ),
        spec(
            "stitches.deletePoints",
            "Delete needle points; deleting the end of a jump joins the stitches on either side",
            Effect::Edit,
            vec![ID, req("indexes", "[int]", "Needle point indexes")],
            |s, p| {
                let i = target(s, p, "id")?;
                let idx = index_list(p)?;
                stitch_edit::delete_points(block(s, i), &idx).map_err(|e| edit_error(p, e))?;
                Ok(Value::Null)
            },
        ),
        spec(
            "stitches.splitAt",
            "Split a stitch pattern in two at a needle point (not its entry or exit point)",
            Effect::Edit,
            vec![ID, req("index", "int", "Needle point index")],
            |s, p| {
                let i = target(s, p, "id")?;
                let k = p.int("index", None, 0, i64::MAX)? as usize;
                let o = s.design.objects[i].clone();
                let (a, c) = stitch_edit::split_at(o.stitch_block.as_ref().expect("checked"), k)
                    .map_err(|e| edit_error(p, e))?;
                s.design.objects[i].stitch_block = Some(a);
                let new = insert_after(s, i + 1, &o, c, "part 2");
                Ok(json!({ "ids": [o.id, new] }))
            },
        ),
        spec(
            "stitches.connect",
            "Join two stitch patterns: the one sewn later continues from the earlier one's exit point and takes its colour",
            Effect::Edit,
            vec![req("ids", "[int]", "Exactly two stitch patterns")],
            |s, p| {
                let ids = p.ids()?;
                if ids.len() != 2 || ids[0] == ids[1] {
                    return Err(p.invalid("ids", "needs exactly two different stitch patterns"));
                }
                let mut at = Vec::new();
                for &id in &ids {
                    let i = s
                        .design
                        .index_of(id)
                        .ok_or_else(|| p.not_found(format!("object {id}")))?;
                    if s.design.objects[i].kind != ObjectKind::Stitches
                        || s.design.objects[i].stitch_block.is_none()
                    {
                        return Err(
                            p.invalid("ids", format!("object {id} is not a stitch pattern"))
                        );
                    }
                    at.push(i);
                }
                at.sort_unstable();
                let (first, second) = (at[0], at[1]);
                let later = s.design.objects[second]
                    .stitch_block
                    .clone()
                    .expect("checked");
                let joined = stitch_edit::connect(
                    s.design.objects[first]
                        .stitch_block
                        .as_ref()
                        .expect("checked"),
                    &later,
                );
                let id = s.design.objects[first].id;
                s.design.objects[first].stitch_block = Some(joined);
                s.design.objects.remove(second);
                s.design.dissolve_single_groups();
                Ok(json!({ "id": id }))
            },
        ),
        spec(
            "stitches.divideByColor",
            "Divide stitch patterns into one pattern per colour, in place",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let idx = indexes(p, &s.design)?;
                let ids: Vec<u64> = idx.iter().map(|&i| s.design.objects[i].id).collect();
                let mut created = Vec::new();
                let mut any = false;
                for id in ids {
                    let i = s.design.index_of(id).expect("known");
                    let o = s.design.objects[i].clone();
                    let Some(b) = o
                        .stitch_block
                        .as_ref()
                        .filter(|_| o.kind == ObjectKind::Stitches)
                    else {
                        continue;
                    };
                    any = true;
                    let mut parts = stitch_edit::divide_by_color(b).into_iter();
                    let Some(first) = parts.next() else { continue };
                    s.design.objects[i].stitch_block = Some(first);
                    created.push(id);
                    for (n, part) in parts.enumerate() {
                        created.push(insert_after(
                            s,
                            i + 1 + n,
                            &o,
                            part,
                            &format!("colour {}", n + 2),
                        ));
                    }
                }
                if !any {
                    return Err(p.failed("no stitch patterns to divide"));
                }
                Ok(json!({ "ids": created }))
            },
        ),
        spec(
            "stitches.splitArea",
            "Split off the needle points inside an outline into a new stitch pattern, sewn last",
            Effect::Edit,
            vec![
                ID,
                req("points", "[[x, y]] mm", "Closed outline, at least 3 points"),
            ],
            |s, p| {
                let i = target(s, p, "id")?;
                let outline = [Contour::new(p.points("points", 3)?, true)];
                let o = s.design.objects[i].clone();
                let (inside, rest) =
                    stitch_edit::split_area(o.stitch_block.as_ref().expect("checked"), |q| {
                        contains(q, &outline)
                    })
                    .map_err(|e| edit_error(p, e))?;
                s.design.objects[i].stitch_block = Some(rest);
                let end = s.design.objects.len();
                let new = insert_after(s, end, &o, inside, "split");
                Ok(json!({ "ids": [new] }))
            },
        ),
    ]
}
