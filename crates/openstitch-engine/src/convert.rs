//! Conversions between PE-DESIGN's three data types (guide: "Comparison of Types of Data",
//! "Converting shape pattern or text pattern to a stitch pattern", "Converting stitch
//! patterns", "Converting region stitching to line stitching"):
//!
//! - `object.toStitches`: shape or text → stitch pattern (the needle points it sews now).
//! - `object.toShapes`: stitch pattern → shapes traced from its stitch coverage, as one outline,
//!   one per colour, or blocks with their own sewing direction (approximation of "Convert to
//!   Outline" / "Convert to Blocks").
//! - `object.regionToLine`: region sewing → running line sewing in the region's thread.
//!
//! Text → shapes is `text.toShapes` (lettering).

#[cfg(feature = "digitize")]
use crate::commands::opt;
use crate::commands::{CommandSpec, Effect, IDS, indexes, spec};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_design::{Compiler, Design, LineSewing, LineType, ObjectKind, StitchBlock};
use serde_json::json;

pub fn commands() -> Vec<CommandSpec> {
    let mut v = vec![to_stitches(), region_to_line()];
    #[cfg(feature = "digitize")]
    v.push(to_shapes());
    v
}

/// The selected objects plus their group members, and text on a selected path / the path of a
/// selected text (they convert together, as in PE-DESIGN).
fn expanded(p: &Params, d: &Design) -> Result<Vec<u64>, CommandError> {
    let idx = indexes(p, d)?;
    let mut ids: Vec<u64> = idx.iter().map(|&i| d.objects[i].id).collect();
    for o in &d.objects {
        let path = o.text.as_ref().and_then(|t| t.path);
        if let Some(path) = path {
            if ids.contains(&o.id) && !ids.contains(&path) {
                ids.push(path);
            }
            if ids.contains(&path) && !ids.contains(&o.id) {
                ids.push(o.id);
            }
        }
    }
    Ok(d.with_groups(&ids))
}

fn to_stitches() -> CommandSpec {
    spec(
        "object.toStitches",
        "Convert shapes and text into stitch patterns (needle points); groups and text on a path convert together, objects that sew nothing are deleted",
        Effect::Edit,
        vec![IDS],
        |s: &mut EngineState, p: &Params| {
            let ids = expanded(p, &s.design)?;
            let compiler = Compiler::for_design(&s.design);
            // Compile every object against the design as it is, before any is replaced.
            let converted: Vec<(u64, Option<StitchBlock>)> = ids
                .iter()
                .filter_map(|&id| {
                    let i = s.design.index_of(id)?;
                    let o = &s.design.objects[i];
                    o.is_outline().then(|| {
                        let pattern = compiler.compile_object(&s.design, i);
                        let block = StitchBlock::from(&pattern);
                        let sews = block
                            .stitches
                            .iter()
                            .any(|st| st.command == openstitch_core::Command::Stitch);
                        (id, sews.then_some(block))
                    })
                })
                .collect();
            if converted.is_empty() {
                return Err(p.failed("no shape or text objects to convert"));
            }
            let (mut done, mut deleted) = (Vec::new(), Vec::new());
            for (id, block) in converted {
                let Some(i) = s.design.index_of(id) else {
                    continue;
                };
                match block {
                    Some(block) => {
                        let o = &mut s.design.objects[i];
                        o.kind = ObjectKind::Stitches;
                        o.stitch_block = Some(block);
                        o.contours.clear();
                        o.region = None;
                        o.line = None;
                        o.text = None;
                        o.hole_sewing = false;
                        done.push(id);
                    }
                    None => {
                        s.design.objects.remove(i);
                        deleted.push(id);
                    }
                }
            }
            s.design.dissolve_single_groups();
            Ok(json!({ "ids": done, "deleted": deleted }))
        },
    )
}

fn region_to_line() -> CommandSpec {
    spec(
        "object.regionToLine",
        "Convert region sewing into line sewing: the outline is sewn with a running stitch in the region's thread",
        Effect::Edit,
        vec![IDS],
        |s: &mut EngineState, p: &Params| {
            let idx = indexes(p, &s.design)?;
            let mut changed = Vec::new();
            for i in idx {
                let o = &mut s.design.objects[i];
                if !o.is_outline() {
                    continue;
                }
                if let Some(region) = o.region.take() {
                    o.line = Some(LineSewing::new(LineType::Running, region.thread));
                    changed.push(o.id);
                }
            }
            if changed.is_empty() {
                return Err(p.failed("none of the objects has region sewing"));
            }
            Ok(json!({ "ids": changed }))
        },
    )
}

