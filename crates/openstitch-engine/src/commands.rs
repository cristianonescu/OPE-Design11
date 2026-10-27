//! The command catalogue. User-facing lengths are millimetres, angles degrees; the model stores
//! 0.1 mm. A command added here is available to every front end.

use crate::params::{CommandError, CommandResult, Params};
use crate::project;
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_core::{Command, PEC_CHART, StitchPattern, Thread};
use openstitch_design::patterns::{self, Pattern, PatternKind};
use openstitch_design::shapes::{self, ArcMode};
use openstitch_design::text::{self, TextAlign, TextDirection, TextSpec};
use openstitch_design::{
    Arrange, Bezier, ChainShape, Compiler, Contour, Design, DesignObject, DesignPage, Feather,
    FeatherSide, Gradation, Layout, LineSewing, LineType, ObjectKind, Rect, RegionSewing,
    RegionType, RunningStitchPath, Transform, UnderlayStyle, Vec2, v2,
};
use serde_json::{Value, json};

/// What a command may change; drives undo bookkeeping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Reads the document only.
    Read,
    /// Changes the document: one undo step, rolled back on failure.
    Edit,
    /// Reads or writes files; document unchanged.
    File,
    /// Undo / redo.
    History,
    /// Changes the user settings (attribute sets, thread charts): saved to the user's settings
    /// file, outside the design and its history.
    Settings,
}

impl Effect {
    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Read => "read",
            Effect::Edit => "edit",
            Effect::File => "file",
            Effect::History => "history",
            Effect::Settings => "settings",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParamDoc {
    pub name: &'static str,
    pub kind: &'static str,
    pub required: bool,
    pub summary: &'static str,
}

pub(crate) const fn req(name: &'static str, kind: &'static str, summary: &'static str) -> ParamDoc {
    ParamDoc {
        name,
        kind,
        required: true,
        summary,
    }
}

pub(crate) const fn opt(name: &'static str, kind: &'static str, summary: &'static str) -> ParamDoc {
    ParamDoc {
        name,
        kind,
        required: false,
        summary,
    }
}

pub type RunFn = fn(&mut EngineState, &Params) -> CommandResult;

#[derive(Clone)]
pub struct CommandSpec {
    pub id: &'static str,
    pub summary: &'static str,
    pub effect: Effect,
    pub params: Vec<ParamDoc>,
    pub run: RunFn,
}

pub(crate) fn spec(
    id: &'static str,
    summary: &'static str,
    effect: Effect,
    params: Vec<ParamDoc>,
    run: RunFn,
) -> CommandSpec {
    CommandSpec {
        id,
        summary,
        effect,
        params,
        run,
    }
}

pub(crate) const MM: f64 = 10.0;
pub(crate) const IDS: ParamDoc = req("ids", "[int] | id: int", "Objects to change");
pub(crate) const PATH: ParamDoc = req(
    "path",
    "string",
    "File path (relative to the session directory)",
);

pub(crate) fn default_fill() -> Thread {
    PEC_CHART[2].clone().unwrap_or(Thread::rgb(0x0A55A3))
}

pub(crate) fn default_line() -> Thread {
    PEC_CHART[20].clone().unwrap_or(Thread::rgb(0))
}

/// `"#rrggbb"`, a Brother PEC chart name (`"Red"`) or chart number 1–64.
pub(crate) fn thread(p: &Params, key: &str) -> Result<Option<Thread>, CommandError> {
    let Some(v) = p.get(key) else { return Ok(None) };
    if v.is_number() {
        let i = p.int(key, None, 1, 64)? as usize;
        return Ok(PEC_CHART[i].clone());
    }
    let s = p.string(key, None)?;
    let s = s.trim();
    if s.starts_with('#')
        && let Some(t) = Thread::from_hex(s)
    {
        return Ok(Some(t));
    }
    let named = PEC_CHART
        .iter()
        .flatten()
        .find(|t| t.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(s)));
    named.cloned().map(Some).ok_or_else(|| {
        p.invalid(
            key,
            "must be #rrggbb, a Brother thread name or chart number 1-64",
        )
    })
}

pub(crate) fn indexes(p: &Params, d: &Design) -> Result<Vec<usize>, CommandError> {
    let ids = p.ids()?;
    if ids.is_empty() {
        return Err(p.invalid("ids", "must not be empty"));
    }
    ids.iter()
        .map(|&id| {
            d.index_of(id)
                .ok_or_else(|| p.not_found(format!("object {id}")))
        })
        .collect()
}

fn describe(o: &DesignObject) -> Value {
    let mut r = json!({ "id": o.id, "kind": kind_str(o.kind), "visible": o.visible });
    if let Some(n) = &o.name {
        r["name"] = json!(n);
    }
    if let Some(b) = o.bounds() {
        r["bounds"] = json!({ "x": b.min_x / MM, "y": b.min_y / MM, "width": b.width() / MM, "height": b.height() / MM });
    }
    if let Some(region) = &o.region {
        r["region"] = json!(region.kind.id());
        r["regionColor"] = json!(region.thread.hex());
    }
    if let Some(line) = &o.line {
        r["line"] = json!(line.kind.id());
        r["lineColor"] = json!(line.thread.hex());
    }
    if let Some(g) = o.group {
        r["group"] = json!(g);
    }
    if let Some(t) = &o.text {
        r["text"] = json!(t.text);
        r["font"] = json!(t.font);
        r["sizeMM"] = json!(t.size);
        if t.kerning {
            r["kerning"] = json!(true);
        }
        if t.direction == TextDirection::Vertical {
            r["direction"] = json!("vertical");
        }
        if !t.characters.is_empty() {
            r["characters"] = json!(t.characters);
        }
        if let Some(path) = t.path {
            r["path"] = json!(path);
        }
    }
    r
}

fn kind_str(k: ObjectKind) -> &'static str {
    match k {
        ObjectKind::Shape => "shape",
        ObjectKind::Stitches => "stitches",
        ObjectKind::Text => "text",
    }
}

pub fn stats(p: &StitchPattern) -> Value {
    let b = p.bounds();
    json!({
        "stitches": p.count(Command::Stitch),
        "jumps": p.count(Command::Jump),
        "trims": p.count(Command::Trim),
        "colorChanges": p.count(Command::ColorChange),
        "widthMM": (b.width() / MM * 10.0).round() / 10.0,
        "heightMM": (b.height() / MM * 10.0).round() / 10.0,
        "colors": p.threads.iter().map(Thread::hex).collect::<Vec<_>>(),
    })
}

pub fn all() -> Vec<CommandSpec> {
    let mut v = Vec::new();
    v.extend(design_commands());
    v.extend(shape_commands());
    v.extend(attribute_commands());
    v.extend(path_commands());
    v.extend(transform_commands());
    v.extend(text_commands());
    v.extend(crate::fonts::commands());
    #[cfg(feature = "digitize")]
    v.extend(crate::font_images::commands());
    v.extend(crate::lettering::commands());
    v.extend(crate::templates::commands());
    v.extend(crate::drawing::commands());
    v.extend(crate::ornament::commands());
    v.extend(crate::user::commands());
    v.extend(crate::punch::commands());
    v.extend(crate::order::commands());
    v.extend(crate::convert::commands());
    v.extend(crate::stitch_edit::commands());
    v.extend(crate::sewing::commands());
    v.extend(crate::stamps::commands());
    v.extend(file_commands());
    #[cfg(feature = "digitize")]
    v.extend(crate::digitize::commands());
    #[cfg(feature = "formats")]
    v.extend(crate::imports::commands());
    #[cfg(feature = "library")]
    v.extend(crate::library::commands());
    #[cfg(feature = "formats")]
    v.extend(crate::transfer::commands());
    v.extend(crate::machine::commands());
    v.extend(crate::sections::commands());
    v.extend(crate::guides::commands());
    v.extend(crate::fabric::commands());
    v.extend(crate::print::commands());
    v.extend(crate::pattern_cmds::commands());
    v.extend(crate::sewing_cmds::commands());
    v.extend(crate::wizards::commands());
    v.push(spec(
        "commands.list",
        "Describe every command",
        Effect::Read,
        vec![],
        |_, _| Ok(list()),
    ));
    v.push(spec(
        "history.undo",
        "Undo the last edit",
        Effect::History,
        vec![],
        |_, _| Ok(Value::Null),
    ));
    v.push(spec(
        "history.redo",
        "Redo the last undone edit",
        Effect::History,
        vec![],
        |_, _| Ok(Value::Null),
    ));
    v
}

