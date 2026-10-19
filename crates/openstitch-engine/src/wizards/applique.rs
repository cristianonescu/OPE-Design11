//! Appliqué wizard (PE-DESIGN Tutorial 9-1): from the selected outlines, the appliqué material
//! line (sewn or cut), a stop, the position line, a stop, the tack-down, a stop, then the
//! covering stitch. Each stage is a line object; the stops are colour changes between threads
//! named after the stages.

use super::{
    MAX_PIECES, closed_outlines, footprint, insert_stages, outer, outline_regions, pattern_colour,
    selected, stage, stage_thread,
};
use crate::commands::{CommandSpec, Effect, MM, opt, req, spec, thread};
use crate::params::{CommandError, CommandResult, Params};
use crate::session::EngineState;
use openstitch_core::Thread;
use openstitch_design::boolean;
use openstitch_design::{ChainShape, DesignObject, LineSewing, LineType};
use serde_json::json;

/// Thread names of the appliqué stages; each is its own colour block, so the machine stops
/// before the next stage.
pub const MATERIAL: &str = "Appliqué material";
pub const POSITION: &str = "Appliqué position";
pub const TACK_DOWN: &str = "Appliqué tack-down";

pub fn commands() -> Vec<CommandSpec> {
    vec![spec(
        "wizard.applique",
        "Appliqué wizard: material line, stop, position line, stop, tack-down, stop, covering stitch (one group)",
        Effect::Edit,
        vec![
            req(
                "ids",
                "[int]",
                "Closed shapes or text to make the appliqué from",
            ),
            opt(
                "material",
                "running|cutting|none",
                "Appliqué material: sewn cutting line, cut by cutwork needles, or none (default running)",
            ),
            opt(
                "tackDown",
                "zigzag|eStitch|vStitch|none",
                "Tack-down stitch (default zigzag)",
            ),
            opt(
                "tackDownWidth",
                "mm",
                "Tack-down zigzag width / E-V stroke 0.5-10 (default 2)",
            ),
            opt(
                "tackDownDensity",
                "lines/mm",
                "Tack-down zigzag density 0.2-5 (default 1)",
            ),
            opt(
                "tackDownInterval",
                "mm",
                "Tack-down E/V interval 0.5-10 (default 3)",
            ),
            opt(
                "cover",
                "zigzag|eStitch|vStitch|chain",
                "Covering stitch (default zigzag)",
            ),
            opt(
                "coverWidth",
                "mm",
                "Zigzag width, E/V stroke or chain size 0.5-10 (default 3)",
            ),
            opt(
                "coverDensity",
                "lines/mm",
                "Zigzag density 1-10 (default 5)",
            ),
            opt("coverInterval", "mm", "E/V interval 0.5-10 (default 3)"),
            opt("coverPitch", "mm", "E/V run pitch 1-10 (default 2.5)"),
            opt(
                "chainShape",
                "triangle|diamond",
                "Chain stitch type (default triangle)",
            ),
            opt("coverTimes", "int 1-5", "Chain / E-V passes (default 1)"),
            opt(
                "output",
                "add|replace",
                "Add the appliqué around the patterns, or sew it along their outlines and delete them (default add)",
            ),
            opt(
                "distance",
                "mm",
                "Add: distance from the patterns 0-50 (default 0)",
            ),
            opt(
                "holes",
                "bool",
                "Replace: keep the patterns' holes as empty areas (default true)",
            ),
            opt(
                "color",
                "color",
                "Covering thread (default the first pattern's colour)",
            ),
        ],
        applique,
    )]
}

