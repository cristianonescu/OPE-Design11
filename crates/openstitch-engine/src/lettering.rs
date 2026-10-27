//! Lettering commands beyond plain text: monograms with decorative frames (PE-DESIGN's
//! Monogram tool and Decorative Import) and Name Drop (text list replacement, one design per
//! list row).

use crate::CommandSpec;
use crate::commands::{Effect, MM, default_fill, default_line, indexes, opt, req, spec, thread};
use crate::params::{CommandError, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_core::Thread;
use openstitch_design::monogram::{self, FRAMES, Letter, MonogramSpec, MonogramStyle};
use openstitch_design::text::{NameDrop, VAlign};
use openstitch_design::{
    DesignObject, LineSewing, LineType, ObjectKind, Rect, RegionSewing, RegionType, TextAlign,
    TextSpec, text,
};
use serde_json::{Value, json};

/// Most rows a Name Drop list may have (PE-DESIGN's limit).
pub const MAX_NAME_DROP_ROWS: usize = 500;
/// Largest Name Drop CSV file read.
const MAX_CSV_BYTES: usize = 1 << 20;

pub(crate) fn commands() -> Vec<CommandSpec> {
    let mut v = name_drop_commands();
    v.extend(monogram_commands());
    v
}

fn monogram_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "monogram.list",
            "Monogram letter styles and decorative frames",
            Effect::Read,
            vec![],
            |_, _| {
                Ok(json!({
                    "styles": MonogramStyle::ALL.iter().map(|s| json!({
                        "id": s.id(), "name": s.name(), "font": s.default_font(),
                        "editableText": s.is_text(),
                    })).collect::<Vec<_>>(),
                    "frames": FRAMES.iter().map(|f| json!({
                        "id": f.id, "name": f.name, "aspect": f.aspect,
                    })).collect::<Vec<_>>(),
                }))
            },
        ),
        spec(
            "monogram.add",
            "Add a monogram: 1-3 capital letters in a monogram style, optionally in a decorative frame (one group)",
            Effect::Edit,
            vec![
                req("letters", "string", "1-3 letters or digits (upper-cased)"),
                opt(
                    "style",
                    "block|script|diamond|circle",
                    "Letter arrangement (default block)",
                ),
                opt(
                    "font",
                    "string",
                    "Font for the letters (default per style; see fonts.list)",
                ),
                opt("size", "mm", "Height of the monogram, 5-300 (default 30)"),
                opt("x", "mm", "Centre x (default page centre)"),
                opt("y", "mm", "Centre y (default page centre)"),
                opt("frame", "string", "Decorative frame id (monogram.list)"),
                opt(
                    "margin",
                    "mm",
                    "Space between letters and frame, 0-50 (default 2)",
                ),
                opt(
                    "maintainAspect",
                    "bool",
                    "Keep the frame's own proportions (default true)",
                ),
                opt("fill", "color", "Letter thread"),
                opt("frameColor", "color", "Frame thread"),
            ],
            |s, p| {
                let style = style_param(p, MonogramStyle::Block)?;
                let page = s.design.page.center();
                let center = openstitch_design::v2(
                    p.opt("x", page.x / MM, -5000.0, 5000.0)? * MM,
                    p.opt("y", page.y / MM, -5000.0, 5000.0)? * MM,
                );
                let spec = MonogramSpec {
                    center,
                    letters: letters_param(p)?,
                    style,
                    font: font_param(s, p, style.default_font())?,
                    height: p.opt("size", 30.0, 5.0, 300.0)? * MM,
                    frame: frame_param(p)?,
                    margin: p.opt("margin", 2.0, 0.0, 50.0)? * MM,
                    maintain_aspect: p.bool("maintainAspect", Some(true))?,
                };
                let letter = thread(p, "fill")?.unwrap_or_else(default_fill);
                let frame = thread(p, "frameColor")?.unwrap_or_else(default_line);
                let at = s.design.objects.len();
                build(s, p, &spec, letter, frame, at)
            },
        ),
        spec(
            "monogram.edit",
            "Change a monogram's letters, style, font, size or frame (rebuilds its group in place)",
            Effect::Edit,
            vec![
                req("id", "int", "Any object of the monogram"),
                opt("letters", "string", "1-3 letters or digits"),
                opt("style", "block|script|diamond|circle", "Letter arrangement"),
                opt("font", "string", "Font for the letters"),
                opt("size", "mm", "Height, 5-300"),
                opt("frame", "string | null", "Frame id, or null for none"),
                opt("margin", "mm", "Space between letters and frame, 0-50"),
                opt("maintainAspect", "bool", "Keep the frame's own proportions"),
            ],
            edit,
        ),
        spec(
            "decoration.add",
            "Add a decorative frame around the given objects or box (PE-DESIGN's Decorative Import)",
            Effect::Edit,
            vec![
                req("frame", "string", "Frame id (monogram.list)"),
                opt(
                    "ids",
                    "[int]",
                    "Objects to frame (else x, y, width, height)",
                ),
                opt("x", "mm", "Box left"),
                opt("y", "mm", "Box top"),
                opt("width", "mm", "Box width"),
                opt("height", "mm", "Box height"),
                opt("margin", "mm", "Space around the box, 0-50 (default 2)"),
                opt(
                    "maintainAspect",
                    "bool",
                    "Keep the frame's own proportions (default true)",
                ),
                opt("color", "color", "Frame thread"),
            ],
            |s, p| {
                if p.is_null("frame") {
                    return Err(p.invalid("frame", "must be a frame id"));
                }
                p.string("frame", None)?;
                let id = frame_param(p)?.ok_or_else(|| p.invalid("frame", "must be a frame id"))?;
                let info = monogram::frame_info(&id).expect("checked");
                let (inner, at) = if p.has("ids") || p.has("id") {
                    let idx = indexes(p, &s.design)?;
                    let b = Rect::enclosing(idx.iter().flat_map(|&i| s.design.objects[i].points()))
                        .ok_or_else(|| p.failed("the objects have no extent"))?;
                    (b, idx.iter().copied().min().unwrap_or(0))
                } else {
                    let x = p.req("x", -5000.0, 5000.0)? * MM;
                    let y = p.req("y", -5000.0, 5000.0)? * MM;
                    let w = p.req("width", 1.0, 2000.0)? * MM;
                    let h = p.req("height", 1.0, 2000.0)? * MM;
                    (
                        Rect {
                            min_x: x,
                            min_y: y,
                            max_x: x + w,
                            max_y: y + h,
                        },
                        s.design.objects.len(),
                    )
                };
                let margin = p.opt("margin", 2.0, 0.0, 50.0)? * MM;
                let keep = p.bool("maintainAspect", Some(true))?;
                let colour = thread(p, "color")?.unwrap_or_else(default_line);
                let b = monogram::frame_box(info, inner, margin, keep);
                check_extent(p, &b)?;
                let objects = frame_objects(&id, b, &colour);
                let ids = insert_group(s, objects, at);
                Ok(
                    json!({ "ids": ids, "group": s.design.objects[s.design.index_of(ids[0]).expect("added")].group }),
                )
            },
        ),
    ]
}