fn list() -> Value {
    let mut cmds = all();
    cmds.sort_by_key(|c| c.id);
    Value::Array(
        cmds.iter()
            .map(|c| {
                json!({
                    "id": c.id, "summary": c.summary, "effect": c.effect.as_str(),
                    "requires": crate::capabilities::required(c).iter().map(|r| r.as_str()).collect::<Vec<_>>(),
                    "params": c.params.iter().map(|p| json!({
                        "name": p.name, "type": p.kind, "required": p.required, "summary": p.summary
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

// MARK: design

/// A Design Property field: at most 255 bytes (one PES header string), no control characters
/// other than line breaks and tabs.
fn validate_property(p: &Params, key: &str, value: &str) -> Result<(), CommandError> {
    if value.len() > openstitch_design::DesignProperties::MAX_BYTES {
        return Err(p.invalid(key, "must be at most 255 bytes (UTF-8)"));
    }
    if value
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(p.invalid(key, "must not contain control characters"));
    }
    Ok(())
}

fn design_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "design.new",
            "Start a new empty design (replaces the current one; undoable)",
            Effect::Edit,
            vec![
                opt("name", "string", "Design name"),
                opt("width", "mm", "Design Page width (default 100)"),
                opt("height", "mm", "Design Page height (default 100)"),
            ],
            |s, p| {
                let w = p.opt("width", 100.0, 10.0, 2000.0)?;
                let h = p.opt("height", 100.0, 10.0, 2000.0)?;
                s.design = Design::new(
                    p.string("name", Some("Untitled"))?,
                    DesignPage {
                        width: w * MM,
                        height: h * MM,
                    },
                );
                Ok(json!({ "name": s.design.name }))
            },
        ),
        spec(
            "design.rename",
            "Rename the design",
            Effect::Edit,
            vec![req("name", "string", "New name")],
            |s, p| {
                let name = p.string("name", None)?;
                if name.is_empty() || name.chars().count() > 200 {
                    return Err(p.invalid("name", "must be 1-200 characters"));
                }
                s.design.name = name;
                Ok(Value::Null)
            },
        ),
        spec(
            "design.setProperties",
            "Set the Design Property fields written to PES (category, author, keywords, comments)",
            Effect::Edit,
            vec![
                opt("category", "string", "Category (\"\" clears)"),
                opt("author", "string", "Author (\"\" clears)"),
                opt("keywords", "string", "Keywords (\"\" clears)"),
                opt("comments", "string", "Comments (\"\" clears)"),
            ],
            |s, p| {
                let mut properties = s.design.properties.clone();
                let mut given = false;
                for key in openstitch_design::DesignProperties::KEYS {
                    if !p.has(key) {
                        continue;
                    }
                    given = true;
                    let value = p.string(key, None)?;
                    validate_property(p, key, &value)?;
                    if let Some(field) = properties.field_mut(key) {
                        *field = value;
                    }
                }
                if !given {
                    return Err(
                        p.invalid("category", "give category, author, keywords or comments")
                    );
                }
                s.design.properties = properties;
                Ok(json!(s.design.properties))
            },
        ),
        spec(
            "design.info",
            "Design summary, stitch statistics, thread length and sewing-time estimates, sewing area",
            Effect::Read,
            vec![],
            |s, _| {
                let pattern = Compiler::for_design(&s.design).compile(&s.design);
                Ok(json!({
                    "name": s.design.name,
                    "properties": s.design.properties,
                    "pageWidthMM": s.design.page.width / MM,
                    "pageHeightMM": s.design.page.height / MM,
                    "objects": s.design.objects.len(),
                    "stitchStats": stats(&pattern),
                    "estimates": crate::machine::estimates(&s.design, &pattern),
                    "sewingArea": crate::machine::sewing_area(&s.design, &pattern),
                }))
            },
        ),
        spec(
            "design.generate",
            "Generate stitches and report statistics",
            Effect::Read,
            vec![],
            |s, _| Ok(stats(&Compiler::for_design(&s.design).compile(&s.design))),
        ),
        spec(
            "object.list",
            "List objects in sewing order",
            Effect::Read,
            vec![],
            |s, _| {
                Ok(Value::Array(
                    s.design.objects.iter().map(describe).collect(),
                ))
            },
        ),
        spec(
            "object.get",
            "Full object data",
            Effect::Read,
            vec![req("id", "int", "Object id")],
            |s, p| {
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                serde_json::to_value(&s.design.objects[i]).map_err(|e| p.failed(e.to_string()))
            },
        ),
    ]
}

// MARK: shapes

fn shape_docs(mut v: Vec<ParamDoc>) -> Vec<ParamDoc> {
    v.extend([
        opt(
            "fill",
            "color",
            "Region fill thread (default Brother Blue unless 'outline' only)",
        ),
        opt("outline", "color", "Outline running-stitch thread"),
        opt("name", "string", "Object name"),
    ]);
    v
}

/// Region and line sewing from the `fill` / `outline` parameters: a closed outline is filled
/// unless only `outline` is given.
fn sewing(
    p: &Params,
    closed: bool,
) -> Result<(Option<RegionSewing>, Option<LineSewing>), CommandError> {
    let fill = thread(p, "fill")?;
    let outline = thread(p, "outline")?;
    let region = (closed && (fill.is_some() || outline.is_none()))
        .then(|| RegionSewing::new(RegionType::Fill, fill.unwrap_or_else(default_fill)));
    let line = (outline.is_some() || !closed).then(|| {
        LineSewing::new(
            LineType::Running,
            outline.clone().unwrap_or_else(default_line),
        )
    });
    Ok((region, line))
}

fn object_name(p: &Params) -> Result<Option<String>, CommandError> {
    if p.has("name") {
        Ok(Some(p.string("name", None)?))
    } else {
        Ok(None)
    }
}

/// Whether object `i` can be one of a hole-sewing pair: a region whose outline does not cross
/// itself, with at most one sewing-direction line and no feathered edges (the guide's limits).
fn hole_member(d: &Design, p: &Params, i: usize) -> Result<(), CommandError> {
    let o = &d.objects[i];
    let Some(r) = o
        .region
        .as_ref()
        .filter(|_| openstitch_design::holes::region_contours(o).is_some())
    else {
        return Err(p.failed(format!("object {} has no region sewing", o.id)));
    };
    if o.contours
        .iter()
        .filter(|c| c.closed)
        .any(|c| crate::shape_ops::self_crossing(&c.outline_points()))
    {
        return Err(p.failed(format!(
            "object {} has an outline that crosses itself",
            o.id
        )));
    }
    if r.direction_lines.len() >= 2 {
        return Err(p.failed(format!(
            "object {} has several sewing-direction lines",
            o.id
        )));
    }
    if r.feather.is_some() {
        return Err(p.failed(format!("object {} has feathered edges", o.id)));
    }
    Ok(())
}

/// The (outer, inner) indexes of a hole-sewing pair: two regions where one fully encloses
/// the other, or one region and the region it encloses (the first sewn after it, else the
/// first before it).
fn hole_pair(s: &EngineState, p: &Params, idx: &[usize]) -> Result<(usize, usize), CommandError> {
    use openstitch_design::holes::object_encloses;
    let d = &s.design;
    for &i in idx {
        hole_member(d, p, i)?;
    }
    let (outer, inner) = match idx {
        [a] => {
            let found = (0..d.objects.len())
                .filter(|&j| j != *a && d.objects[j].visible)
                .filter(|&j| object_encloses(&d.objects[*a], &d.objects[j]))
                .min_by_key(|&j| (j < *a, j));
            let Some(j) = found else {
                return Err(p.failed(format!(
                    "object {} encloses no other region",
                    d.objects[*a].id
                )));
            };
            hole_member(d, p, j)?;
            (*a, j)
        }
        [a, b] => {
            if object_encloses(&d.objects[*a], &d.objects[*b]) {
                (*a, *b)
            } else if object_encloses(&d.objects[*b], &d.objects[*a]) {
                (*b, *a)
            } else {
                return Err(p.failed("hole sewing needs one region fully inside the other"));
            }
        }
        _ => return Err(p.invalid("ids", "hole sewing pairs exactly two regions")),
    };
    Ok((outer, inner))
}

fn add_shape(s: &mut EngineState, p: &Params, contour: Contour) -> CommandResult {
    add_shape_ending(s, p, contour, None)
}

/// Adds a shape whose region starts and ends at `ends` (PE-DESIGN's default for its Shape tools).
fn add_shape_ending(
    s: &mut EngineState,
    p: &Params,
    contour: Contour,
    ends: Option<fn(&Contour) -> Option<Vec2>>,
) -> CommandResult {
    let point = ends.and_then(|f| f(&contour));
    let (mut region, line) = sewing(p, contour.closed)?;
    if let (Some(r), Some(at)) = (region.as_mut(), point) {
        r.entry = Some(at);
        r.exit = Some(at);
    }
    // The outline is sewn from the same point (PE-DESIGN sews a circle's line from its bottom),
    // and text fitted to it starts there.
    let mut line = line;
    if let (Some(l), Some(at)) = (line.as_mut(), point) {
        l.entry = Some(at);
    }
    let mut object = DesignObject::shape(vec![contour], region, line);
    object.name = object_name(p)?;
    Ok(json!({ "id": s.design.add(object) }))
}

fn shape_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "shape.rectangle",
            "Add a rectangle",
            Effect::Edit,
            shape_docs(vec![
                req("x", "mm", "Left"),
                req("y", "mm", "Top"),
                req("width", "mm", "Width"),
                req("height", "mm", "Height"),
                opt(
                    "cornerRadius",
                    "mm",
                    "Rounded corners (PE-DESIGN edge radius; default 0, clamped to half the shorter side)",
                ),
            ]),
            |s, p| {
                let (x, y) = (
                    p.req("x", -5000.0, 5000.0)? * MM,
                    p.req("y", -5000.0, 5000.0)? * MM,
                );
                let (w, h) = (
                    p.req("width", 0.1, 2000.0)? * MM,
                    p.req("height", 0.1, 2000.0)? * MM,
                );
                let radius = p.opt("cornerRadius", 0.0, 0.0, 1000.0)? * MM;
                let r = if radius > 0.0 {
                    shapes::rounded_rectangle(w, h, radius)
                        .transformed(&Transform::translation(x, y))
                } else {
                    Contour::rectangle(x, y, w, h)
                };
                add_shape_ending(s, p, r, Some(shapes::bottom_centre))
            },
        ),
        spec(
            "shape.ellipse",
            "Add an ellipse or circle",
            Effect::Edit,
            shape_docs(vec![
                req("cx", "mm", "Centre x"),
                req("cy", "mm", "Centre y"),
                req("rx", "mm", "Horizontal radius"),
                opt("ry", "mm", "Vertical radius (default rx)"),
            ]),
            |s, p| {
                let rx = p.req("rx", 0.1, 1000.0)?;
                let e = Contour::ellipse(
                    v2(
                        p.req("cx", -5000.0, 5000.0)? * MM,
                        p.req("cy", -5000.0, 5000.0)? * MM,
                    ),
                    rx * MM,
                    p.opt("ry", rx, 0.1, 1000.0)? * MM,
                    72,
                );
                add_shape_ending(s, p, e, Some(shapes::bottom_centre))
            },
        ),
        spec(
            "shape.polygon",
            "Add a polygon (closed) or polyline (closed: false)",
            Effect::Edit,
            shape_docs(vec![
                req("points", "[[x, y]] mm", "Vertices"),
                opt("closed", "bool", "Closed region (default true)"),
            ]),
            |s, p| {
                let closed = p.bool("closed", Some(true))?;
                let points = p.points("points", if closed { 3 } else { 2 })?;
                add_shape(s, p, Contour::new(points, closed))
            },
        ),
        spec(
            "shape.freehand",
            "Add a freehand (Pencil) outline: the traced points simplified to within a tolerance and smoothed into curves",
            Effect::Edit,
            shape_docs(vec![
                req(
                    "points",
                    "[[x, y]] mm",
                    "The traced points in order (2-20000)",
                ),
                opt("closed", "bool", "Closed region (default true)"),
                opt(
                    "tolerance",
                    "mm",
                    "How far the outline may leave the trace (0.01-10, default 0.3)",
                ),
                opt(
                    "smooth",
                    "bool",
                    "Curves through the kept points (default true; false: straight lines)",
                ),
            ]),
            |s, p| {
                let closed = p.bool("closed", Some(true))?;
                let points = p.points("points", 2)?;
                if points.len() > openstitch_design::freehand::MAX_POINTS {
                    return Err(p.invalid(
                        "points",
                        format!("at most {} points", openstitch_design::freehand::MAX_POINTS),
                    ));
                }
                let tolerance = p.opt("tolerance", 0.3, 0.01, 10.0)? * MM;
                let smooth = p.bool("smooth", Some(true))?;
                let c = openstitch_design::freehand::fit(&points, closed, tolerance, smooth)
                    .ok_or_else(|| p.invalid("points", "the trace is too short"))?;
                add_shape(s, p, c)
            },
        ),
        spec(
            "shape.arc",
            "Add an elliptical arc, arc and string (chord) or fan (pie slice)",
            Effect::Edit,
            shape_docs(vec![
                req("cx", "mm", "Centre x"),
                req("cy", "mm", "Centre y"),
                req("rx", "mm", "Horizontal radius"),
                opt("ry", "mm", "Vertical radius (default rx)"),
                opt(
                    "start",
                    "degrees",
                    "Start angle, clockwise from 3 o'clock (default 0)",
                ),
                req(
                    "sweep",
                    "degrees",
                    "Angle swept, -360...360 (negative = anticlockwise; ±360 = full ellipse)",
                ),
                opt(
                    "mode",
                    "arc|chord|fan",
                    "How the arc is closed (default arc)",
                ),
            ]),
            |s, p| {
                let rx = p.req("rx", 0.1, 1000.0)?;
                let ry = p.opt("ry", rx, 0.1, 1000.0)?;
                let start = p.opt("start", 0.0, -3600.0, 3600.0)?;
                let sweep = p.req("sweep", -360.0, 360.0)?;
                if sweep.abs() < 0.1 {
                    return Err(p.invalid("sweep", "must be at least 0.1 degrees"));
                }
                let mode = match p.string("mode", Some("arc"))?.as_str() {
                    "arc" => ArcMode::Arc,
                    "chord" => ArcMode::Chord,
                    "fan" => ArcMode::Fan,
                    _ => return Err(p.invalid("mode", "must be arc, chord or fan")),
                };
                let center = v2(
                    p.req("cx", -5000.0, 5000.0)? * MM,
                    p.req("cy", -5000.0, 5000.0)? * MM,
                );
                let c = shapes::arc(center, rx * MM, ry * MM, start, sweep, mode);
                add_shape(s, p, c)
            },
        ),
        spec(
            "shape.preset",
            "Add a preset shape fitted to a box: polygon, star, heart, triangle, diamond, arrow, cross, crescent, teardrop",
            Effect::Edit,
            shape_docs(vec![
                req(
                    "kind",
                    "polygon|star|heart|triangle|diamond|arrow|cross|crescent|teardrop",
                    "Preset",
                ),
                req("cx", "mm", "Centre x"),
                req("cy", "mm", "Centre y"),
                req("width", "mm", "Width before rotation"),
                opt("height", "mm", "Height before rotation (default width)"),
                opt("rotation", "degrees", "Clockwise rotation (default 0)"),
                opt(
                    "sides",
                    "int 3-64",
                    "Polygon sides / star points (default 5)",
                ),
                opt(
                    "ratio",
                    "factor 0.05-0.95",
                    "Star inner radius, arrow shaft or cross arm thickness, crescent bite (default 0.4)",
                ),
            ]),
            |s, p| {
                let sides = p.int("sides", Some(5), 3, 64)? as usize;
                let ratio = p.opt("ratio", 0.4, 0.05, 0.95)?;
                let kind = p.string("kind", None)?;
                // PE-DESIGN 11's start point for the matching gallery shapes; its polygons follow
                // no one rule (hexagon mid-edge, pentagon and octagon at a corner), so they keep none.
                let ends: Option<fn(&Contour) -> Option<Vec2>> = match kind.as_str() {
                    "star" | "heart" | "diamond" | "teardrop" => Some(shapes::bottom_centre),
                    "triangle" => Some(shapes::bottom_left),
                    _ => None,
                };
                let unit = match kind.as_str() {
                    "polygon" => shapes::regular_polygon(sides),
                    "star" => shapes::star(sides, ratio),
                    "heart" => shapes::heart(),
                    "triangle" => shapes::regular_polygon(3),
                    "diamond" => shapes::regular_polygon(4),
                    "arrow" => shapes::arrow(ratio, 0.4),
                    "cross" => shapes::cross(ratio),
                    "crescent" => shapes::crescent(ratio * 2.0),
                    "teardrop" => shapes::teardrop(),
                    _ => {
                        return Err(p.invalid(
                            "kind",
                            "must be polygon, star, heart, triangle, diamond, arrow, cross, crescent or teardrop",
                        ));
                    }
                };
                let w = p.req("width", 0.1, 2000.0)?;
                let h = p.opt("height", w, 0.1, 2000.0)?;
                let center = v2(
                    p.req("cx", -5000.0, 5000.0)? * MM,
                    p.req("cy", -5000.0, 5000.0)? * MM,
                );
                let rotation = p.opt("rotation", 0.0, -3600.0, 3600.0)?;
                let c = shapes::fit(&unit, center, w * MM, h * MM, rotation);
                add_shape_ending(s, p, c, ends)
            },
        ),
    ]
}

// MARK: attributes

// MARK: paths and point editing

const CONTOUR: ParamDoc = opt(
    "contour",
    "int",
    "Contour index within the object (default 0)",
);

/// The shape object and contour named by `id` / `contour`.
fn contour_mut<'a>(s: &'a mut EngineState, p: &Params) -> Result<&'a mut Contour, CommandError> {
    let id = p.int("id", None, 0, i64::MAX)? as u64;
    let i = s
        .design
        .index_of(id)
        .ok_or_else(|| p.not_found(format!("object {id}")))?;
    let o = &mut s.design.objects[i];
    match o.kind {
        ObjectKind::Shape => {}
        ObjectKind::Stitches => {
            return Err(p.failed(format!(
                "object {id} is a stitch pattern; convert it to a shape first"
            )));
        }
        ObjectKind::Text => {
            return Err(p.failed(format!(
                "object {id} is text; convert it to shapes first (text.toShapes)"
            )));
        }
    }
    let n = o.contours.len();
    let c = p.int("contour", Some(0), 0, n.saturating_sub(1) as i64)? as usize;
    o.contours
        .get_mut(c)
        .ok_or_else(|| p.not_found(format!("contour {c} of object {id}")))
}

