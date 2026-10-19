//! Drawing and editing commands beyond single shapes: grouping, arranged copies, pasting
//! sewing attributes and pasting cut objects. Behaviour follows the PE-DESIGN 11 guide pages
//! "Grouping/Ungrouping embroidery patterns", "Arrange Copies" and "Copying sewing
//! attributes".

use crate::commands::{CommandSpec, Effect, IDS, MM, indexes, opt, req, spec};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_design::{Design, DesignObject, ObjectKind, Rect, Transform, v2};
use serde_json::json;
use std::collections::HashMap;

/// Most objects one arrange or duplicate command may add.
pub const MAX_COPIES: usize = 10_000;

pub fn commands() -> Vec<CommandSpec> {
    let mut v = group_commands();
    v.push(arrange_command());
    v.push(paste_attributes_command());
    v.push(paste_command());
    v.push(entry_exit_command());
    v.push(spec(
        "object.setJumpTrimming",
        "Jump-stitch trimming per object on multi-needle machines (off: its jumps are never trimmed; colour changes still cut)",
        Effect::Edit,
        vec![IDS, req("enabled", "bool", "Trim jumps")],
        |s, p| {
            let on = p.bool("enabled", None)?;
            for i in indexes(p, &s.design)? {
                s.design.objects[i].trim_jumps = (!on).then_some(false);
            }
            Ok(json!(null))
        },
    ));
    v.extend(crate::shape_ops::commands());
    v
}

/// Copies the objects at `idx` with `t` applied, appended in sewing order. Each copied group
/// becomes a new group, and text on a path copied along with its path follows the copy.
pub fn copy_set(design: &mut Design, idx: &[usize], t: &Transform) -> Vec<u64> {
    let originals: Vec<DesignObject> = idx.iter().map(|&i| design.objects[i].clone()).collect();
    add_set(design, originals, t)
}

/// Adds `objects` (with `t` applied) under new ids, appended in sewing order: groups become
/// new groups, text on a path added along with its path follows it, and a path left behind
/// is dropped when it is no longer in the design.
fn add_set(design: &mut Design, objects: Vec<DesignObject>, t: &Transform) -> Vec<u64> {
    let mut groups: HashMap<u64, u64> = HashMap::new();
    let mut ids: HashMap<u64, u64> = HashMap::new();
    let mut out = Vec::new();
    for mut copy in objects {
        copy.apply(t);
        if let Some(g) = copy.group {
            copy.group = Some(*groups.entry(g).or_insert_with(|| design.new_group_id()));
        }
        let old = copy.id;
        let new = design.add(copy);
        ids.insert(old, new);
        out.push(new);
    }
    // A hole-sewing pair copied together stays a pair; an outer shape copied alone loses it.
    for &id in &out {
        let i = design.index_of(id).expect("just added");
        let o = &mut design.objects[i];
        if let Some(h) = o.hole_inner {
            o.hole_inner = ids.get(&h).copied();
            o.hole_sewing = o.hole_inner.is_some();
        }
    }
    for &id in &out {
        let i = design.index_of(id).expect("just added");
        let path = design.objects[i].text.as_ref().and_then(|spec| spec.path);
        if let Some(p) = path {
            let to = ids
                .get(&p)
                .copied()
                .or_else(|| design.index_of(p).map(|_| p));
            if let Some(spec) = &mut design.objects[i].text {
                spec.path = to;
            }
        }
    }
    out
}

/// Most objects one paste may add.
pub const MAX_PASTE: usize = 10_000;