fn letters_param(p: &Params) -> Result<String, CommandError> {
    monogram::normalize_letters(&p.string("letters", None)?)
        .ok_or_else(|| p.invalid("letters", "must be 1-3 letters or digits"))
}

fn style_param(p: &Params, default: MonogramStyle) -> Result<MonogramStyle, CommandError> {
    if !p.has("style") {
        return Ok(default);
    }
    MonogramStyle::from_id(&p.string("style", None)?)
        .ok_or_else(|| p.invalid("style", "must be block, script, diamond or circle"))
}

fn font_param(s: &EngineState, p: &Params, default: &str) -> Result<String, CommandError> {
    let f = p.string("font", Some(default))?;
    text::resolve(&f, &s.design.fonts)
        .map(|r| r.id().to_string())
        .ok_or_else(|| p.invalid("font", format!("'{f}' is not a font; see fonts.list")))
}

/// The `frame` parameter: a frame id, or `None` when absent or null.
fn frame_param(p: &Params) -> Result<Option<String>, CommandError> {
    if !p.has("frame") || p.is_null("frame") {
        return Ok(None);
    }
    let id = p.string("frame", None)?;
    monogram::frame_info(&id)
        .map(|f| Some(f.id.to_string()))
        .ok_or_else(|| {
            let ids: Vec<&str> = FRAMES.iter().map(|f| f.id).collect();
            p.invalid("frame", format!("must be one of {}", ids.join(", ")))
        })
}