fn path_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "shape.path",
            "Add a path: vertices plus an optional cubic Bézier per segment",
            Effect::Edit,
            shape_docs(vec![
                req("points", "[[x, y]] mm", "Vertices"),
                opt("closed", "bool", "Closed region (default true)"),
                opt(
                    "curves",
                    "[null | [[c1x, c1y], [c2x, c2y]]] mm",
                    "One entry per segment (segment i runs from vertex i to i+1, the last closes the path); null = straight",
                ),
            ]),
            |s, p| {
                let closed = p.bool("closed", Some(true))?;
                let points = p.points("points", 2)?;
                let mut contour = Contour::new(points, closed);
                if p.has("curves") {
                    let curves = p.array("curves")?;
                    if curves.len() != contour.segment_count() {
                        return Err(p.invalid(
                            "curves",
                            format!("needs one entry per segment ({})", contour.segment_count()),
                        ));
                    }
                    for (i, v) in curves.iter().enumerate() {
                        let c = p
                            .bezier_value("curves", v)?
                            .map(|(c1, c2)| Bezier { c1, c2 });
                        contour.set_curve(i, c);
                    }
                }
                if closed && contour.points.len() < 3 && !contour.has_curves() {
                    return Err(
                        p.invalid("points", "a closed straight path needs at least 3 points")
                    );
                }
                add_shape(s, p, contour)
            },
        ),
        spec(
            "object.setPoint",
            "Move a vertex (its curve handles move with it)",
            Effect::Edit,
            vec![
                req("id", "int", "Shape object"),
                CONTOUR,
                req("index", "int", "Vertex index"),
                req("x", "mm", "New x"),
                req("y", "mm", "New y"),
            ],
            |s, p| {
                let x = p.req("x", -5000.0, 5000.0)? * MM;
                let y = p.req("y", -5000.0, 5000.0)? * MM;
                let c = contour_mut(s, p)?;
                let index = p.int("index", None, 0, c.points.len() as i64 - 1)? as usize;
                let delta = v2(x, y) - c.points[index];
                c.points[index] = v2(x, y);
                let segs = c.segment_count();
                let outgoing = (index < segs).then_some(index);
                let incoming = if index > 0 {
                    Some(index - 1)
                } else if c.closed {
                    Some(segs - 1)
                } else {
                    None
                };
                if let Some(i) = outgoing.filter(|&i| c.curve(i).is_some()) {
                    let mut b = c.curve(i).expect("curve");
                    b.c1 = b.c1 + delta;
                    c.set_curve(i, Some(b));
                }
                if let Some(i) = incoming.filter(|&i| c.curve(i).is_some()) {
                    let mut b = c.curve(i).expect("curve");
                    b.c2 = b.c2 + delta;
                    c.set_curve(i, Some(b));
                }
                Ok(Value::Null)
            },
        ),
        spec(
            "object.insertPoint",
            "Insert a vertex on a segment (curves are split exactly)",
            Effect::Edit,
            vec![
                req("id", "int", "Shape object"),
                CONTOUR,
                req("segment", "int", "Segment index (from vertex i to i+1)"),
                opt(
                    "t",
                    "factor",
                    "Position along the segment, 0-1 exclusive (default 0.5)",
                ),
            ],
            |s, p| {
                let t = p.opt("t", 0.5, 0.001, 0.999)?;
                let c = contour_mut(s, p)?;
                let seg = p.int("segment", None, 0, c.segment_count() as i64 - 1)? as usize;
                if c.points.len() >= 100_000 {
                    return Err(p.failed("contour already has 100000 points"));
                }
                Ok(json!({ "index": c.insert_point(seg, t) }))
            },
        ),
        spec(
            "object.deletePoint",
            "Delete a vertex, joining its neighbouring segments",
            Effect::Edit,
            vec![
                req("id", "int", "Shape object"),
                CONTOUR,
                req("index", "int", "Vertex index"),
            ],
            |s, p| {
                let c = contour_mut(s, p)?;
                let index = p.int("index", None, 0, c.points.len() as i64 - 1)? as usize;
                // Open: 2 points. Closed: 3, or 2 when curves keep the region non-degenerate.
                let minimum = if c.closed && !c.has_curves() { 3 } else { 2 };
                if c.points.len() <= minimum {
                    return Err(p.failed(format!("this contour needs at least {minimum} points")));
                }
                c.delete_point(index);
                Ok(Value::Null)
            },
        ),
        spec(
            "object.setSegment",
            "Make a segment a cubic Bézier (curve) or straight (straight: true)",
            Effect::Edit,
            vec![
                req("id", "int", "Shape object"),
                CONTOUR,
                req("segment", "int", "Segment index (from vertex i to i+1)"),
                opt(
                    "curve",
                    "[[c1x, c1y], [c2x, c2y]] mm",
                    "Control points; omitted with straight false = handles at 1/3 and 2/3 of the chord",
                ),
                opt("straight", "bool", "true makes the segment a straight line"),
            ],
            |s, p| {
                let straight = p.bool("straight", Some(false))?;
                let given = match p.get("curve") {
                    Some(v) => p.bezier_value("curve", v)?,
                    None => None,
                };
                if straight && given.is_some() {
                    return Err(p.invalid("straight", "cannot be combined with 'curve'"));
                }
                let c = contour_mut(s, p)?;
                let seg = p.int("segment", None, 0, c.segment_count() as i64 - 1)? as usize;
                let curve = if straight {
                    None
                } else {
                    let (a, b) = c.segment_ends(seg);
                    let (c1, c2) = given.unwrap_or((a.lerp(b, 1.0 / 3.0), a.lerp(b, 2.0 / 3.0)));
                    Some(Bezier { c1, c2 })
                };
                c.set_curve(seg, curve);
                Ok(Value::Null)
            },
        ),
    ]
}

