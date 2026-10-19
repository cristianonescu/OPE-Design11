//! Embroidered patch wizard (PE-DESIGN Tutorial 9-2): a satin border around the patterns at a
//! distance, then a cutting line outside the border (sewn for cutting by hand, or cut by
//! cutwork needles).

use super::{footprint, insert_stages, outer, selected, stage, stage_thread};
use crate::commands::{CommandSpec, Effect, MM, default_line, opt, req, spec, thread};
use crate::params::{CommandResult, Params};
use crate::session::EngineState;
use openstitch_design::boolean;
use openstitch_design::{Contour, LineSewing, LineType};
use serde_json::json;

/// Thread name of the sewn cutting line; its own colour block, so the machine stops before it.
pub const CUTTING_LINE: &str = "Patch cutting line";

pub fn commands() -> Vec<CommandSpec> {
    vec![spec(
        "wizard.patch",
        "Embroidered patch wizard: satin border around the patterns, then a cutting line outside it (one group sewn after them)",
        Effect::Edit,
        vec![
            req("ids", "[int]", "Patterns the patch holds"),
            opt("border", "bool", "Sew a satin border (default true)"),
            opt("width", "mm", "Border width 1-10 (default 3)"),
            opt("density", "lines/mm", "Border density 1-10 (default 4.5)"),
            opt(
                "distance",
                "mm",
                "Gap between the patterns and the border 0-50 (default 5)",
            ),
            opt(
                "cutting",
                "running|cutting|none",
                "Cutting line: sewn for scissors, cut by cutwork needles, or none (default running)",
            ),
            opt(
                "cutDistance",
                "mm",
                "Gap between the border (or the patterns) and the cutting line 0-20 (default 3)",
            ),
            opt("color", "color", "Border thread (default black)"),
        ],
        patch,
    )]
}

fn patch(s: &mut EngineState, p: &Params) -> CommandResult {
    let idx = selected(p, &s.design)?;
    let border = p.bool("border", Some(true))?;
    let width = p.opt("width", 3.0, 1.0, 10.0)?;
    let density = p.opt("density", 4.5, 1.0, 10.0)?;
    let distance = p.opt("distance", 5.0, 0.0, 50.0)? * MM;
    let cut = match p.string("cutting", Some("running"))?.as_str() {
        "running" => Some(false),
        "cutting" => Some(true),
        "none" => None,
        _ => return Err(p.invalid("cutting", "must be running, cutting or none")),
    };
    let cut_distance = p.opt("cutDistance", 3.0, 0.0, 20.0)? * MM;
    if !border && cut.is_none() {
        return Err(p.failed("nothing to make: no border and no cutting line"));
    }
    let color = thread(p, "color")?.unwrap_or_else(default_line);
    let around = footprint(&s.design, &idx);
    if around.is_empty() {
        return Err(p.failed("the patterns have no outline or stitches"));
    }
    let ring = |d: f64| -> Vec<Contour> {
        if d <= 0.0 {
            return around.clone();
        }
        outer(&boolean::offset(&around, d, true))
    };
    let mut stages = Vec::new();
    let mut edge = distance;
    if border {
        let mut line = LineSewing::new(LineType::Zigzag, color.clone());
        line.width = width;
        line.density = density;
        stages.push(stage(
            "Patch border",
            ring(distance + width * MM / 2.0),
            line,
        ));
        edge += width * MM;
    }
    if let Some(cutwork) = cut {
        let mut line = LineSewing::new(
            if cutwork {
                LineType::Cutting
            } else {
                LineType::Running
            },
            stage_thread(&color, CUTTING_LINE),
        );
        // Cut lines keep short bridges, cut by hand afterwards, as PE-DESIGN notes.
        line.leave_uncut = cutwork;
        let gap = if border {
            edge + cut_distance
        } else {
            cut_distance
        };
        stages.push(stage("Patch cutting line", ring(gap), line));
    }
    let at = idx[idx.len() - 1] + 1;
    let ids = insert_stages(&mut s.design, at, stages);
    Ok(json!({ "ids": ids }))
}