fn check_extent(p: &Params, b: &Rect) -> Result<(), CommandError> {
    if b.width() > 2000.0 * MM || b.height() > 2000.0 * MM {
        return Err(p.failed("the frame would be larger than 2000 mm"));
    }
    Ok(())
}

fn frame_objects(id: &str, b: Rect, colour: &Thread) -> Vec<DesignObject> {
    monogram::frame(id, b)
        .into_iter()
        .map(|part| {
            let mut o = if part.filled {
                DesignObject::shape(
                    part.contours,
                    Some(RegionSewing::new(RegionType::Fill, colour.clone())),
                    None,
                )
            } else {
                DesignObject::shape(
                    part.contours,
                    None,
                    Some(LineSewing::new(LineType::Running, colour.clone())),
                )
            };
            o.name = Some(format!("Frame {id}"));
            o
        })
        .collect()
}

/// Adds `objects` as one group at sewing position `at`; their ids.
fn insert_group(s: &mut EngineState, objects: Vec<DesignObject>, at: usize) -> Vec<u64> {
    let group = (objects.len() > 1).then(|| s.design.new_group_id());
    let mut ids = Vec::new();
    for (k, mut o) in objects.into_iter().enumerate() {
        o.id = s.design.next_object_id;
        s.design.next_object_id += 1;
        o.group = group;
        ids.push(o.id);
        s.design.objects.insert(at + k, o);
    }
    ids
}

/// Builds the monogram `spec` centred on `center` at sewing position `at`: the frame first,
/// then the letters, all in one group.
fn build(
    s: &mut EngineState,
    p: &Params,
    spec: &MonogramSpec,
    letter: Thread,
    frame_thread: Thread,
    at: usize,
) -> Result<Value, CommandError> {
    let laid = monogram::letters(spec, &s.design.fonts).map_err(|e| p.failed(e.to_string()))?;
    let mut objects = Vec::new();
    let mut letter_points = Vec::new();
    for (k, l) in laid.into_iter().enumerate() {
        let region = Some(RegionSewing::new(RegionType::Fill, letter.clone()));
        let mut o = match l {
            Letter::Text(t) => {
                let layout = text::layout_in(&t, None, &s.design.fonts)
                    .map_err(|e| p.failed(e.to_string()))?;
                let mut o = DesignObject::shape(layout.contours(), region, None);
                o.kind = ObjectKind::Text;
                o.name = Some(format!("Monogram '{}'", t.text));
                o.text = Some(openstitch_design::TextSpec {
                    glyphs: layout.glyph_counts(),
                    ..*t
                });
                o
            }
            Letter::Shape { ch, contours } => {
                let mut o = DesignObject::shape(contours, region, None);
                o.name = Some(format!("Monogram '{ch}'"));
                o
            }
        };
        if k == 0 {
            o.monogram = Some(Box::new(spec.clone()));
        }
        letter_points.extend(o.points());
        objects.push(o);
    }
    let inner = Rect::enclosing(letter_points)
        .ok_or_else(|| p.failed("the font has none of the letters"))?;
    let mut all = Vec::new();
    if let Some(id) = &spec.frame {
        let info = monogram::frame_info(id).expect("checked");
        let b = monogram::frame_box(info, inner, spec.margin, spec.maintain_aspect);
        check_extent(p, &b)?;
        all.extend(frame_objects(id, b, &frame_thread));
    }
    all.extend(objects);
    let ids = insert_group(s, all, at);
    let group = s.design.objects[s.design.index_of(ids[0]).expect("added")].group;
    Ok(json!({ "ids": ids, "group": group, "letters": spec.letters }))
}