fn applique(s: &mut EngineState, p: &Params) -> CommandResult {
    let idx = selected(p, &s.design)?;
    let material = match p.string("material", Some("running"))?.as_str() {
        "running" => Some(false),
        "cutting" => Some(true),
        "none" => None,
        _ => return Err(p.invalid("material", "must be running, cutting or none")),
    };
    let replace = match p.string("output", Some("add"))?.as_str() {
        "add" => false,
        "replace" => true,
        _ => return Err(p.invalid("output", "must be add or replace")),
    };
    let distance = p.opt("distance", 0.0, 0.0, 50.0)? * MM;
    if replace && p.has("distance") {
        return Err(p.invalid("distance", "applies to output add only"));
    }
    let holes = p.bool("holes", Some(true))?;
    let color = match thread(p, "color")? {
        Some(t) => t,
        None => pattern_colour(&s.design.objects[idx[0]]),
    };
    let tack = tack_down(p, &color)?;
    let cover = cover(p, color.clone())?;

    let outline = if replace {
        closed_outlines(p, &s.design, &idx)?;
        let regions = outline_regions(&s.design, &idx);
        if holes {
            boolean::contours(&regions)
        } else {
            outer(&regions)
        }
    } else {
        let around = footprint(&s.design, &idx);
        if distance > 0.0 {
            outer(&boolean::offset(&around, distance, true))
        } else {
            around
        }
    };
    if outline.is_empty() {
        return Err(p.failed("the patterns have no outline to make an appliqué from"));
    }
    if outline.iter().map(|c| c.points.len()).sum::<usize>() > MAX_PIECES * 10 {
        return Err(p.failed("the outline is too complex"));
    }

    let mut stages: Vec<DesignObject> = Vec::new();
    if let Some(cut) = material {
        let mut line = LineSewing::new(
            if cut {
                LineType::Cutting
            } else {
                LineType::Running
            },
            stage_thread(&color, MATERIAL),
        );
        // Cut lines keep short bridges (cut by hand afterwards), as PE-DESIGN does.
        line.leave_uncut = cut;
        stages.push(stage(MATERIAL, outline.clone(), line));
    }
    let position = LineSewing::new(LineType::Running, stage_thread(&color, POSITION));
    stages.push(stage(POSITION, outline.clone(), position));
    if let Some(t) = tack {
        stages.push(stage(TACK_DOWN, outline.clone(), t));
    }
    stages.push(stage("Appliqué cover", outline, cover));

    let at = idx[0];
    let mut removed = Vec::new();
    if replace {
        removed = idx.iter().map(|&i| s.design.objects[i].id).collect();
        crate::shape_ops::remove(&mut s.design, &idx);
    }
    let ids = insert_stages(&mut s.design, at, stages);
    Ok(json!({ "ids": ids, "removed": removed }))
}

fn tack_down(p: &Params, color: &Thread) -> Result<Option<LineSewing>, CommandError> {
    let kind = match p.string("tackDown", Some("zigzag"))?.as_str() {
        "zigzag" => LineType::Zigzag,
        "eStitch" => LineType::EStitch,
        "vStitch" => LineType::VStitch,
        "none" => return Ok(None),
        _ => {
            return Err(p.invalid("tackDown", "must be zigzag, eStitch, vStitch or none"));
        }
    };
    let mut l = LineSewing::new(kind, stage_thread(color, TACK_DOWN));
    l.width = p.opt("tackDownWidth", 2.0, 0.5, 10.0)?;
    l.density = p.opt("tackDownDensity", 1.0, 0.2, 5.0)?;
    l.spacing = Some(p.opt("tackDownInterval", 3.0, 0.5, 10.0)?);
    Ok(Some(l))
}

fn cover(p: &Params, color: Thread) -> Result<LineSewing, CommandError> {
    let kind = match p.string("cover", Some("zigzag"))?.as_str() {
        "zigzag" => LineType::Zigzag,
        "eStitch" => LineType::EStitch,
        "vStitch" => LineType::VStitch,
        "chain" => LineType::Chain,
        _ => return Err(p.invalid("cover", "must be zigzag, eStitch, vStitch or chain")),
    };
    let mut l = LineSewing::new(kind, color);
    l.width = p.opt("coverWidth", 3.0, 0.5, 10.0)?;
    l.density = p.opt("coverDensity", 5.0, 1.0, 10.0)?;
    l.spacing = Some(p.opt("coverInterval", 3.0, 0.5, 10.0)?);
    l.pitch = p.opt("coverPitch", 2.5, 1.0, 10.0)?;
    l.run_times = p.int("coverTimes", Some(1), 1, 5)? as u32;
    l.chain_shape = match p.string("chainShape", Some("triangle"))?.as_str() {
        "triangle" => ChainShape::Triangle,
        "diamond" => ChainShape::Diamond,
        _ => return Err(p.invalid("chainShape", "must be triangle or diamond")),
    };
    Ok(l)
}