/// True when `id` names a pattern that region type `kind` can use.
fn region_pattern_exists(kind: RegionType, id: &str, user: &[Pattern]) -> bool {
    let found = |k: PatternKind| {
        patterns::find(k, id).is_some() || user.iter().any(|p| p.kind == k && p.id == id)
    };
    match kind {
        RegionType::ProgrammableFill => found(PatternKind::Fill),
        RegionType::MotifFill => found(PatternKind::Motif),
        RegionType::DecorativeFill => found(PatternKind::Decorative),
        RegionType::NetFill => patterns::NET_PATTERNS.iter().any(|n| n.id == id),
        RegionType::ZigzagNetFill => patterns::NET_PATTERNS[..patterns::ZIGZAG_NET_PATTERNS]
            .iter()
            .any(|n| n.id == id),
        _ => false,
    }
}

/// An optional string parameter that must be one of `options` (value, result).
fn choice<T: Copy>(
    p: &Params,
    key: &str,
    options: &[(&str, T)],
) -> Result<Option<T>, CommandError> {
    if !p.has(key) {
        return Ok(None);
    }
    let v = p.string(key, None)?;
    options
        .iter()
        .find(|(name, _)| *name == v)
        .map(|(_, t)| Some(*t))
        .ok_or_else(|| {
            let names: Vec<&str> = options.iter().map(|(n, _)| *n).collect();
            p.invalid(key, format!("must be one of {}", names.join(", ")))
        })
}