/// Edit ▸ Cut and Paste: objects as `object.get` returns them, added back under new ids.
fn paste_command() -> CommandSpec {
    spec(
        "object.paste",
        "Add objects given as object.get returns them (Cut and Paste): new ids, groups kept together, appended in sewing order",
        Effect::Edit,
        vec![
            req("objects", "[object]", "Object data (1-10000)"),
            opt("dx", "mm", "Offset (default 0)"),
            opt("dy", "mm", "Offset (default 0)"),
        ],
        |s, p| {
            let arr = p.array("objects")?;
            if arr.is_empty() || arr.len() > MAX_PASTE {
                return Err(p.invalid("objects", format!("needs 1...{MAX_PASTE} objects")));
            }
            let objects = arr
                .iter()
                .enumerate()
                .map(|(k, v)| {
                    let o: DesignObject = serde_json::from_value(v.clone())
                        .map_err(|e| p.invalid("objects", format!("object {k}: {e}")))?;
                    let ok = match o.kind {
                        ObjectKind::Stitches => o.stitch_block.is_some(),
                        ObjectKind::Text => o.text.is_some(),
                        ObjectKind::Shape => !o.contours.is_empty(),
                    };
                    let finite = o
                        .contours
                        .iter()
                        .flat_map(|c| &c.points)
                        .all(|q| q.x.is_finite() && q.y.is_finite());
                    if !ok || !finite {
                        return Err(p.invalid("objects", format!("object {k} is incomplete")));
                    }
                    Ok(o)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let t = Transform::translation(
                p.opt("dx", 0.0, -5000.0, 5000.0)? * MM,
                p.opt("dy", 0.0, -5000.0, 5000.0)? * MM,
            );
            Ok(json!({ "ids": add_set(&mut s.design, objects, &t) }))
        },
    )
}

// MARK: groups

fn group_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "object.group",
            "Group objects (2 or more; groups they already belong to are merged in)",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let ids = expanded(s, p)?;
                if ids.len() < 2 {
                    return Err(p.failed("grouping needs at least 2 objects"));
                }
                let d = &mut s.design;
                let first = d.objects[d.index_of(ids[0]).expect("known")].group;
                let same = first.is_some()
                    && ids
                        .iter()
                        .all(|&id| d.objects[d.index_of(id).expect("known")].group == first);
                let g = match first {
                    Some(g) if same => g,
                    _ => d.new_group_id(),
                };
                for id in &ids {
                    let i = d.index_of(*id).expect("known");
                    d.objects[i].group = Some(g);
                }
                Ok(json!({ "group": g, "ids": ids }))
            },
        ),
        spec(
            "object.ungroup",
            "Dissolve the groups the objects belong to",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let ids = expanded(s, p)?;
                let d = &mut s.design;
                let mut any = false;
                for id in &ids {
                    let i = d.index_of(*id).expect("known");
                    any |= d.objects[i].group.take().is_some();
                }
                if !any {
                    return Err(p.failed("none of these objects is grouped"));
                }
                Ok(json!({ "ids": ids }))
            },
        ),
    ]
}

/// The objects named by `id` / `ids` plus the other members of their groups.
fn expanded(s: &EngineState, p: &Params) -> Result<Vec<u64>, CommandError> {
    let idx = indexes(p, &s.design)?;
    let ids: Vec<u64> = idx.iter().map(|&i| s.design.objects[i].id).collect();
    Ok(s.design.with_groups(&ids))
}

// MARK: arrange copies

fn arrange_command() -> CommandSpec {
    spec(
        "object.arrangeCopies",
        "Arrange copies: mirror (horizontal, vertical, four-way), circle or matrix",
        Effect::Edit,
        vec![
            IDS,
            req(
                "mode",
                "mirrorHorizontal|mirrorVertical|mirrorFourWay|circle|matrix",
                "Arrangement",
            ),
            opt(
                "spacing",
                "mm",
                "Mirror: gap between original and copy (default 0, negative overlaps)",
            ),
            opt(
                "count",
                "int 2-360",
                "Circle: patterns on the circle, original included",
            ),
            opt(
                "radius",
                "mm",
                "Circle: distance from the circle centre to the selection centre",
            ),
            opt(
                "cx",
                "mm",
                "Circle centre x (default: radius below the selection centre)",
            ),
            opt("cy", "mm", "Circle centre y"),
            opt(
                "rotate",
                "bool",
                "Circle: turn each copy to face the centre like the original (default true)",
            ),
            opt("rows", "int 1-100", "Matrix rows (default 1)"),
            opt("columns", "int 1-100", "Matrix columns (default 1)"),
            opt(
                "spacingX",
                "mm",
                "Matrix: horizontal gap (default 0, negative overlaps)",
            ),
            opt(
                "spacingY",
                "mm",
                "Matrix: vertical gap (default 0, negative overlaps)",
            ),
            opt(
                "mergeOverlapping",
                "bool",
                "Merge closed shapes that overlap after copying (default false; at most 200 objects)",
            ),
            opt(
                "optimizeOrder",
                "bool",
                "Sew same-colour patterns together to save colour changes (default false)",
            ),
        ],
        |s, p| {
            let mut idx = indexes(p, &s.design)?;
            idx.sort_unstable();
            idx.dedup();
            let Some(b) = Rect::enclosing(idx.iter().flat_map(|&i| s.design.objects[i].points()))
            else {
                return Err(p.failed("the objects have no geometry"));
            };
            let transforms = arrangement(p, b)?;
            if transforms.len() * idx.len() > MAX_COPIES {
                return Err(p.failed(format!("would add more than {MAX_COPIES} objects")));
            }
            let mut copies = Vec::new();
            for t in &transforms {
                copies.extend(copy_set(&mut s.design, &idx, t));
            }
            let mut all: Vec<u64> = idx.iter().map(|&i| s.design.objects[i].id).collect();
            all.extend(&copies);
            if p.bool("mergeOverlapping", Some(false))? {
                if all.len() > 200 {
                    return Err(p.invalid("mergeOverlapping", "works on at most 200 objects"));
                }
                let gone = crate::shape_ops::merge_overlapping(&mut s.design, &all);
                all.retain(|id| !gone.contains(id));
                copies.retain(|id| !gone.contains(id));
            }
            if p.bool("optimizeOrder", Some(false))? {
                group_by_colour(&mut s.design, &all);
            }
            Ok(json!({ "ids": copies, "objects": all }))
        },
    )
}