fn edit(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let id = p.int("id", None, 0, i64::MAX)? as u64;
    if s.design.index_of(id).is_none() {
        return Err(p.not_found(format!("object {id}")));
    }
    let members = s.design.with_groups(&[id]);
    let holder = members
        .iter()
        .filter_map(|&m| s.design.objects.get(s.design.index_of(m)?))
        .find(|o| o.monogram.is_some())
        .ok_or_else(|| p.failed(format!("object {id} is not part of a monogram")))?;
    let mut spec = (**holder.monogram.as_ref().expect("monogram")).clone();
    let letter = holder
        .region
        .as_ref()
        .map_or_else(default_fill, |r| r.thread.clone());
    let frame_thread = members
        .iter()
        .filter_map(|&m| s.design.objects.get(s.design.index_of(m)?))
        .find(|o| o.name.as_deref().is_some_and(|n| n.starts_with("Frame ")))
        .and_then(|o| {
            o.region
                .as_ref()
                .map(|r| r.thread.clone())
                .or_else(|| o.line.as_ref().map(|l| l.thread.clone()))
        })
        .unwrap_or_else(default_line);
    if p.has("letters") {
        spec.letters = letters_param(p)?;
    }
    let style = style_param(p, spec.style)?;
    if style != spec.style && !p.has("font") {
        spec.font = style.default_font().into();
    }
    spec.style = style;
    if p.has("font") {
        spec.font = font_param(s, p, &spec.font)?;
    }
    spec.height = p.opt("size", spec.height / MM, 5.0, 300.0)? * MM;
    if p.has("frame") || p.is_null("frame") {
        spec.frame = frame_param(p)?;
    }
    spec.margin = p.opt("margin", spec.margin / MM, 0.0, 50.0)? * MM;
    spec.maintain_aspect = p.bool("maintainAspect", Some(spec.maintain_aspect))?;

    let at = members
        .iter()
        .filter_map(|&m| s.design.index_of(m))
        .min()
        .unwrap_or(0);
    s.design.objects.retain(|o| !members.contains(&o.id));
    build(s, p, &spec, letter, frame_thread, at)
}

// MARK: Name Drop

/// Text objects with the Name Drop attribute, in sewing order: the list's columns.
pub(crate) fn name_drop_columns(d: &openstitch_design::Design) -> Vec<usize> {
    d.objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.text.as_ref().is_some_and(|t| t.name_drop.is_some()))
        .map(|(i, _)| i)
        .collect()
}

/// A row of strings from a JSON array (a lone string is a one-column row).
fn row_value(p: &Params, key: &str, v: &Value) -> Result<Vec<String>, CommandError> {
    match v {
        Value::String(s) => Ok(vec![s.clone()]),
        Value::Array(a) => a
            .iter()
            .map(|c| {
                c.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| p.invalid(key, "rows must hold strings"))
            })
            .collect(),
        _ => Err(p.invalid(key, "rows must be strings or arrays of strings")),
    }
}

/// Sets the Name Drop texts of `s` to `values` (one per column) and lays them out.
pub(crate) fn apply_row(
    s: &mut EngineState,
    p: &Params,
    values: &[String],
) -> Result<(), CommandError> {
    let cols = name_drop_columns(&s.design);
    if cols.is_empty() {
        return Err(p.failed("the design has no text with the Name Drop attribute"));
    }
    if values.len() != cols.len() {
        return Err(p.failed(format!(
            "a row needs {} value(s), one per Name Drop text; got {}",
            cols.len(),
            values.len()
        )));
    }
    for (&i, v) in cols.iter().zip(values) {
        let n = v.chars().count();
        if n == 0 || n > crate::commands::MAX_TEXT_CHARS {
            return Err(p.failed(format!(
                "Name Drop text must be 1-{} characters",
                crate::commands::MAX_TEXT_CHARS
            )));
        }
        s.design.objects[i]
            .text
            .as_mut()
            .expect("name drop text")
            .text = v.clone();
        crate::commands::relayout(s, p, i)?;
    }
    Ok(())
}

/// The rows of a Name Drop list from `rows` or a CSV file `csv`.
fn rows_param(s: &EngineState, p: &Params) -> Result<Vec<Vec<String>>, CommandError> {
    let rows = match (p.has("rows"), p.has("csv")) {
        (true, false) => p
            .array("rows")?
            .iter()
            .map(|v| row_value(p, "rows", v))
            .collect::<Result<Vec<_>, _>>()?,
        (false, true) => {
            let path = s.resolve(&p.string("csv", None)?, Access::Read, &p.command)?;
            read_csv(&path).map_err(|e| p.failed(e))?
        }
        _ => return Err(p.invalid("rows", "give either rows or csv")),
    };
    if rows.is_empty() || rows.len() > MAX_NAME_DROP_ROWS {
        return Err(p.invalid("rows", format!("must hold 1-{MAX_NAME_DROP_ROWS} rows")));
    }
    Ok(rows)
}