fn attribute_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "object.setRegion",
            "Set region sew type and attributes (type 'none' removes)",
            Effect::Edit,
            vec![
                IDS,
                opt(
                    "type",
                    "fill|satin|programmableFill|motifFill|piping|crossStitch|concentricCircle|radial|spiral|flexibleSpiral|stippling|netFill|zigzagNetFill|decorativeFill|none",
                    "Region sew type",
                ),
                opt("color", "color", "Thread"),
                opt(
                    "angle",
                    "degrees",
                    "Sewing direction, counter-clockwise on screen as in PE-DESIGN (0 = horizontal)",
                ),
                opt(
                    "density",
                    "rows/mm",
                    "Row density (fill and satin 0.1-14, others 0.5-10)",
                ),
                opt("stitchLength", "mm", "Fill stitch length"),
                opt("pullCompensation", "mm", "Row end extension"),
                opt("underlay", "bool", "Underlay (style in underlayStyle)"),
                opt(
                    "underlayStyle",
                    "edge|zigzag|edgeZigzag|double",
                    "Underlay: edge run, inset rows, both, or two crossing layers",
                ),
                opt(
                    "underlaySpacing",
                    "mm",
                    "Distance between underlay rows, 1-10",
                ),
                opt(
                    "autoDirection",
                    "bool",
                    "Choose the direction from the shape (rows across its narrow side)",
                ),
                opt(
                    "gradation",
                    "percent",
                    "Continuous gradation from 100 % to this % of density at the far side (5-100; 100 turns gradation off)",
                ),
                opt(
                    "gradationPattern",
                    "bool",
                    "Gradation as PE-DESIGN's pattern (default Max 53 %, Min 5 %, 9 steps); false turns gradation off",
                ),
                opt(
                    "gradationMax",
                    "percent",
                    "Gradation pattern: density at the start side, % of density (5-100)",
                ),
                opt(
                    "gradationMin",
                    "percent",
                    "Gradation pattern: density at the far side, % of density (5-100)",
                ),
                opt(
                    "gradationSteps",
                    "int",
                    "Gradation pattern: number of equal bands (2-20)",
                ),
                opt(
                    "featherSide",
                    "start|end|both",
                    "Feathered edge: which row ends",
                ),
                opt(
                    "featherWidth",
                    "mm",
                    "Feathered edge depth, 0-10 (0 turns it off)",
                ),
                opt(
                    "splitLength",
                    "mm",
                    "Satin: split stitches longer than this, 2-12 (0 turns splitting off)",
                ),
                opt(
                    "runningStitchPath",
                    "inside|alongOutline|onOutline",
                    "Fill and satin: running stitches between separate parts (default inside; not with Auto direction, a feathered edge or text; inside not with gradation)",
                ),
                opt(
                    "travel",
                    "bool",
                    "Travel between parts with running stitches (true) or jump (false)",
                ),
                opt(
                    "pattern",
                    "string",
                    "Pattern id from patterns.list: fill (programmable fill), motif (motif fill), decorative, net (net / zigzag net fill)",
                ),
                opt(
                    "patternWidth",
                    "mm",
                    "Pattern width; zigzag net: zigzag width, 0.5-50",
                ),
                opt("patternHeight", "mm", "Pattern height, 0.5-50"),
                opt(
                    "spacing",
                    "mm",
                    "Cross size, stippling / net / flexible spiral spacing, motif gaps, 0-20 (0.5 minimum except motif fill)",
                ),
                opt(
                    "runPitch",
                    "mm",
                    "Running stitch length of line-based fills, 1-10",
                ),
                opt(
                    "frequency",
                    "percent",
                    "Fill, piping, programmable fill: needle shift per row, % of a stitch, 0-50 (fill default 33)",
                ),
                opt(
                    "rowOffset",
                    "percent",
                    "Shift of every other pattern row, 0-100",
                ),
                opt(
                    "columnOffset",
                    "percent",
                    "Programmable fill: shift of every other tile column, 0-100",
                ),
                opt(
                    "startInside",
                    "bool",
                    "Concentric circle / spirals: start at the centre",
                ),
                opt("counterclockwise", "bool", "Spirals turn counter-clockwise"),
                opt(
                    "triple",
                    "bool",
                    "Cross stitch: sew every diagonal three times",
                ),
                opt(
                    "randomShift",
                    "percent",
                    "Decorative fill: random shift of needle points, 0-50",
                ),
                opt(
                    "baseSewing",
                    "bool",
                    "Programmable fill: sew a plain fill underneath",
                ),
            ],
            |s, p| {
                let idx = indexes(p, &s.design)?;
                let kind = match p
                    .get("type")
                    .map(|_| p.string("type", None))
                    .transpose()?
                    .as_deref()
                {
                    None => None,
                    Some("none") => Some(None),
                    Some(t) => Some(Some(RegionType::from_id(t).ok_or_else(|| {
                        p.invalid("type", "is not a region sew type (see the parameter list)")
                    })?)),
                };
                let color = thread(p, "color")?;
                let user_patterns = s.design.patterns.clone();
                let hole_inners: Vec<u64> = s
                    .design
                    .objects
                    .iter()
                    .filter(|o| o.hole_sewing)
                    .filter_map(|o| o.hole_inner)
                    .collect();
                for i in idx {
                    let o = &mut s.design.objects[i];
                    if !o.is_outline() || !o.has_closed_contour() {
                        return Err(p.failed(format!("object {} has no closed outline", o.id)));
                    }
                    if kind == Some(None) {
                        o.region = None;
                        continue;
                    }
                    // A new region starts with the requested type's defaults.
                    let mut r = o.region.clone().unwrap_or_else(|| {
                        RegionSewing::new(
                            kind.flatten().unwrap_or(RegionType::Fill),
                            default_fill(),
                        )
                    });
                    if let Some(Some(k)) = kind
                        && r.kind != k
                    {
                        // The direction and the start and end points stay (as in PE-DESIGN).
                        let (angle, entry, exit) = (r.angle, r.entry, r.exit);
                        r = RegionSewing::new(k, r.thread);
                        (r.angle, r.entry, r.exit) = (angle, entry, exit);
                    }
                    if let Some(c) = &color {
                        r.thread = c.clone();
                    }
                    r.angle = p.opt("angle", r.angle, -360.0, 360.0)?;
                    let (lo, hi) = r.kind.density_range();
                    r.density = p.opt("density", r.density, lo, hi)?;
                    r.stitch_length = p.opt("stitchLength", r.stitch_length, 1.0, 10.0)?;
                    r.pull_compensation =
                        p.opt("pullCompensation", r.pull_compensation, 0.0, 2.0)?;
                    r.underlay = p.bool("underlay", Some(r.underlay))?;
                    if let Some(u) = choice(
                        p,
                        "underlayStyle",
                        &[
                            ("edge", UnderlayStyle::Edge),
                            ("zigzag", UnderlayStyle::Zigzag),
                            ("edgeZigzag", UnderlayStyle::EdgeZigzag),
                            ("double", UnderlayStyle::Double),
                            ("edgeFill", UnderlayStyle::EdgeFill),
                        ],
                    )? {
                        r.underlay_style = u;
                    }
                    if p.has("underlaySpacing") {
                        r.underlay_spacing = Some(p.req("underlaySpacing", 1.0, 10.0)?);
                    }
                    r.auto_direction = p.bool("autoDirection", Some(r.auto_direction))?;
                    if p.has("gradation") {
                        let g = p.req("gradation", 5.0, 100.0)?;
                        r.gradation = (g < 100.0).then(|| Gradation::continuous(g));
                    }
                    if p.has("gradationPattern") {
                        r.gradation = p.bool("gradationPattern", None)?.then(|| {
                            r.gradation
                                .filter(|g| g.steps.is_some())
                                .unwrap_or(Gradation::PE_DEFAULT)
                        });
                    }
                    if ["gradationMax", "gradationMin", "gradationSteps"]
                        .iter()
                        .any(|k| p.has(k))
                    {
                        let mut g = r
                            .gradation
                            .filter(|g| g.steps.is_some())
                            .unwrap_or(Gradation::PE_DEFAULT);
                        g.max = p.opt("gradationMax", g.max, 5.0, 100.0)?;
                        g.min = p.opt("gradationMin", g.min, 5.0, 100.0)?;
                        let steps = g.steps.unwrap_or(9) as f64;
                        g.steps = Some(p.opt("gradationSteps", steps, 2.0, 20.0)?.round() as u32);
                        r.gradation = Some(g);
                    }
                    let side = choice(
                        p,
                        "featherSide",
                        &[
                            ("start", FeatherSide::Start),
                            ("end", FeatherSide::End),
                            ("both", FeatherSide::Both),
                        ],
                    )?;
                    if p.has("featherWidth") || side.is_some() {
                        if o.hole_sewing || hole_inners.contains(&o.id) {
                            return Err(p.failed(format!(
                                "object {} uses hole sewing: feathered edges are unavailable",
                                o.id
                            )));
                        }
                        // PE-DESIGN 11.50's default width is 3.0 mm.
                        let current = r.feather.unwrap_or(Feather {
                            side: FeatherSide::Start,
                            width: 3.0,
                        });
                        let width = p.opt("featherWidth", current.width, 0.0, 10.0)?;
                        r.feather = (width > 0.0).then_some(Feather {
                            side: side.unwrap_or(current.side),
                            width,
                        });
                    }
                    if p.has("splitLength") {
                        let l = p.req("splitLength", 0.0, 12.0)?;
                        if l > 0.0 && l < 2.0 {
                            return Err(p.invalid("splitLength", "must be 0 or 2...12"));
                        }
                        r.split_length = (l > 0.0).then_some(l);
                    }
                    if p.has("travel") {
                        r.running_stitch_path = if p.bool("travel", None)? {
                            r.running_stitch_path.or(Some(RunningStitchPath::Inside))
                        } else {
                            None
                        };
                    }
                    if p.has("runningStitchPath") {
                        let id = p.string("runningStitchPath", None)?;
                        let path = RunningStitchPath::from_id(&id).ok_or_else(|| {
                            p.invalid(
                                "runningStitchPath",
                                "must be inside, alongOutline or onOutline",
                            )
                        })?;
                        let unavailable = if o.kind == ObjectKind::Text {
                            Some("is not available for text")
                        } else if !matches!(r.kind, RegionType::Fill | RegionType::Satin) {
                            Some("is only for fill and satin")
                        } else if r.auto_direction {
                            Some("is not available with Auto direction")
                        } else if r.feather.is_some() {
                            Some("is not available with a feathered edge")
                        } else if path == RunningStitchPath::Inside && r.gradation.is_some() {
                            Some("inside is not available with gradation")
                        } else {
                            None
                        };
                        if let Some(reason) = unavailable {
                            return Err(p.invalid("runningStitchPath", reason));
                        }
                        r.running_stitch_path = Some(path);
                    }
                    if p.has("pattern") {
                        let id = p.string("pattern", None)?;
                        if !region_pattern_exists(r.kind, &id, &user_patterns) {
                            return Err(p.invalid(
                                "pattern",
                                format!(
                                    "'{id}' is not a {} pattern; see patterns.list",
                                    r.kind.id()
                                ),
                            ));
                        }
                        r.pattern = Some(id);
                    }
                    if p.has("patternWidth") {
                        r.pattern_width = Some(p.req("patternWidth", 0.5, 50.0)?);
                    }
                    if p.has("patternHeight") {
                        r.pattern_height = Some(p.req("patternHeight", 0.5, 50.0)?);
                    }
                    if p.has("spacing") {
                        let lo = if r.kind == RegionType::MotifFill {
                            0.0
                        } else {
                            0.5
                        };
                        r.spacing = Some(p.req("spacing", lo, 20.0)?);
                    }
                    if p.has("runPitch") {
                        r.run_pitch = Some(p.req("runPitch", 1.0, 10.0)?);
                    }
                    if p.has("frequency") {
                        r.frequency = Some(p.req("frequency", 0.0, 50.0)?);
                    }
                    if p.has("rowOffset") {
                        r.row_offset = Some(p.req("rowOffset", 0.0, 100.0)?);
                    }
                    if p.has("columnOffset") {
                        r.column_offset = Some(p.req("columnOffset", 0.0, 100.0)?);
                    }
                    r.start_inside = p.bool("startInside", Some(r.start_inside))?;
                    r.counterclockwise = p.bool("counterclockwise", Some(r.counterclockwise))?;
                    r.triple = p.bool("triple", Some(r.triple))?;
                    if p.has("randomShift") {
                        r.random_shift = Some(p.req("randomShift", 0.0, 50.0)?);
                    }
                    r.base_sewing = p.bool("baseSewing", Some(r.base_sewing))?;
                    // The guide's exclusions, for an option this call turns on or changes.
                    let text = o.kind == ObjectKind::Text;
                    let gradation_keys = [
                        "gradation",
                        "gradationPattern",
                        "gradationMax",
                        "gradationMin",
                        "gradationSteps",
                    ];
                    if r.gradation.is_some() && gradation_keys.iter().any(|k| p.has(k)) {
                        let why = if text {
                            Some("is not available for text")
                        } else {
                            r.gradation_unavailable()
                        };
                        if let Some(why) = why {
                            return Err(p.invalid("gradation", why));
                        }
                    }
                    if r.feather.is_some() && (p.has("featherWidth") || p.has("featherSide")) {
                        let why = if text {
                            Some("is not available for text")
                        } else {
                            r.feather_unavailable()
                        };
                        if let Some(why) = why {
                            return Err(p.invalid("featherWidth", why));
                        }
                    }
                    o.region = Some(r);
                }
                Ok(Value::Null)
            },
        ),
        spec(
            "object.setLine",
            "Set line sew type and attributes (type 'none' removes)",
            Effect::Edit,
            vec![
                IDS,
                opt(
                    "type",
                    "running|triple|zigzag|motif|stem|candlewick|eStitch|vStitch|chain|cutting|none",
                    "Line sew type",
                ),
                opt("color", "color", "Thread"),
                opt(
                    "pitch",
                    "mm",
                    "Running/triple stitch length, run pitch, 1-10",
                ),
                opt(
                    "runTimes",
                    "int 1-6",
                    "Running and E/V passes (1-5); chain passes (1-6)",
                ),
                opt(
                    "width",
                    "mm",
                    "Zigzag, motif, stem width; candlewick, chain size; E/V stroke, 0.5-10",
                ),
                opt(
                    "density",
                    "stitches/mm",
                    "Zigzag density (lines/mm); candlewick spokes ÷ 2, 0.1-14",
                ),
                opt(
                    "motif",
                    "string",
                    "Motif id from patterns.list (motif lines)",
                ),
                opt("height", "mm", "Motif height, 0.5-20"),
                opt(
                    "spacing",
                    "mm",
                    "Gap between motifs (0-20), stem stitches, knots, E/V strokes (0.5-20)",
                ),
                opt(
                    "angle",
                    "degrees",
                    "Stem stitch angle to the line, counter-clockwise, 45-135",
                ),
                opt("single", "bool", "Stem: single instead of triple stitches"),
                opt(
                    "strokeTimes",
                    "int 1-3",
                    "E stitch: times each stroke is sewn",
                ),
                opt(
                    "arrange",
                    "normal|mirror|alternate",
                    "Side of motifs, E/V strokes, chain links",
                ),
                opt("chainShape", "triangle|diamond", "Chain link shape"),
                opt("leaveUncut", "bool", "Cutting: leave short uncut bridges"),
                opt("halfStitch", "bool", "Zigzag: thin the inner side of bends"),
                opt(
                    "underSewing",
                    "bool",
                    "Zigzag: running stitch a quarter of the width to each side first",
                ),
            ],
            |s, p| {
                let idx = indexes(p, &s.design)?;
                let kind = match p
                    .get("type")
                    .map(|_| p.string("type", None))
                    .transpose()?
                    .as_deref()
                {
                    None => None,
                    Some("none") => Some(None),
                    Some(t) => Some(Some(LineType::from_id(t).ok_or_else(|| {
                        p.invalid("type", "is not a line sew type (see the parameter list)")
                    })?)),
                };
                let color = thread(p, "color")?;
                let motif = match p.get("motif") {
                    None => None,
                    Some(_) => {
                        let id = p.string("motif", None)?;
                        s.design
                            .find_pattern(PatternKind::Motif, &id)
                            .ok_or_else(|| {
                                p.invalid(
                                    "motif",
                                    format!("'{id}' is not a motif; see patterns.list"),
                                )
                            })?;
                        Some(id)
                    }
                };
                let arrange = match p
                    .get("arrange")
                    .map(|_| p.string("arrange", None))
                    .transpose()?
                    .as_deref()
                {
                    None => None,
                    Some("normal") => Some(Arrange::Normal),
                    Some("mirror") => Some(Arrange::Mirror),
                    Some("alternate") => Some(Arrange::Alternate),
                    Some(_) => {
                        return Err(p.invalid("arrange", "must be normal, mirror or alternate"));
                    }
                };
                let chain_shape = match p
                    .get("chainShape")
                    .map(|_| p.string("chainShape", None))
                    .transpose()?
                    .as_deref()
                {
                    None => None,
                    Some("triangle") => Some(ChainShape::Triangle),
                    Some("diamond") => Some(ChainShape::Diamond),
                    Some(_) => return Err(p.invalid("chainShape", "must be triangle or diamond")),
                };
                for i in idx {
                    let o = &mut s.design.objects[i];
                    if !o.is_outline() {
                        return Err(p.failed(format!("object {} is a stitch pattern", o.id)));
                    }
                    if kind == Some(None) {
                        o.line = None;
                        continue;
                    }
                    let mut l = o
                        .line
                        .clone()
                        .unwrap_or_else(|| LineSewing::new(LineType::Running, default_line()));
                    // Switching the type gives the new type's own attributes, as PE-DESIGN does;
                    // the colour and the entry point stay.
                    if let Some(Some(k)) = kind
                        && k != l.kind
                    {
                        l = LineSewing {
                            entry: l.entry,
                            ..LineSewing::new(k, l.thread.clone())
                        };
                    }
                    if let Some(c) = &color {
                        l.thread = c.clone();
                    }
                    l.pitch = p.opt("pitch", l.pitch, 1.0, 10.0)?;
                    // Guide: run times 1-5, Chain Time 1-6.
                    let max_times = if l.kind == LineType::Chain { 6 } else { 5 };
                    l.run_times = p.int(
                        "runTimes",
                        Some(i64::from(l.run_times.min(max_times))),
                        1,
                        max_times.into(),
                    )? as u32;
                    l.width = p.opt("width", l.width, 0.5, 10.0)?;
                    // PE-DESIGN's zigzag density runs from 0.1 to 14 lines/mm.
                    l.density = p.opt("density", l.density, 0.1, 14.0)?;
                    if motif.is_some() {
                        l.motif.clone_from(&motif);
                    }
                    if p.has("height") {
                        l.height = Some(p.req("height", 0.5, 20.0)?);
                    }
                    if p.has("spacing") {
                        let lo = if l.kind == LineType::Motif { 0.0 } else { 0.5 };
                        l.spacing = Some(p.req("spacing", lo, 20.0)?);
                    }
                    if p.has("angle") {
                        l.angle = Some(p.req("angle", 45.0, 135.0)?);
                    }
                    l.single = p.bool("single", Some(l.single))?;
                    if p.has("strokeTimes") {
                        l.stroke_times = Some(p.int("strokeTimes", None, 1, 3)? as u32);
                    }
                    if let Some(a) = arrange {
                        l.arrange = a;
                    }
                    if let Some(c) = chain_shape {
                        l.chain_shape = c;
                    }
                    l.leave_uncut = p.bool("leaveUncut", Some(l.leave_uncut))?;
                    l.half_stitch = p.bool("halfStitch", Some(l.half_stitch))?;
                    l.underlay = p.bool("underSewing", Some(l.underlay))?;
                    o.line = Some(l);
                }
                Ok(Value::Null)
            },
        ),
        spec(
            "object.setHoleSewing",
            "Hole sewing (PE-DESIGN): of two regions where one fully encloses the other, the outer one is sewn around the inner shape. With one id the region it encloses is found; off clears the pair",
            Effect::Edit,
            vec![IDS, req("enabled", "bool", "Hole sewing on/off")],
            |s, p| {
                let on = p.bool("enabled", None)?;
                let mut idx = indexes(p, &s.design)?;
                idx.sort_unstable();
                idx.dedup();
                if !on {
                    let ids: Vec<u64> = idx.iter().map(|&i| s.design.objects[i].id).collect();
                    for o in &mut s.design.objects {
                        if ids.contains(&o.id) || o.hole_inner.is_some_and(|h| ids.contains(&h)) {
                            o.hole_sewing = false;
                            o.hole_inner = None;
                        }
                    }
                    return Ok(Value::Null);
                }
                let (outer, inner) = hole_pair(s, p, &idx)?;
                let inner = s.design.objects[inner].id;
                let o = &mut s.design.objects[outer];
                o.hole_sewing = true;
                o.hole_inner = Some(inner);
                Ok(json!({ "outer": o.id, "inner": inner }))
            },
        ),
        spec(
            "object.setVisible",
            "Show or hide objects (hidden objects are not sewn)",
            Effect::Edit,
            vec![IDS, req("visible", "bool", "Visibility")],
            |s, p| {
                let v = p.bool("visible", None)?;
                for i in indexes(p, &s.design)? {
                    s.design.objects[i].visible = v;
                }
                Ok(Value::Null)
            },
        ),
    ]
}