/// One transform per copy (the original is not included).
fn arrangement(p: &Params, b: Rect) -> Result<Vec<Transform>, CommandError> {
    let mode = p.string("mode", None)?;
    let gap = || p.opt("spacing", 0.0, -1000.0, 1000.0).map(|g| g * MM);
    Ok(match mode.as_str() {
        "mirrorHorizontal" => {
            let x = b.max_x + gap()? / 2.0;
            vec![Transform::scaling(-1.0, 1.0, v2(x, 0.0))]
        }
        "mirrorVertical" => {
            let y = b.max_y + gap()? / 2.0;
            vec![Transform::scaling(1.0, -1.0, v2(0.0, y))]
        }
        "mirrorFourWay" => {
            let g = gap()?;
            let axis = v2(b.max_x + g / 2.0, b.max_y + g / 2.0);
            vec![
                Transform::scaling(-1.0, 1.0, axis),
                Transform::scaling(1.0, -1.0, axis),
                Transform::scaling(-1.0, -1.0, axis),
            ]
        }
        "circle" => {
            let n = p.int("count", None, 2, 360)? as usize;
            let radius = p.req("radius", 0.0, 2000.0)? * MM;
            let center = if p.has("cx") || p.has("cy") {
                v2(
                    p.req("cx", -5000.0, 5000.0)? * MM,
                    p.req("cy", -5000.0, 5000.0)? * MM,
                )
            } else {
                b.center() + v2(0.0, radius)
            };
            let rotate = p.bool("rotate", Some(true))?;
            (1..n)
                .map(|k| {
                    let r = Transform::rotation(360.0 * k as f64 / n as f64, center);
                    if rotate {
                        r
                    } else {
                        let to = r.apply(b.center()) - b.center();
                        Transform::translation(to.x, to.y)
                    }
                })
                .collect()
        }
        "matrix" => {
            let rows = p.int("rows", Some(1), 1, 100)? as usize;
            let cols = p.int("columns", Some(1), 1, 100)? as usize;
            if rows * cols < 2 {
                return Err(p.invalid("rows", "rows × columns must be at least 2"));
            }
            let step_x = b.width() + p.opt("spacingX", 0.0, -1000.0, 1000.0)? * MM;
            let step_y = b.height() + p.opt("spacingY", 0.0, -1000.0, 1000.0)? * MM;
            if (cols > 1 && step_x <= 0.0) || (rows > 1 && step_y <= 0.0) {
                return Err(p.invalid("spacingX", "would stack copies on top of each other"));
            }
            (0..rows)
                .flat_map(|r| (0..cols).map(move |c| (r, c)))
                .skip(1)
                .map(|(r, c)| Transform::translation(c as f64 * step_x, r as f64 * step_y))
                .collect()
        }
        _ => {
            return Err(p.invalid(
                "mode",
                "must be mirrorHorizontal, mirrorVertical, mirrorFourWay, circle or matrix",
            ));
        }
    })
}