fn read_csv(path: &std::path::Path) -> Result<Vec<Vec<String>>, String> {
    let meta = std::fs::metadata(path).map_err(|_| "cannot read the CSV file".to_string())?;
    if meta.len() > MAX_CSV_BYTES as u64 {
        return Err("CSV file larger than 1 MiB".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "cannot read the CSV file".to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "the CSV file is not UTF-8".to_string())?;
    parse_csv(text.trim_start_matches('\u{feff}'))
}

/// RFC 4180 CSV: comma-separated, `"` quotes fields (doubled inside), CRLF or LF rows. Blank
/// lines are skipped.
pub fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let push_row = |row: &mut Vec<String>, rows: &mut Vec<Vec<String>>| -> Result<(), String> {
        let r = std::mem::take(row);
        if !(r.len() == 1 && r[0].is_empty()) {
            if rows.len() >= MAX_NAME_DROP_ROWS {
                return Err(format!("more than {MAX_NAME_DROP_ROWS} rows"));
            }
            rows.push(r);
        }
        Ok(())
    };
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' if field.is_empty() => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' if chars.peek() == Some(&'\n') => {}
                '\n' | '\r' => {
                    row.push(std::mem::take(&mut field));
                    push_row(&mut row, &mut rows)?;
                }
                _ => field.push(c),
            }
        }
        if field.chars().count() > crate::commands::MAX_TEXT_CHARS || row.len() > 64 {
            return Err("CSV field or row too long".into());
        }
    }
    if quoted {
        return Err("unterminated quoted field".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        push_row(&mut row, &mut rows)?;
    }
    Ok(rows)
}