// MARK: transforms

fn transform(
    s: &mut EngineState,
    p: &Params,
    make: impl Fn(Rect) -> Result<Transform, CommandError>,
) -> CommandResult {
    let idx = indexes(p, &s.design)?;
    let Some(bbox) = Rect::enclosing(idx.iter().flat_map(|&i| s.design.objects[i].points())) else {
        return Ok(Value::Null);
    };
    let t = make(bbox)?;
    for i in idx {
        s.design.objects[i].apply(&t);
    }
    Ok(Value::Null)
}

const ORIGIN_X: ParamDoc = opt(
    "originX",
    "mm",
    "Fixed point x (default: centre of the objects)",
);
const ORIGIN_Y: ParamDoc = opt(
    "originY",
    "mm",
    "Fixed point y (default: centre of the objects)",
);

/// `originX`/`originY` in mm when given (both required if either is), else the centre of `b`.
fn origin(p: &Params, b: Rect) -> Result<openstitch_design::Vec2, CommandError> {
    if !p.has("originX") && !p.has("originY") {
        return Ok(b.center());
    }
    Ok(v2(
        p.req("originX", -5000.0, 5000.0)? * MM,
        p.req("originY", -5000.0, 5000.0)? * MM,
    ))
}

fn transform_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "object.move",
            "Move objects",
            Effect::Edit,
            vec![IDS, opt("dx", "mm", "Right"), opt("dy", "mm", "Down")],
            |s, p| {
                let (dx, dy) = (
                    p.opt("dx", 0.0, -5000.0, 5000.0)?,
                    p.opt("dy", 0.0, -5000.0, 5000.0)?,
                );
                transform(s, p, |_| Ok(Transform::translation(dx * MM, dy * MM)))
            },
        ),
        spec(
            "object.scale",
            "Scale objects about their common centre (or originX/originY)",
            Effect::Edit,
            vec![
                IDS,
                req("sx", "factor", "Horizontal scale"),
                opt("sy", "factor", "Vertical scale (default sx)"),
                ORIGIN_X,
                ORIGIN_Y,
            ],
            |s, p| {
                let sx = p.req("sx", 0.01, 100.0)?;
                let sy = p.opt("sy", sx, 0.01, 100.0)?;
                transform(s, p, |b| Ok(Transform::scaling(sx, sy, origin(p, b)?)))
            },
        ),
        spec(
            "object.rotate",
            "Rotate objects clockwise about their common centre (or originX/originY)",
            Effect::Edit,
            vec![IDS, req("degrees", "degrees", "Angle"), ORIGIN_X, ORIGIN_Y],
            |s, p| {
                let deg = p.req("degrees", -3600.0, 3600.0)?;
                transform(s, p, |b| Ok(Transform::rotation(deg, origin(p, b)?)))
            },
        ),
        spec(
            "object.flip",
            "Mirror objects",
            Effect::Edit,
            vec![IDS, req("axis", "horizontal|vertical", "Flip direction")],
            |s, p| {
                let (sx, sy) = match p.string("axis", None)?.as_str() {
                    "horizontal" => (-1.0, 1.0),
                    "vertical" => (1.0, -1.0),
                    _ => return Err(p.invalid("axis", "must be horizontal or vertical")),
                };
                transform(s, p, |b| Ok(Transform::scaling(sx, sy, b.center())))
            },
        ),
        spec(
            "object.center",
            "Centre objects on the Design Page",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let page = s.design.page.center();
                transform(s, p, |b| {
                    Ok(Transform::translation(
                        page.x - b.center().x,
                        page.y - b.center().y,
                    ))
                })
            },
        ),
        spec(
            "object.duplicate",
            "Duplicate objects, offset by dx/dy",
            Effect::Edit,
            vec![
                IDS,
                opt("dx", "mm", "Offset (default 5)"),
                opt("dy", "mm", "Offset (default 5)"),
            ],
            |s, p| {
                let idx = indexes(p, &s.design)?;
                let t = Transform::translation(
                    p.opt("dx", 5.0, -5000.0, 5000.0)? * MM,
                    p.opt("dy", 5.0, -5000.0, 5000.0)? * MM,
                );
                Ok(json!({ "ids": crate::drawing::copy_set(&mut s.design, &idx, &t) }))
            },
        ),
        spec(
            "object.delete",
            "Delete objects",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let ids: Vec<u64> = indexes(p, &s.design)?
                    .iter()
                    .map(|&i| s.design.objects[i].id)
                    .collect();
                s.design.objects.retain(|o| !ids.contains(&o.id));
                Ok(Value::Null)
            },
        ),
        spec(
            "object.reorder",
            "Move an object to a position in the sewing order",
            Effect::Edit,
            vec![
                req("id", "int", "Object"),
                req("index", "int", "New 0-based position"),
            ],
            |s, p| {
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let from = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                let to = p.int(
                    "index",
                    None,
                    0,
                    s.design.objects.len().saturating_sub(1) as i64,
                )? as usize;
                let o = s.design.objects.remove(from);
                s.design.objects.insert(to, o);
                Ok(Value::Null)
            },
        ),
    ]
}

// MARK: lettering

/// Longest string a text object may hold, in characters.
pub const MAX_TEXT_CHARS: usize = 2000;
/// Largest laid-out text, mm (the largest Design Page).
const MAX_TEXT_EXTENT: f64 = 2000.0;

fn text_docs(text_required: bool) -> Vec<ParamDoc> {
    let text = if text_required {
        req("text", "string", "Text; \\n starts a new line")
    } else {
        opt("text", "string", "Text; \\n starts a new line")
    };
    vec![
        text,
        opt(
            "font",
            "string",
            "Font id or family from fonts.list (default lato-bold)",
        ),
        opt("size", "mm", "Capital letter height, 1-300 (default 10)"),
        opt(
            "letterSpacing",
            "mm",
            "Extra space between letters, -100...100",
        ),
        opt(
            "lineSpacingPercent",
            "%",
            "Line spacing, % of the font's line height, 0...500 (default 100)",
        ),
        opt(
            "lineSpacing",
            "mm",
            "Extra space between lines on top of the %, -100...100 (default 0)",
        ),
        opt(
            "align",
            "left|center|right",
            "Alignment about x, or along the path (top, middle, bottom for vertical text)",
        ),
        opt(
            "kerning",
            "bool",
            "Apply the font's kerning (TrueType/OpenType fonts; default off)",
        ),
        opt(
            "direction",
            "horizontal|vertical",
            "Vertical: characters top to bottom, lines as columns right to left (not on a path)",
        ),
    ]
}

/// Applies the optional text attributes of `p` to `spec`.
fn text_params(
    p: &Params,
    spec: &mut TextSpec,
    fonts: &[openstitch_design::DesignFont],
) -> Result<(), CommandError> {
    if p.has("text") {
        let t = p.string("text", None)?;
        let n = t.chars().count();
        if n == 0 || n > MAX_TEXT_CHARS {
            return Err(p.invalid("text", format!("must be 1-{MAX_TEXT_CHARS} characters")));
        }
        spec.text = t;
        spec.trim_char_edits();
    }
    if p.has("font") {
        let f = p.string("font", None)?;
        let info = text::resolve(&f, fonts)
            .ok_or_else(|| p.invalid("font", format!("'{f}' is not a font; see fonts.list")))?;
        spec.font = info.id().into();
    }
    spec.size = p.opt("size", spec.size, 1.0, 300.0)?;
    spec.letter_spacing = p.opt("letterSpacing", spec.letter_spacing, -100.0, 100.0)?;
    spec.line_spacing_percent =
        p.opt("lineSpacingPercent", spec.line_spacing_percent, 0.0, 500.0)?;
    spec.line_spacing = p.opt("lineSpacing", spec.line_spacing, -100.0, 100.0)?;
    if p.has("align") {
        spec.align = match p.string("align", None)?.as_str() {
            "left" => TextAlign::Left,
            "center" | "centre" => TextAlign::Center,
            "right" => TextAlign::Right,
            _ => return Err(p.invalid("align", "must be left, center or right")),
        };
    }
    spec.kerning = p.bool("kerning", Some(spec.kerning))?;
    if p.has("direction") {
        spec.direction = match p.string("direction", None)?.as_str() {
            "horizontal" => TextDirection::Horizontal,
            "vertical" => TextDirection::Vertical,
            _ => return Err(p.invalid("direction", "must be horizontal or vertical")),
        };
    }
    Ok(())
}