/// The thread an object starts with, for colour grouping.
pub(crate) fn first_colour(o: &DesignObject) -> Option<String> {
    o.region
        .as_ref()
        .map(|r| r.thread.hex())
        .or_else(|| o.line.as_ref().map(|l| l.thread.hex()))
        .or_else(|| {
            o.stitch_block
                .as_ref()
                .and_then(|b| b.threads.first())
                .map(|t| t.hex())
        })
}

/// Reorders the objects `ids` among their own sewing-order slots so equal first colours sew
/// together, colours in order of first appearance (stable within a colour).
pub(crate) fn group_by_colour(d: &mut Design, ids: &[u64]) {
    let mut slots: Vec<usize> = ids.iter().filter_map(|&id| d.index_of(id)).collect();
    slots.sort_unstable();
    let mut order: Vec<Option<String>> = Vec::new();
    for &i in &slots {
        let c = first_colour(&d.objects[i]);
        if !order.contains(&c) {
            order.push(c);
        }
    }
    let mut objects: Vec<DesignObject> = slots.iter().map(|&i| d.objects[i].clone()).collect();
    objects.sort_by_key(|o| {
        let c = first_colour(o);
        order.iter().position(|x| *x == c)
    });
    for (slot, o) in slots.into_iter().zip(objects) {
        d.objects[slot] = o;
    }
}

// MARK: paste attributes

const ATTRIBUTES: [&str; 5] = [
    "regionColor",
    "lineColor",
    "regionSewing",
    "lineSewing",
    "holeSewing",
];

fn paste_attributes_command() -> CommandSpec {
    spec(
        "object.pasteAttributes",
        "Copy thread colours, sew types and sewing attributes from one object to others",
        Effect::Edit,
        vec![
            req("from", "int", "Source object"),
            IDS,
            opt(
                "attributes",
                "[regionColor|lineColor|regionSewing|lineSewing|holeSewing]",
                "Only these (default all); between a shape and a text only colours are copied",
            ),
        ],
        |s, p| {
            let from = p.int("from", None, 0, i64::MAX)? as u64;
            let src = s
                .design
                .index_of(from)
                .map(|i| s.design.objects[i].clone())
                .ok_or_else(|| p.not_found(format!("object {from}")))?;
            if !src.is_outline() {
                return Err(p.failed(format!("object {from} is a stitch pattern")));
            }
            let wanted: Vec<String> = if p.has("attributes") {
                let list = p.array("attributes")?;
                if list.is_empty() {
                    return Err(p.invalid("attributes", "must not be empty"));
                }
                list.iter()
                    .map(|v| {
                        v.as_str()
                            .filter(|a| ATTRIBUTES.contains(a))
                            .map(str::to_string)
                            .ok_or_else(|| {
                                p.invalid("attributes", format!("must be among {ATTRIBUTES:?}"))
                            })
                    })
                    .collect::<Result<_, _>>()?
            } else {
                ATTRIBUTES.iter().map(|a| a.to_string()).collect()
            };
            let has = |a: &str| wanted.iter().any(|w| w == a);
            let mut changed = Vec::new();
            for i in indexes(p, &s.design)? {
                let dst = &mut s.design.objects[i];
                if dst.id == from {
                    continue;
                }
                if !dst.is_outline() {
                    return Err(p.failed(format!("object {} is a stitch pattern", dst.id)));
                }
                let colours_only = src.kind != dst.kind;
                if !colours_only && has("regionSewing") && dst.has_closed_contour() {
                    let old = dst.region.clone();
                    dst.region = src.region.clone().map(|mut r| {
                        // Thread and the points placed on this object's own outline stay.
                        if let Some(o) = &old {
                            r.thread = o.thread.clone();
                        }
                        let keep = old.as_ref();
                        r.entry = keep.and_then(|o| o.entry);
                        r.exit = keep.and_then(|o| o.exit);
                        r.center = keep.and_then(|o| o.center);
                        r.reference = keep.and_then(|o| o.reference);
                        r.direction_curve = keep.and_then(|o| o.direction_curve.clone());
                        r
                    });
                }
                if !colours_only && has("lineSewing") {
                    let old = dst.line.clone();
                    dst.line = src.line.clone().map(|mut l| {
                        if let Some(o) = &old {
                            l.thread = o.thread.clone();
                        }
                        l.entry = old.as_ref().and_then(|o| o.entry);
                        l
                    });
                }
                if has("regionColor")
                    && let (Some(d), Some(sr)) = (&mut dst.region, &src.region)
                {
                    d.thread = sr.thread.clone();
                }
                if has("lineColor")
                    && let (Some(d), Some(sl)) = (&mut dst.line, &src.line)
                {
                    d.thread = sl.thread.clone();
                }
                if !colours_only && has("holeSewing") {
                    // The copy pairs with the region it encloses, found when it is sewn.
                    dst.hole_sewing = src.hole_sewing && dst.region.is_some();
                    dst.hole_inner = None;
                }
                if dst.region.is_none() {
                    dst.hole_sewing = false;
                    dst.hole_inner = None;
                }
                changed.push(dst.id);
            }
            Ok(json!({ "ids": changed }))
        },
    )
}