/// Rows as CSV, quoting fields that need it.
pub fn write_csv(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for r in rows {
        let line: Vec<String> = r
            .iter()
            .map(|f| {
                if f.contains([',', '"', '\n', '\r']) {
                    format!("\"{}\"", f.replace('"', "\"\""))
                } else {
                    f.clone()
                }
            })
            .collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out
}

fn align_param(p: &Params, default: TextAlign) -> Result<TextAlign, CommandError> {
    if !p.has("align") {
        return Ok(default);
    }
    Ok(match p.string("align", None)?.as_str() {
        "left" => TextAlign::Left,
        "center" | "centre" => TextAlign::Center,
        "right" => TextAlign::Right,
        _ => return Err(p.invalid("align", "must be left, center or right")),
    })
}

fn valign_param(p: &Params, default: VAlign) -> Result<VAlign, CommandError> {
    if !p.has("valign") {
        return Ok(default);
    }
    Ok(match p.string("valign", None)?.as_str() {
        "top" => VAlign::Top,
        "middle" => VAlign::Middle,
        "bottom" => VAlign::Bottom,
        _ => return Err(p.invalid("valign", "must be top, middle or bottom")),
    })
}

/// Turns the Name Drop attribute on (fixing the text's current box) or off for one text.
fn set_name_drop(s: &mut EngineState, p: &Params, id: u64) -> Result<(), CommandError> {
    let i = crate::commands::text_index(s, p, id)?;
    let enabled = p.bool("enabled", Some(true))?;
    let o = &s.design.objects[i];
    let t = o.text.clone().expect("text");
    if !enabled {
        if t.name_drop.is_some() {
            s.design.objects[i].text.as_mut().expect("text").name_drop = None;
            crate::commands::relayout(s, p, i)?;
        }
        return Ok(());
    }
    if t.path.is_some() {
        return Err(p.failed(format!(
            "object {id} follows a path; Name Drop is not available for text fitted to an outline"
        )));
    }
    let members = s.design.with_groups(&[id]);
    if members.iter().any(|&m| {
        s.design
            .index_of(m)
            .is_some_and(|k| s.design.objects[k].monogram.is_some())
    }) {
        return Err(p.failed(format!(
            "object {id} is a monogram letter; Name Drop is not available for monograms"
        )));
    }
    if let Some(openstitch_design::DesignFont {
        data: openstitch_design::FontData::Embroidery(e),
        ..
    }) = s.design.fonts.iter().find(|f| f.id == t.font)
        && e.glyphs.iter().any(|g| g.stitches.is_some())
    {
        return Err(p.failed(format!(
            "object {id} is user-mapped text; Name Drop is not available for it"
        )));
    }
    let old = t.name_drop.clone();
    let label = if p.has("label") {
        let l = p.string("label", None)?.trim().to_string();
        if l.is_empty() || l.chars().count() > 64 {
            return Err(p.invalid("label", "must be 1-64 characters"));
        }
        Some(l)
    } else {
        old.as_ref().and_then(|n| n.label.clone())
    };
    let maintain_aspect = p.bool(
        "maintainAspect",
        Some(old.as_ref().is_none_or(|n| n.maintain_aspect)),
    )?;
    let align = align_param(p, old.as_ref().map_or(TextAlign::Center, |n| n.align))?;
    let valign = valign_param(p, old.as_ref().map_or(VAlign::Middle, |n| n.valign))?;
    let nd = match old {
        // Already on: keep its box.
        Some(n) => NameDrop {
            maintain_aspect,
            align,
            valign,
            label,
            ..n
        },
        None => {
            // Turning it on resets the spacing and character edits (as in PE-DESIGN), then
            // fixes the box the text occupies before its placement transform.
            let plain = TextSpec {
                letter_spacing: 0.0,
                line_spacing_percent: 100.0,
                line_spacing: 0.0,
                characters: Vec::new(),
                transform: openstitch_design::Transform::IDENTITY,
                ..t.clone()
            };
            let l = text::layout_in(&plain, None, &s.design.fonts)
                .map_err(|e| p.failed(e.to_string()))?;
            let b = Rect::enclosing(
                l.glyphs
                    .iter()
                    .flat_map(|g| g.contours.iter().flat_map(|c| c.outline_points())),
            )
            .ok_or_else(|| p.failed(format!("text of object {id} has no outline")))?;
            NameDrop {
                x: b.min_x,
                y: b.min_y,
                width: b.width(),
                height: b.height(),
                maintain_aspect,
                align,
                valign,
                label,
            }
        }
    };
    let spec = s.design.objects[i].text.as_mut().expect("text");
    if spec.name_drop.is_none() {
        spec.letter_spacing = 0.0;
        spec.line_spacing_percent = 100.0;
        spec.line_spacing = 0.0;
        spec.characters.clear();
    }
    spec.name_drop = Some(nd);
    crate::commands::relayout(s, p, i)?;
    Ok(())
}

/// Checks a base file name: no directories, no control characters.
fn base_name(p: &Params) -> Result<String, CommandError> {
    let n = p.string("name", None)?;
    let ok = (1..=64).contains(&n.chars().count())
        && !n.starts_with('.')
        && !n.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && !n.chars().any(char::is_control);
    if !ok {
        return Err(p.invalid(
            "name",
            "must be 1-64 characters without / \\ : * ? \" < > | or a leading dot",
        ));
    }
    Ok(n)
}

fn name_drop_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "text.setNameDrop",
            "Turn the Name Drop attribute on or off: the text is fitted into its current box whatever it says (resets letter and line spacing and character edits)",
            Effect::Edit,
            vec![
                crate::commands::IDS,
                opt("enabled", "bool", "On (default) or off"),
                opt(
                    "maintainAspect",
                    "bool",
                    "Scale the replacement uniformly (default true); off stretches it to the box",
                ),
                opt(
                    "align",
                    "left|center|right",
                    "Horizontal place in the box (default center)",
                ),
                opt(
                    "valign",
                    "top|middle|bottom",
                    "Vertical place in the box (default middle)",
                ),
                opt("label", "string", "Name of the field (template slot)"),
            ],
            |s, p| {
                let ids = p.ids()?;
                if ids.is_empty() {
                    return Err(p.invalid("ids", "must not be empty"));
                }
                for &id in &ids {
                    set_name_drop(s, p, id)?;
                }
                Ok(json!({ "columns": name_drop_columns(&s.design).len() }))
            },
        ),
        spec(
            "nameDrop.list",
            "The Name Drop texts (the list's columns), in sewing order",
            Effect::Read,
            vec![],
            |s, _| {
                Ok(Value::Array(
                    name_drop_columns(&s.design)
                        .into_iter()
                        .map(|i| {
                            let o = &s.design.objects[i];
                            let t = o.text.as_ref().expect("text");
                            let nd = t.name_drop.as_ref().expect("name drop");
                            json!({
                                "id": o.id, "text": t.text, "label": nd.label,
                                "widthMM": nd.width / MM, "heightMM": nd.height / MM,
                                "maintainAspect": nd.maintain_aspect,
                            })
                        })
                        .collect(),
                ))
            },
        ),
        spec(
            "nameDrop.apply",
            "Put one row of a Name Drop list into the design (one value per Name Drop text)",
            Effect::Edit,
            vec![req(
                "values",
                "[string]",
                "One value per Name Drop text, in order",
            )],
            |s, p| {
                let values = row_value(
                    p,
                    "values",
                    p.get("values")
                        .ok_or_else(|| p.invalid("values", "required"))?,
                )?;
                apply_row(s, p, &values)?;
                Ok(json!({ "columns": values.len() }))
            },
        ),
        spec(
            "nameDrop.readCsv",
            "Read a Name Drop list from a CSV file (one row per design, one column per Name Drop text)",
            Effect::Read,
            vec![req(
                "path",
                "string",
                "CSV file (UTF-8, at most 1 MiB and 500 rows)",
            )],
            |s, p| {
                let path = s.resolve(&p.string("path", None)?, Access::Read, &p.command)?;
                let rows = read_csv(&path).map_err(|e| p.failed(e))?;
                Ok(json!({ "rows": rows }))
            },
        ),
        spec(
            "nameDrop.writeCsv",
            "Save a Name Drop list as a CSV file",
            Effect::File,
            vec![
                req("path", "string", "CSV file"),
                req("rows", "[[string]]", "The list"),
            ],
            |s, p| {
                let rows = rows_param(s, p)?;
                let path = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
                std::fs::write(&path, write_csv(&rows)).map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({ "path": path.display().to_string(), "rows": rows.len() }))
            },
        ),
        spec(
            "nameDrop.export",
            "Name Drop To Folder: write one embroidery file per list row, named <name>001, <name>002, … (the design is unchanged)",
            Effect::File,
            vec![
                opt(
                    "rows",
                    "[[string] | string]",
                    "The list: one row per design",
                ),
                opt("csv", "string", "Or a CSV file holding the list"),
                opt(
                    "dir",
                    "string",
                    "Folder (default the session folder); must exist",
                ),
                req("name", "string", "Base file name"),
                opt("format", "pes|dst|…", "File type (default pes)"),
                opt(
                    "pesVersion",
                    "1t|1|6|6t",
                    "PES only: version (default 6; 1t = machine only, smallest)",
                ),
            ],
            export,
        ),
    ]
}