/// `text.setCharacter`: edits (or resets) characters `index..index + count` of a text.
fn set_characters(s: &mut EngineState, p: &Params) -> CommandResult {
    let id = p.int("id", None, 0, i64::MAX)? as u64;
    let i = text_index(s, p, id)?;
    let mut spec = s.design.objects[i].text.clone().expect("text object");
    let n = spec.text.chars().count();
    if n == 0 {
        return Err(p.failed(format!("object {id} has no characters")));
    }
    let index = p.int("index", None, 0, n as i64 - 1)? as usize;
    let count = p.int("count", Some(1), 1, (n - index) as i64)? as usize;
    let reset = p.bool("reset", Some(false))?;
    for k in index..index + count {
        let mut e = match spec.char_edit(k) {
            Some(e) if !reset => e.clone(),
            _ => openstitch_design::CharEdit {
                index: k,
                ..Default::default()
            },
        };
        e.x = p.opt("x", e.x, -500.0, 500.0)?;
        e.y = p.opt("y", e.y, -500.0, 500.0)?;
        e.rotation = p.opt("rotation", e.rotation, -360.0, 360.0)?;
        if p.is_null("size") {
            e.size = None;
        } else if p.has("size") {
            e.size = Some(p.opt("size", spec.size, 1.0, 300.0)?);
        }
        spec.set_char_edit(e);
    }
    s.design.objects[i].text = Some(spec);
    let mut r = relayout(s, p, i)?;
    r["characters"] = json!(
        s.design.objects[i]
            .text
            .as_ref()
            .map_or(0, |t| t.characters.len())
    );
    Ok(r)
}

/// The `path` parameter: a shape object (not `text_id` itself) whose first contour text follows.
/// `None` when absent or null.
fn path_param(s: &EngineState, p: &Params, text_id: u64) -> Result<Option<u64>, CommandError> {
    if !p.has("path") {
        return Ok(None);
    }
    let id = p.int("path", None, 0, i64::MAX)? as u64;
    if s.design.index_of(id).is_none() {
        return Err(p.not_found(format!("path object {id}")));
    }
    match s.design.path_contour(id) {
        Some(c) if id != text_id && c.points.len() >= 2 => {}
        _ => return Err(p.invalid("path", "must be the id of a shape object with an outline")),
    }
    // As in PE-DESIGN, only one text can be fitted to an outline.
    if let Some(other) = s
        .design
        .objects
        .iter()
        .find(|o| o.id != text_id && o.text.as_ref().is_some_and(|t| t.path == Some(id)))
    {
        return Err(p.failed(format!(
            "text {} is already fitted to object {id}; only one text fits an outline",
            other.id
        )));
    }
    Ok(Some(id))
}

/// Lays out object `i` again and checks the result is sewable and of sane size.
pub(crate) fn relayout(s: &mut EngineState, p: &Params, i: usize) -> CommandResult {
    let layout: Layout = s
        .design
        .layout_text(i)
        .map_err(|e| p.failed(e.to_string()))?;
    let o = &s.design.objects[i];
    let missing: Vec<String> = layout.missing.iter().map(char::to_string).collect();
    if layout.glyphs.is_empty() {
        return Err(if missing.is_empty() {
            p.failed(format!("text of object {} has no visible characters", o.id))
        } else {
            p.failed(format!(
                "the font has no glyphs for {}",
                missing.concat().chars().take(20).collect::<String>()
            ))
        });
    }
    let b = o.bounds().unwrap_or(Rect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    });
    if !(b.width() <= MAX_TEXT_EXTENT * MM && b.height() <= MAX_TEXT_EXTENT * MM) {
        return Err(p.failed(format!(
            "text would be {:.0} x {:.0} mm; the limit is {MAX_TEXT_EXTENT} mm",
            b.width() / MM,
            b.height() / MM
        )));
    }
    Ok(json!({
        "id": o.id, "glyphs": layout.glyphs.len(), "missing": missing,
        "shaping": layout.shaping,
    }))
}

pub(crate) fn text_index(s: &EngineState, p: &Params, id: u64) -> Result<usize, CommandError> {
    let i = s
        .design
        .index_of(id)
        .ok_or_else(|| p.not_found(format!("object {id}")))?;
    if s.design.objects[i].kind != ObjectKind::Text || s.design.objects[i].text.is_none() {
        return Err(p.failed(format!("object {id} is not text")));
    }
    Ok(i)
}

fn text_commands() -> Vec<CommandSpec> {
    let mut add_docs = text_docs(true);
    add_docs.extend([
        opt(
            "x",
            "mm",
            "Alignment point of the first baseline (default page centre)",
        ),
        opt("y", "mm", "First baseline (default page centre)"),
        opt(
            "path",
            "int",
            "Shape object whose outline the text follows (x and y are then ignored)",
        ),
    ]);
    let mut set_docs = vec![IDS];
    set_docs.extend(text_docs(false));
    vec![
        spec(
            "patterns.list",
            "Stitch patterns: motifs (motif line, motif fill), programmable fill tiles, decorative fill rows and net patterns",
            Effect::Read,
            vec![],
            |s, _| {
                let kind_id = |k: PatternKind| match k {
                    PatternKind::Motif => "motif",
                    PatternKind::Fill => "fill",
                    PatternKind::Decorative => "decorative",
                };
                let mut out: Vec<Value> = patterns::library()
                    .iter()
                    .map(|pt| json!({ "kind": kind_id(pt.kind), "id": pt.id, "name": pt.name }))
                    .collect();
                out.extend(s.design.patterns.iter().map(|pt| {
                    json!({ "kind": kind_id(pt.kind), "id": pt.id, "name": pt.name, "user": true })
                }));
                out.extend(patterns::NET_PATTERNS.iter().enumerate().map(|(i, n)| {
                    json!({
                        "kind": "net", "id": n.id, "name": n.name,
                        "zigzagNet": i < patterns::ZIGZAG_NET_PATTERNS,
                    })
                }));
                Ok(Value::Array(out))
            },
        ),
        spec(
            "fonts.list",
            "Fonts available for lettering: bundled fonts, then fonts loaded into the design",
            Effect::Read,
            vec![],
            |s, _| Ok(crate::fonts::list(s)),
        ),
        spec(
            "text.add",
            "Add lettering (a text object sewn like a shape, glyph by glyph)",
            Effect::Edit,
            shape_docs(add_docs),
            |s, p| {
                let mut spec = TextSpec::new(p.string("text", None)?, text::DEFAULT_FONT, 10.0);
                spec.align = TextAlign::Center;
                text_params(p, &mut spec, &s.design.fonts)?;
                let page = s.design.page.center();
                let x = p.opt("x", page.x / MM, -5000.0, 5000.0)?;
                let y = p.opt("y", page.y / MM, -5000.0, 5000.0)?;
                spec.transform = Transform::translation(x * MM, y * MM);
                let (region, line) = sewing(p, true)?;
                let mut object = DesignObject::shape(Vec::new(), region, line);
                object.kind = ObjectKind::Text;
                object.name = object_name(p)?;
                let path = path_param(s, p, s.design.next_object_id)?;
                if let Some(path) = path {
                    spec.path = Some(path);
                    spec.transform = Transform::IDENTITY;
                }
                object.text = Some(spec);
                s.design.add(object);
                relayout(s, p, s.design.objects.len() - 1)
            },
        ),
        spec(
            "object.setText",
            "Change the text, font, size, spacing or alignment of text objects",
            Effect::Edit,
            set_docs,
            |s, p| {
                let (mut missing, mut shaping) = (Vec::new(), Vec::new());
                for id in p.ids()? {
                    let i = text_index(s, p, id)?;
                    let mut spec = s.design.objects[i].text.clone().expect("text object");
                    text_params(p, &mut spec, &s.design.fonts)?;
                    s.design.objects[i].text = Some(spec);
                    let r = relayout(s, p, i)?;
                    for (key, all) in [("missing", &mut missing), ("shaping", &mut shaping)] {
                        for m in r[key].as_array().into_iter().flatten() {
                            if !all.contains(m) {
                                all.push(m.clone());
                            }
                        }
                    }
                }
                Ok(json!({ "missing": missing, "shaping": shaping }))
            },
        ),
        spec(
            "text.setCharacter",
            "Edit single characters of a text: offset, rotation and size per character (reset clears them)",
            Effect::Edit,
            vec![
                req("id", "int", "Text object"),
                req(
                    "index",
                    "int",
                    "First character, counted in Unicode characters from 0 (line breaks count)",
                ),
                opt("count", "int", "Characters to edit from index (default 1)"),
                opt(
                    "x",
                    "mm",
                    "Offset to the right, -500...500 (along the path for path text)",
                ),
                opt("y", "mm", "Offset downwards, -500...500"),
                opt(
                    "rotation",
                    "degrees",
                    "Clockwise turn about the character's middle, -360...360",
                ),
                opt(
                    "size",
                    "mm | null",
                    "Capital height of these characters, 1-300; null returns to the text's size",
                ),
                opt(
                    "reset",
                    "bool",
                    "Clear the characters' edits first (alone: back to normal)",
                ),
            ],
            set_characters,
        ),
        spec(
            "text.setPath",
            "Fit text to a shape's outline (text on a path), or release it with path null. One text per outline",
            Effect::Edit,
            vec![
                req("id", "int", "Text object"),
                opt(
                    "path",
                    "int | null",
                    "Shape object to follow; null or omitted releases",
                ),
                opt(
                    "align",
                    "string",
                    "Along the outline from its start: left, center (default when first fitted), right, spaced (evenly round the whole outline) or stretch (letters scaled to fill it)",
                ),
                opt(
                    "valign",
                    "string",
                    "Across the outline: bottom (baseline on it, default), middle or top",
                ),
                opt(
                    "upright",
                    "bool",
                    "Letters stay upright instead of turning with the outline (default false)",
                ),
                opt(
                    "otherSide",
                    "bool",
                    "Run on the other side of the outline, following it the other way (default false)",
                ),
            ],
            |s, p| {
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = text_index(s, p, id)?;
                match path_param(s, p, id)? {
                    Some(path) => {
                        let old = s.design.objects[i].text.clone().expect("text object");
                        // A text fitted for the first time starts from the dialog's defaults:
                        // centred, above the line, turned along it, this side.
                        let (fit, align) = if old.path.is_some() {
                            (old.fit, old.align)
                        } else {
                            (openstitch_design::PathFit::default(), TextAlign::Center)
                        };
                        let (align, spread) = match p.has("align").then(|| p.string("align", None))
                        {
                            None => (align, fit.spread),
                            Some(a) => match a?.as_str() {
                                "spaced" => (align, openstitch_design::Spread::Even),
                                "stretch" => (align, openstitch_design::Spread::Fill),
                                _ => (
                                    crate::lettering::align_param(p, align)?,
                                    openstitch_design::Spread::None,
                                ),
                            },
                        };
                        let fit = openstitch_design::PathFit {
                            valign: crate::lettering::valign_param(p, fit.valign)?,
                            upright: p.bool("upright", Some(fit.upright))?,
                            spread,
                            other_side: p.bool("otherSide", Some(fit.other_side))?,
                        };
                        let t = s.design.objects[i].text.as_mut().expect("text object");
                        t.path = Some(path);
                        t.fit = fit;
                        t.align = align;
                        t.transform = Transform::IDENTITY;
                    }
                    None => s.design.detach_text(i),
                }
                relayout(s, p, i)
            },
        ),
        spec(
            "text.toShapes",
            "Convert text objects into shape objects, one per glyph (outlines become editable)",
            Effect::Edit,
            vec![IDS],
            |s, p| {
                let mut idx = indexes(p, &s.design)?;
                idx.sort_unstable();
                idx.dedup();
                for &i in &idx {
                    text_index(s, p, s.design.objects[i].id)?;
                }
                let mut new_ids = Vec::new();
                for &i in idx.iter().rev() {
                    let o = s.design.objects.remove(i);
                    let spec = o.text.clone().expect("text object");
                    let path = spec.path.and_then(|id| s.design.path_contour(id));
                    let chars: Vec<char> = text::layout_in(&spec, path.as_ref(), &s.design.fonts)
                        .map(|l| l.outlined().map(|g| g.ch).collect())
                        .unwrap_or_default();
                    // Stitch glyphs (user-mapped text) become one stitch pattern object.
                    if let Some(block) = o.stitch_block.clone() {
                        let mut st = DesignObject::stitches(block);
                        st.visible = o.visible;
                        st.name = Some(format!(
                            "{} stitches",
                            o.name.clone().unwrap_or_else(|| format!("Text {}", o.id))
                        ));
                        st.id = s.design.next_object_id;
                        s.design.next_object_id += 1;
                        new_ids.push(st.id);
                        s.design.objects.insert(i, st);
                    }
                    if o.contours.is_empty() {
                        continue;
                    }
                    let groups = o.glyph_groups();
                    let base = o.name.clone().unwrap_or_else(|| format!("Text {}", o.id));
                    for (k, group) in groups.iter().enumerate() {
                        let mut shape =
                            DesignObject::shape(group.to_vec(), o.region.clone(), o.line.clone());
                        shape.visible = o.visible;
                        shape.hole_sewing = o.hole_sewing;
                        shape.hole_inner = o.hole_inner;
                        shape.name = Some(match chars.get(k) {
                            Some(c) if chars.len() == groups.len() => format!("{base} '{c}'"),
                            _ => format!("{base} {}", k + 1),
                        });
                        shape.id = s.design.next_object_id;
                        s.design.next_object_id += 1;
                        new_ids.push(shape.id);
                        s.design.objects.insert(i + k, shape);
                    }
                }
                let ids: Vec<u64> = s
                    .design
                    .objects
                    .iter()
                    .map(|o| o.id)
                    .filter(|id| new_ids.contains(id))
                    .collect();
                Ok(json!({ "ids": ids }))
            },
        ),
    ]
}

