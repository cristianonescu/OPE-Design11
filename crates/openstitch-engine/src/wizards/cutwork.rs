//! Cutwork wizard (PE-DESIGN Tutorial 9-3, "On Using the Cutwork Functions"): the selected
//! closed outlines become cutouts. Order: stabilizing run around each cutout, the cut lines,
//! a run securing water-soluble stabilizer, net fill across the cutouts, then the covering
//! satin over the cut edges with a zigzag underlay.
//!
//! The cutwork machine protocol is not public, so the cut is written the way cutting lines
//! already are: each cutwork needle (blade angle 0°, 45°, 90°, 135°) is its own colour block
//! with the thread named "Cutwork needle N (A°)"; the colour change before each block is where
//! the machine stops for the needle to be selected (see `openstitch_design::compile`).

use super::{
    closed_outlines, insert_stages, outline_regions, pattern_colour, selected, stage, stage_thread,
};
use crate::commands::{CommandSpec, Effect, MM, opt, req, spec, thread};
use crate::params::{CommandResult, Params};
use crate::session::EngineState;
use openstitch_design::boolean;
use openstitch_design::patterns::NET_PATTERNS;
use openstitch_design::{DesignObject, LineSewing, LineType, ObjectKind, RegionSewing, RegionType};
use serde_json::json;

/// Thread name of a cutting line sewn for scissors (`cutting: "running"`).
pub const CUTTING_LINE: &str = "Cutwork cutting line";
/// Net fill patterns offered by the wizard (the guide lists five).
const NETS: usize = 5;
/// How far outside the cut the stabilizing run lies, mm.
const STABILIZE_MM: f64 = 1.0;
/// How far outside the cut the run securing the water-soluble stabilizer lies, mm.
const SECURE_MM: f64 = 0.5;

pub fn commands() -> Vec<CommandSpec> {
    vec![spec(
        "wizard.cutwork",
        "Cutwork wizard: the closed outlines become cutouts with stabilizing run, cut lines, stabilizer run, net fill and covering satin (one group, replaces them)",
        Effect::Edit,
        vec![
            req(
                "ids",
                "[int]",
                "Closed shapes, or text (not both), to cut out",
            ),
            opt(
                "stabilize",
                "bool",
                "Running stitch around the cut edge before cutting (default true)",
            ),
            opt(
                "cutting",
                "cutting|running",
                "Cut with the cutwork needles, or sew a line to cut by hand (default cutting)",
            ),
            opt(
                "secureStabilizer",
                "bool",
                "Running stitch after cutting to hold water-soluble stabilizer (default true)",
            ),
            opt(
                "netFill",
                "bool",
                "Net fill across the cutouts (default true)",
            ),
            opt(
                "netPattern",
                "int 1-5",
                "Net pattern (square, diamond, triangle, honeycomb, star; default 1)",
            ),
            opt("netSpacing", "mm", "Net line spacing 2-10 (default 3)"),
            opt("netDirection", "degrees", "Net direction 0-90 (default 0)"),
            opt(
                "cover",
                "bool",
                "Covering satin over the cut edges (default true)",
            ),
            opt("coverWidth", "mm", "Satin width 3-6 (default 4)"),
            opt(
                "coverDensity",
                "lines/mm",
                "Satin density 5-7 (default 6.5)",
            ),
            opt(
                "color",
                "color",
                "Thread (default the first pattern's colour)",
            ),
        ],
        cutwork,
    )]
}

fn cutwork(s: &mut EngineState, p: &Params) -> CommandResult {
    let idx = selected(p, &s.design)?;
    closed_outlines(p, &s.design, &idx)?;
    let text = idx
        .iter()
        .filter(|&&i| s.design.objects[i].kind == ObjectKind::Text)
        .count();
    if text != 0 && text != idx.len() {
        return Err(p.failed("shape and text patterns cannot be combined"));
    }
    for &i in &idx {
        let o = &s.design.objects[i];
        if o.contours
            .iter()
            .any(|c| crate::shape_ops::self_crossing(&c.outline_points()))
        {
            return Err(p.failed(format!(
                "object {} has an outline that crosses itself",
                o.id
            )));
        }
    }
    let stabilize = p.bool("stabilize", Some(true))?;
    let needles = match p.string("cutting", Some("cutting"))?.as_str() {
        "cutting" => true,
        "running" => false,
        _ => return Err(p.invalid("cutting", "must be cutting or running")),
    };
    let secure = p.bool("secureStabilizer", Some(true))?;
    let net = p.bool("netFill", Some(true))?;
    let net_pattern = p.int("netPattern", Some(1), 1, NETS as i64)? as usize;
    let net_spacing = p.opt("netSpacing", 3.0, 2.0, 10.0)?;
    let net_direction = p.opt("netDirection", 0.0, 0.0, 90.0)?;
    let cover = p.bool("cover", Some(true))?;
    let width = p.opt("coverWidth", 4.0, 3.0, 6.0)?;
    let density = p.opt("coverDensity", 6.5, 5.0, 7.0)?;
    let color = match thread(p, "color")? {
        Some(t) => t,
        None => pattern_colour(&s.design.objects[idx[0]]),
    };

    let regions = outline_regions(&s.design, &idx);
    let cut = boolean::contours(&regions);
    if cut.is_empty() {
        return Err(p.failed("the outlines enclose no area to cut out"));
    }
    let around = |mm: f64| boolean::contours(&boolean::offset(&cut, mm * MM, true));
    let running = || LineSewing::new(LineType::Running, color.clone());

    let mut stages: Vec<DesignObject> = Vec::new();
    if stabilize {
        stages.push(stage(
            "Cutwork stabilizing run",
            around(STABILIZE_MM),
            running(),
        ));
    }
    let line = if needles {
        LineSewing::new(LineType::Cutting, color.clone())
    } else {
        LineSewing::new(LineType::Running, stage_thread(&color, CUTTING_LINE))
    };
    stages.push(stage("Cutwork cutting line", cut.clone(), line));
    if secure {
        stages.push(stage(
            "Cutwork stabilizer run",
            around(SECURE_MM),
            running(),
        ));
    }
    if net {
        // The net reaches under the satin (or just past the cut) so it is anchored in fabric.
        let reach = if cover { width / 4.0 } else { SECURE_MM };
        let mut r = RegionSewing::new(RegionType::NetFill, color.clone());
        r.pattern = Some(NET_PATTERNS[net_pattern - 1].id.to_string());
        r.spacing = Some(net_spacing);
        r.angle = net_direction;
        let mut o = DesignObject::shape(around(reach), Some(r), None);
        o.name = Some("Cutwork net fill".into());
        stages.push(o);
    }
    if cover {
        let mut under = LineSewing::new(LineType::Zigzag, color.clone());
        under.width = width * 0.6;
        under.density = 1.0;
        stages.push(stage("Cutwork cover underlay", cut.clone(), under));
        let mut satin = LineSewing::new(LineType::Zigzag, color.clone());
        satin.width = width;
        satin.density = density;
        stages.push(stage("Cutwork cover", cut, satin));
    }

    let at = idx[0];
    let removed: Vec<u64> = idx.iter().map(|&i| s.design.objects[i].id).collect();
    crate::shape_ops::remove(&mut s.design, &idx);
    let ids = insert_stages(&mut s.design, at, stages);
    Ok(json!({ "ids": ids, "removed": removed }))
}