// MARK: entry and exit points

fn entry_exit_command() -> CommandSpec {
    spec(
        "object.setEntryExit",
        "Set where region and line sewing start and end (snapped to the outline; null clears)",
        Effect::Edit,
        vec![
            req("id", "int", "Shape object"),
            opt("regionEntry", "[x, y] mm | null", "Region entry point (R1)"),
            opt("regionExit", "[x, y] mm | null", "Region exit point (R2)"),
            opt(
                "lineEntry",
                "[x, y] mm | null",
                "Line entry point (L1): closed outlines start there, open ones from the nearer end",
            ),
        ],
        |s, p| {
            let id = p.int("id", None, 0, i64::MAX)? as u64;
            let i = s
                .design
                .index_of(id)
                .ok_or_else(|| p.not_found(format!("object {id}")))?;
            let o = &mut s.design.objects[i];
            if o.kind != openstitch_design::ObjectKind::Shape {
                return Err(p.failed(format!(
                    "object {id}: entry and exit points can be set on shapes only"
                )));
            }
            let keys = ["regionEntry", "regionExit", "lineEntry"];
            if !keys.iter().any(|k| p.has(k) || p.is_null(k)) {
                return Err(p.invalid("regionEntry", "give regionEntry, regionExit or lineEntry"));
            }
            // Some(None) clears, Some(Some(p)) sets, None leaves alone.
            let read =
                |key: &str| -> Result<Option<Option<openstitch_design::Vec2>>, CommandError> {
                    if p.is_null(key) {
                        Ok(Some(None))
                    } else if p.has(key) {
                        Ok(Some(Some(p.point(key)?)))
                    } else {
                        Ok(None)
                    }
                };
            let (re, rx, le) = (
                read("regionEntry")?,
                read("regionExit")?,
                read("lineEntry")?,
            );
            let closed: Vec<_> = o.contours.iter().filter(|c| c.closed).cloned().collect();
            // Clearing a point the object cannot have is fine; setting one is not.
            if (matches!(re, Some(Some(_))) || matches!(rx, Some(Some(_)))) && o.region.is_none() {
                return Err(p.failed(format!("object {id} has no region sewing")));
            }
            if matches!(le, Some(Some(_))) && o.line.is_none() {
                return Err(p.failed(format!("object {id} has no line sewing")));
            }
            if let Some(r) = &mut o.region {
                if let Some(v) = re {
                    r.entry = v.map(|q| openstitch_design::route::snap(&closed, q));
                }
                if let Some(v) = rx {
                    r.exit = v.map(|q| openstitch_design::route::snap(&closed, q));
                }
            }
            if let (Some(v), Some(l)) = (le, &mut o.line) {
                l.entry = v.map(|q| {
                    o.contours
                        .iter()
                        .filter_map(|c| c.nearest(q))
                        .min_by(|a, b| a.point.distance(q).total_cmp(&b.point.distance(q)))
                        .map_or(q, |n| n.point)
                });
            }
            let mm = |v: Option<openstitch_design::Vec2>| v.map(|q| [q.x / MM, q.y / MM]);
            Ok(json!({
                "regionEntry": mm(o.region.as_ref().and_then(|r| r.entry)),
                "regionExit": mm(o.region.as_ref().and_then(|r| r.exit)),
                "lineEntry": mm(o.line.as_ref().and_then(|l| l.entry)),
            }))
        },
    )
}