#[cfg(feature = "digitize")]
fn to_shapes() -> CommandSpec {
    use openstitch_core::{Command, Thread};
    use openstitch_design::{DesignObject, RegionSewing, RegionType, Vec2, v2};
    use openstitch_digitize::coverage::{self, Area, CoverageOptions};

    /// Sewn runs per colour block of a stitch pattern.
    fn runs_by_block(b: &StitchBlock) -> Vec<Vec<Vec<Vec2>>> {
        let mut blocks: Vec<Vec<Vec<Vec2>>> = vec![vec![Vec::new()]];
        for st in &b.stitches {
            let runs = blocks.last_mut().expect("non-empty");
            match st.command {
                Command::Stitch => runs.last_mut().expect("non-empty").push(v2(st.x, st.y)),
                Command::Jump | Command::Trim => {
                    if runs.last().is_some_and(|r| !r.is_empty()) {
                        runs.push(Vec::new());
                    }
                }
                Command::ColorChange => blocks.push(vec![Vec::new()]),
                Command::Stop | Command::End => {}
            }
        }
        blocks
    }

    fn shape(areas: &[Area], thread: &Thread, kind: RegionType) -> Option<DesignObject> {
        let main = areas.iter().max_by(|a, b| a.area.total_cmp(&b.area))?;
        let mut region = RegionSewing::new(kind, thread.clone());
        region.angle = main.angle;
        let contours = areas
            .iter()
            .flat_map(|a| a.contours.iter().cloned())
            .collect();
        Some(DesignObject::shape(contours, Some(region), None))
    }

    spec(
        "object.toShapes",
        "Convert stitch patterns into shapes traced from the area their stitches cover (approximation): one outline, one per colour, or blocks with their own sewing direction",
        Effect::Edit,
        vec![
            IDS,
            opt(
                "mode",
                "whole|parts|blocks",
                "One outline for everything (first thread), one shape per colour, or one block per covered area with satin/fill and direction from its stitches (default parts)",
            ),
            opt(
                "sensitivity",
                "coarse|normal|fine",
                "Tracing grid 0.3 / 0.2 / 0.1 mm and gap closing (default normal)",
            ),
        ],
        |s: &mut EngineState, p: &Params| {
            let idx = indexes(p, &s.design)?;
            let mode = p.string("mode", Some("parts"))?;
            if !matches!(mode.as_str(), "whole" | "parts" | "blocks") {
                return Err(p.invalid("mode", "must be whole, parts or blocks"));
            }
            let (cell, close) = match p.string("sensitivity", Some("normal"))?.as_str() {
                "coarse" => (3.0, 7.0),
                "normal" => (2.0, 5.0),
                "fine" => (1.0, 3.5),
                _ => return Err(p.invalid("sensitivity", "must be coarse, normal or fine")),
            };
            let options = CoverageOptions {
                cell,
                close,
                ..CoverageOptions::default()
            };
            let ids: Vec<u64> = idx
                .iter()
                .map(|&i| &s.design.objects[i])
                .filter(|o| o.kind == ObjectKind::Stitches && o.stitch_block.is_some())
                .map(|o| o.id)
                .collect();
            if ids.is_empty() {
                return Err(p.failed("no stitch patterns to convert"));
            }
            let mut created = Vec::new();
            for id in ids {
                let i = s.design.index_of(id).expect("known");
                let original = s.design.objects[i].clone();
                let block = original.stitch_block.as_ref().expect("checked");
                let thread_of = |k: usize| {
                    block
                        .threads
                        .get(k)
                        .or(block.threads.last())
                        .cloned()
                        .unwrap_or(Thread::rgb(0))
                };
                let blocks = runs_by_block(block);
                let mut shapes: Vec<DesignObject> = Vec::new();
                match mode.as_str() {
                    "whole" => {
                        let all: Vec<Vec<Vec2>> = blocks.iter().flatten().cloned().collect();
                        shapes.extend(shape(
                            &coverage::trace(&all, &options),
                            &thread_of(0),
                            RegionType::Fill,
                        ));
                    }
                    "parts" => {
                        let mut seen: Vec<Thread> = Vec::new();
                        for k in 0..blocks.len() {
                            let t = thread_of(k);
                            if seen.contains(&t) {
                                continue;
                            }
                            let runs: Vec<Vec<Vec2>> = (k..blocks.len())
                                .filter(|&j| thread_of(j) == t)
                                .flat_map(|j| blocks[j].iter().cloned())
                                .collect();
                            seen.push(t.clone());
                            shapes.extend(shape(
                                &coverage::trace(&runs, &options),
                                &t,
                                RegionType::Fill,
                            ));
                        }
                    }
                    _ => {
                        for (k, runs) in blocks.iter().enumerate() {
                            for a in coverage::trace(runs, &options) {
                                // Satin stitches span the area; fill stitches are shorter.
                                let satin = a.thickness > 0.0
                                    && a.thickness <= 100.0
                                    && a.stitch_length >= 0.7 * a.thickness;
                                let kind = if satin {
                                    RegionType::Satin
                                } else {
                                    RegionType::Fill
                                };
                                shapes.extend(shape(std::slice::from_ref(&a), &thread_of(k), kind));
                            }
                        }
                    }
                }
                if shapes.is_empty() {
                    return Err(p.failed(format!("object {id} has no sewn stitches to trace")));
                }
                let group = if shapes.len() > 1 {
                    Some(original.group.unwrap_or_else(|| s.design.new_group_id()))
                } else {
                    original.group
                };
                s.design.objects.remove(i);
                let base = original
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Pattern {id}"));
                for (n, mut o) in shapes.into_iter().enumerate() {
                    o.name = Some(format!("{base} outline {}", n + 1));
                    o.group = group;
                    o.visible = original.visible;
                    let new = s.design.add(o);
                    let last = s.design.objects.pop().expect("just added");
                    s.design.objects.insert(i + n, last);
                    created.push(new);
                }
            }
            s.design.dissolve_single_groups();
            Ok(json!({ "ids": created }))
        },
    )
}