#[cfg(feature = "formats")]
fn export(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    use openstitch_formats::FileType;
    let rows = rows_param(s, p)?;
    let name = base_name(p)?;
    let ext = p.string("format", Some("pes"))?.to_ascii_lowercase();
    let ty = FileType::from_extension(&ext)
        .filter(|t| t.can_write())
        .ok_or_else(|| {
            let names: Vec<_> = FileType::writable().map(|t| t.extension()).collect();
            p.invalid("format", format!("use {}", names.join(", ")))
        })?;
    let pes = openstitch_formats::pes::WriteOptions::parse(&p.string(
        "pesVersion",
        Some(openstitch_formats::pes::WriteOptions::DEFAULT_NAME),
    )?)
    .ok_or_else(|| p.invalid("pesVersion", "use 1t, 1, 6 or 6t"))?;
    let dir = p.string("dir", Some(""))?;
    // Every row is laid out, compiled and encoded before any file is written.
    let mut files = Vec::new();
    for (k, row) in rows.iter().enumerate() {
        let mut scratch = s.clone();
        apply_row(&mut scratch, p, row).map_err(|e| p.failed(format!("row {}: {}", k + 1, e)))?;
        let pattern =
            openstitch_design::Compiler::for_design(&scratch.design).compile(&scratch.design);
        if pattern.count(openstitch_core::Command::Stitch) == 0 {
            return Err(p.failed(format!("row {} has no stitches", k + 1)));
        }
        let bytes = match ty {
            FileType::Pes => openstitch_formats::pes::write_with(&pattern, &pes),
            _ => ty.write(&pattern),
        }
        .map_err(|e| p.failed(e.to_string()))?;
        let file = format!("{name}{:03}.{}", k + 1, ty.extension());
        let rel = if dir.is_empty() {
            file
        } else {
            format!("{}/{file}", dir.trim_end_matches(['/', '\\']))
        };
        let path = s.resolve(&rel, Access::Write, &p.command)?;
        files.push((path, bytes));
    }
    let mut written = Vec::new();
    for (path, bytes) in files {
        std::fs::write(&path, bytes).map_err(|e| p.failed(format!("{}: {e}", path.display())))?;
        written.push(path.display().to_string());
    }
    Ok(json!({ "files": written, "count": written.len() }))
}

#[cfg(not(feature = "formats"))]
fn export(_: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    Err(p.failed("embroidery file export is not built in"))
}