// MARK: files

/// `dir/name.ext` → `dir/name-N.ext` (split exports).
fn numbered(path: &str, n: usize) -> String {
    let p = std::path::Path::new(path);
    let stem = p
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let name = match p.extension() {
        Some(ext) => format!("{stem}-{n}.{}", ext.to_string_lossy()),
        None => format!("{stem}-{n}"),
    };
    p.with_file_name(name).display().to_string()
}

pub(crate) fn read_file(
    s: &EngineState,
    p: &Params,
    max: usize,
) -> Result<(std::path::PathBuf, Vec<u8>), CommandError> {
    let path = s.resolve(&p.string("path", None)?, Access::Read, &p.command)?;
    let meta = std::fs::metadata(&path)
        .map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
    if meta.len() > max as u64 {
        return Err(p.failed("file too large"));
    }
    let data =
        std::fs::read(&path).map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
    Ok((path, data))
}

fn file_commands() -> Vec<CommandSpec> {
    let mut v = vec![
        spec(
            "project.save",
            "Save the editable design (.osd)",
            Effect::File,
            vec![PATH],
            |s, p| {
                let path = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
                std::fs::write(&path, project::encode(&s.design)?)
                    .map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({ "path": path.display().to_string() }))
            },
        ),
        spec(
            "project.open",
            "Open an editable design (.osd), replacing the current one",
            Effect::Edit,
            vec![PATH],
            |s, p| {
                let (_, data) = read_file(s, p, project::MAX_SIZE)?;
                s.design = project::decode(&data)?;
                Ok(json!({ "name": s.design.name, "objects": s.design.objects.len() }))
            },
        ),
    ];
    v.extend(codec_commands());
    v
}

#[cfg(not(feature = "formats"))]
fn codec_commands() -> Vec<CommandSpec> {
    Vec::new()
}

#[cfg(feature = "formats")]
fn codec_commands() -> Vec<CommandSpec> {
    use openstitch_design::StitchBlock;
    use openstitch_formats::FileType;

    fn file_type(p: &Params, path: &std::path::Path) -> Result<FileType, CommandError> {
        let raw = if p.has("format") {
            p.string("format", None)?
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_string()
        };
        FileType::from_extension(&raw).ok_or_else(|| {
            let names: Vec<_> = FileType::ALL.iter().map(|t| t.extension()).collect();
            p.invalid("format", format!("unsupported; use {}", names.join(", ")))
        })
    }

    vec![
        spec(
            "stitches.import",
            "Import an embroidery file as a stitch pattern object",
            Effect::Edit,
            vec![
                PATH,
                opt("format", "pes|pec|dst|...", "Override the extension"),
                opt(
                    "adoptProperties",
                    "bool",
                    "Copy the file's Design Property fields into the design (default: only when the design has none)",
                ),
            ],
            |s, p| {
                let (path, data) = read_file(s, p, 64 << 20)?;
                let pattern = file_type(p, &path)?
                    .read(&data)
                    .map_err(|e| p.failed(e.to_string()))?;
                let found = openstitch_design::DesignProperties::from_metadata(&pattern.metadata);
                let adopt = p.bool("adoptProperties", Some(s.design.properties.is_empty()))?
                    && !found.is_empty();
                let mut object = DesignObject::stitches(StitchBlock::from(&pattern));
                object.name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                let id = s.design.add(object);
                if adopt {
                    s.design.properties = found.clone();
                }
                Ok(json!({ "id": id, "properties": found, "propertiesAdopted": adopt }))
            },
        ),
        spec(
            "design.export",
            "Generate stitches and write a machine file",
            Effect::File,
            vec![
                PATH,
                opt("format", "pes|pec|dst|...", "Override the extension"),
                opt(
                    "pesVersion",
                    "1t|1|6|6t",
                    "PES only: version 1 or 6, `t` = no objects (default 6: RGB threads, names, stitch group; 1t = machine only, smallest)",
                ),
                opt(
                    "split",
                    "bool",
                    "When the design exceeds the format's limits, write NAME-1.ext, NAME-2.ext… (default false: fail)",
                ),
            ],
            |s, p| {
                let raw_path = p.string("path", None)?;
                let path = s.resolve(&raw_path, Access::Write, &p.command)?;
                let split = p.bool("split", Some(false))?;
                let ty = file_type(p, &path)?;
                let pes_options = openstitch_formats::pes::WriteOptions::parse(&p.string(
                    "pesVersion",
                    Some(openstitch_formats::pes::WriteOptions::DEFAULT_NAME),
                )?)
                .ok_or_else(|| p.invalid("pesVersion", "use 1t, 1, 6 or 6t"))?;
                if !ty.can_write() {
                    let names: Vec<_> = FileType::writable().map(|t| t.extension()).collect();
                    return Err(p.invalid(
                        "format",
                        format!("{ty} is read only; use {}", names.join(", ")),
                    ));
                }
                let pattern = Compiler::for_design(&s.design).compile(&s.design);
                if pattern.count(Command::Stitch) == 0 {
                    return Err(p.failed("design has no stitches"));
                }
                let write = |part: &openstitch_core::StitchPattern| match ty {
                    FileType::Pes => openstitch_formats::pes::write_with(part, &pes_options),
                    _ => ty.write(part),
                };
                let parts = if split {
                    openstitch_formats::split::split_for(ty, &pattern)
                        .map_err(|e| p.failed(e.to_string()))?
                } else {
                    vec![pattern.clone()]
                };
                // Encode everything before writing anything, so a failure leaves no files.
                let mut files = Vec::with_capacity(parts.len());
                for (i, part) in parts.iter().enumerate() {
                    let bytes = write(part).map_err(|e| match e {
                        openstitch_formats::FormatError::ValueOutOfRange(_) if !split => {
                            p.failed(format!("{e}; pass split: true to write several files"))
                        }
                        e => p.failed(e.to_string()),
                    })?;
                    let target = if parts.len() == 1 {
                        path.clone()
                    } else {
                        s.resolve(&numbered(&raw_path, i + 1), Access::Write, &p.command)?
                    };
                    files.push((target, bytes));
                }
                for (target, bytes) in &files {
                    std::fs::write(target, bytes).map_err(|e| p.failed(e.to_string()))?;
                }
                let paths: Vec<String> =
                    files.iter().map(|(t, _)| t.display().to_string()).collect();
                Ok(json!({
                    "path": paths[0],
                    "paths": paths,
                    "stats": stats(&pattern),
                    "sewingArea": crate::machine::sewing_area(&s.design, &pattern),
                }))
            },
        ),
    ]
}
