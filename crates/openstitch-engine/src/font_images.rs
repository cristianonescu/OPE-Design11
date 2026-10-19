//! Font Creator: characters of an embroidery font from an image in one step (PE-DESIGN's
//! "Creating New Font Characters From Background Images", done automatically). The image —
//! a file, or the design's background image as placed on the Design Page — is thresholded,
//! its ink split into connected components, the components grouped into characters (rows top
//! to bottom, left to right; pieces stacked over each other, like the dot of an "i", stay one
//! character) and each character traced with the digitizing crate's tracer, as Auto Punch
//! traces colour areas. `fonts.traceImage` previews the characters so they can be assigned
//! before `fonts.fromImage` stores them, as one undo step.
//!
//! Characters are region (block) outlines, like the guide's block-type punching: text using
//! the font sews them with the text object's region sewing, the Font Creator default.

use crate::commands::{CommandSpec, Effect, MM, ParamDoc, opt, req, spec};
use crate::fonts::{embroidery_mut, store_glyph};
use crate::params::{CommandError, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_design::fonts::EmbroideryGlyph;
use openstitch_design::{Contour, Rect, v2};
use openstitch_digitize::raster::MAX_FILE_BYTES;
use openstitch_digitize::regions::{self, NONE, TraceOptions};
use openstitch_digitize::{Frame, Raster};
use serde_json::{Value, json};

/// Most characters one image may give.
const MAX_CHARACTERS: usize = 1024;
/// Side bearings of a fitted character, fractions of the standard height: the outline starts
/// this far right of the left set line, and the right set line is twice this past its right
/// edge (as `fonts.setGlyph` defaults the width).
const BEARING: f64 = 0.04;
/// Two components whose horizontal extents overlap by this fraction of the narrower one are
/// one character (the dot and stem of "i", the parts of "=" or "%" stacked over each other).
const STACK_OVERLAP: f64 = 0.5;
/// Most points of one previewed outline.
const MAX_PREVIEW_POINTS: usize = 400;

fn image_docs(mut v: Vec<ParamDoc>) -> Vec<ParamDoc> {
    v.extend([
        opt(
            "path",
            "string",
            "Image file: PNG, JPEG or BMP (default: the design's background image as shown)",
        ),
        opt(
            "mode",
            "string",
            "split: one character per group of ink (default); whole: the whole image is one character",
        ),
        opt(
            "threshold",
            "int 1-254",
            "Pixels darker than this are ink (default 128)",
        ),
        opt(
            "invert",
            "bool",
            "Light lettering on a dark ground (default: detected from the image border)",
        ),
        opt(
            "minArea",
            "int",
            "Ink specks smaller than this many pixels are ignored (default: from the image size)",
        ),
        opt(
            "resolution",
            "int 64-4096",
            "Working resolution, longest side in pixels (default 1200)",
        ),
        opt(
            "capPixels",
            "int",
            "Image pixels from baseline to capital line (default: the tallest character)",
        ),
        opt(
            "x",
            "mm",
            "whole mode on the background image: left set line on the Design Page (with y; the image is used where it is placed, unscaled)",
        ),
        opt(
            "y",
            "mm",
            "whole mode on the background image: baseline on the Design Page",
        ),
        opt(
            "simplify",
            "px",
            "Tracing tolerance, 0.2-5 pixels (default 0.7)",
        ),
        opt(
            "curves",
            "bool",
            "Fit Bézier curves to the traced outlines (default true)",
        ),
    ]);
    v
}

pub(crate) fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "fonts.traceImage",
            "Font Creator: find the characters of an image (scanned lettering, a drawn alphabet) before fonts.fromImage; the design is not changed",
            Effect::Read,
            image_docs(vec![
                opt(
                    "font",
                    "string",
                    "Embroidery font whose standard height sizes the characters (default 10 mm)",
                ),
                opt(
                    "chars",
                    "string | [string]",
                    "Characters to assign in order (a space or \"\" skips one)",
                ),
                opt(
                    "outlines",
                    "bool",
                    "Include each character's traced outline as mm polylines (default false)",
                ),
            ]),
            trace_image,
        ),
        spec(
            "fonts.fromImage",
            "Font Creator: trace an image into characters of an embroidery font in one step (one character, or one per group of ink assigned in order)",
            Effect::Edit,
            image_docs(vec![
                req("font", "string", "Embroidery font id"),
                req(
                    "chars",
                    "string | [string]",
                    "Characters for the image's characters in order (a space or \"\" skips one); whole mode takes one",
                ),
            ]),
            from_image,
        ),
    ]
}

/// One character found in the image: a group of ink components.
struct Group {
    components: Vec<u32>,
    /// Bounding box `[x0, x1) × [y0, y1)`, working pixels.
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    row: usize,
    /// Baseline of its row, working pixels from the top.
    baseline: usize,
}

/// The thresholded image and its characters.
struct Detected {
    w: usize,
    h: usize,
    id: Vec<u32>,
    groups: Vec<Group>,
    rows: usize,
    inverted: bool,
    /// Where working pixels land on the Design Page (background image only).
    page: Option<Frame>,
}

/// Reads the image: the `path` file, or the background image as shown on the Design Page.
fn load(s: &EngineState, p: &Params) -> Result<(Raster, Option<Frame>), CommandError> {
    if p.has("path") {
        let (_, data) = crate::commands::read_file(s, p, MAX_FILE_BYTES)?;
        let image = Raster::decode(&data).map_err(|e| p.failed(e.to_string()))?;
        return Ok((image, None));
    }
    let bg = s.design.background.as_ref().ok_or_else(|| {
        p.failed("give path, or place a background image first (View ▸ Template ▸ Open / background.set)")
    })?;
    let path = s.resolve(&bg.path, Access::Read, &p.command)?;
    let meta = std::fs::metadata(&path)
        .map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
    if meta.len() > MAX_FILE_BYTES as u64 {
        return Err(p.failed("background image too large"));
    }
    let data =
        std::fs::read(&path).map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
    let image = Raster::decode(&data)
        .map_err(|e| p.failed(e.to_string()))?
        .with_adjust(&bg.adjust);
    let frame = Frame {
        origin: v2(bg.x, bg.y),
        scale: bg.width / f64::from(image.width().max(1)),
    };
    Ok((image, Some(frame)))
}

fn mode(p: &Params) -> Result<bool, CommandError> {
    match p.string("mode", Some("split"))?.as_str() {
        "split" => Ok(false),
        "whole" => Ok(true),
        _ => Err(p.invalid("mode", "must be split or whole")),
    }
}

fn detect(s: &EngineState, p: &Params) -> Result<Detected, CommandError> {
    let whole = mode(p)?;
    let threshold = p.int("threshold", Some(128), 1, 254)? as f64;
    let resolution = p.int("resolution", Some(1200), 64, 4096)? as u32;
    let (image, page) = load(s, p)?;
    let work = image.fit(resolution);
    let page = page.map(|f| f.rescaled(image.width(), work.width()));
    let (w, h) = (work.width() as usize, work.height() as usize);
    let dark: Vec<bool> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| work.luma_over_white(x as u32, y as u32) < threshold)
        .collect();
    let inverted = if p.has("invert") {
        p.bool("invert", None)?
    } else {
        // Ink is the minority on the image border.
        let border: Vec<bool> = (0..w)
            .flat_map(|x| [dark[x], dark[(h - 1) * w + x]])
            .chain((0..h).flat_map(|y| [dark[y * w], dark[y * w + w - 1]]))
            .collect();
        border.iter().filter(|&&d| d).count() * 2 > border.len()
    };
    let labels: Vec<u16> = dark
        .iter()
        .map(|&d| if d != inverted { 0 } else { NONE })
        .collect();
    let (id, components) = regions::components(&labels, w, h);
    let auto_min = ((w * h) / 50_000).max(4);
    let min_area = p.int("minArea", Some(auto_min as i64), 1, 1 << 24)? as usize;
    let kept: Vec<u32> = (0..components.len() as u32)
        .filter(|&c| components[c as usize].area >= min_area)
        .collect();
    if kept.is_empty() {
        return Err(
            p.failed("the image has no ink to trace (try another threshold, invert or minArea)")
        );
    }
    let boxed = |cs: &[u32]| {
        let k = |c: &u32| &components[*c as usize];
        (
            cs.iter().map(|c| k(c).x0).min().unwrap_or(0),
            cs.iter().map(|c| k(c).y0).min().unwrap_or(0),
            cs.iter().map(|c| k(c).x1).max().unwrap_or(0),
            cs.iter().map(|c| k(c).y1).max().unwrap_or(0),
        )
    };
    let group = |cs: Vec<u32>, row: usize| {
        let (x0, y0, x1, y1) = boxed(&cs);
        Group {
            components: cs,
            x0,
            y0,
            x1,
            y1,
            row,
            baseline: y1,
        }
    };
    let (groups, rows) = if whole {
        (vec![group(kept, 0)], 1)
    } else {
        // Rows: components join the row whose vertical extent holds their centre (or that
        // holds the row's centre), taken from the top.
        let mut order = kept;
        order.sort_by_key(|&c| {
            let k = &components[c as usize];
            (k.y0 + k.y1, k.x0)
        });
        let mut rows: Vec<(usize, usize, Vec<u32>)> = Vec::new();
        for c in order {
            let k = &components[c as usize];
            let centre = (k.y0 + k.y1) / 2;
            match rows.iter_mut().find(|(y0, y1, _)| {
                (*y0..*y1).contains(&centre) || (k.y0..k.y1).contains(&((y0 + y1) / 2))
            }) {
                Some(r) => {
                    r.0 = r.0.min(k.y0);
                    r.1 = r.1.max(k.y1);
                    r.2.push(c);
                }
                None => rows.push((k.y0, k.y1, vec![c])),
            }
        }
        rows.sort_by_key(|r| r.0);
        let mut groups = Vec::new();
        for (row, (_, _, mut cs)) in rows.iter().cloned().enumerate() {
            cs.sort_by_key(|&c| components[c as usize].x0);
            let mut in_row: Vec<Group> = Vec::new();
            for c in cs {
                let k = &components[c as usize];
                let stacked = in_row.iter_mut().rev().find(|g| {
                    let overlap = g.x1.min(k.x1).saturating_sub(g.x0.max(k.x0));
                    let narrower = (g.x1 - g.x0).min(k.x1 - k.x0).max(1);
                    overlap as f64 >= STACK_OVERLAP * narrower as f64
                });
                match stacked {
                    Some(g) => {
                        g.components.push(c);
                        (g.x0, g.y0, g.x1, g.y1) = boxed(&g.components);
                    }
                    None => in_row.push(group(vec![c], row)),
                }
            }
            in_row.sort_by_key(|g| g.x0);
            // The row's baseline: the median bottom of its characters (descenders hang below).
            let mut bottoms: Vec<usize> = in_row.iter().map(|g| g.y1).collect();
            bottoms.sort_unstable();
            let baseline = bottoms[bottoms.len() / 2];
            for g in &mut in_row {
                g.baseline = baseline;
            }
            groups.extend(in_row);
        }
        (groups, rows.len())
    };
    if groups.len() > MAX_CHARACTERS {
        return Err(p.failed(format!(
            "the image has {} characters; at most {MAX_CHARACTERS} (raise minArea)",
            groups.len()
        )));
    }
    Ok(Detected {
        w,
        h,
        id,
        groups,
        rows,
        inverted,
        page,
    })
}

/// How detected characters become glyphs: fitted to the standard height, or as placed on
/// the Design Page relative to a set line and baseline.
enum Placement {
    /// Design units per working pixel.
    Fit(f64),
    /// Page frame of the working image, moved so the set line and baseline are the origin.
    Page(Frame),
}

fn placement(p: &Params, d: &Detected, cap: f64) -> Result<Placement, CommandError> {
    if p.has("x") || p.has("y") {
        let frame = d.page.ok_or_else(|| {
            p.invalid(
                "x",
                "places a character from the background image; omit path",
            )
        })?;
        if d.groups.len() != 1 || !matches!(p.string("mode", Some("split"))?.as_str(), "whole") {
            return Err(p.invalid("x", "is for mode whole"));
        }
        let x = p.req("x", -5000.0, 5000.0)? * MM;
        let y = p.req("y", -5000.0, 5000.0)? * MM;
        return Ok(Placement::Page(Frame {
            origin: frame.origin - v2(x, y),
            scale: frame.scale,
        }));
    }
    let tallest = d
        .groups
        .iter()
        .map(|g| g.baseline.saturating_sub(g.y0))
        .max()
        .unwrap_or(1)
        .max(1);
    let cap_px = p.int("capPixels", Some(tallest as i64), 1, d.h.max(1) as i64)? as f64;
    Ok(Placement::Fit(cap / cap_px))
}

/// A traced character, in glyph coordinates (left set line and baseline at the origin).
struct Traced {
    contours: Vec<Contour>,
    advance: f64,
}

fn trace_group(d: &Detected, g: &Group, at: &Placement, cap: f64, o: &TraceOptions) -> Traced {
    let (mw, mh) = (g.x1 - g.x0, g.y1 - g.y0);
    let mut m = vec![false; mw * mh];
    for y in 0..mh {
        for x in 0..mw {
            let c = d.id[(y + g.y0) * d.w + x + g.x0];
            m[y * mw + x] = c != u32::MAX && g.components.contains(&c);
        }
    }
    let frame = match at {
        Placement::Fit(s) => Frame {
            origin: v2(BEARING * cap - g.x0 as f64 * s, -(g.baseline as f64) * s),
            scale: *s,
        },
        Placement::Page(f) => *f,
    };
    let contours = regions::trace_mask(&m, mw, mh, g.x0, g.y0, o, &frame);
    let right =
        Rect::enclosing(contours.iter().flat_map(Contour::outline_points)).map_or(0.0, |b| b.max_x);
    Traced {
        contours,
        advance: (right + 2.0 * BEARING * cap).max(0.0),
    }
}

fn trace_options(p: &Params) -> Result<TraceOptions, CommandError> {
    Ok(TraceOptions {
        simplify: p.opt("simplify", 0.7, 0.2, 5.0)?,
        curves: p.bool("curves", Some(true))?,
        ..TraceOptions::default()
    })
}

/// The `chars` parameter: one entry per character in order, `None` to skip.
fn assignments(p: &Params) -> Result<Vec<Option<char>>, CommandError> {
    let bad = || {
        p.invalid(
            "chars",
            "must be a string or an array of one-character strings",
        )
    };
    let one = |v: &str| -> Result<Option<char>, CommandError> {
        let mut it = v.chars();
        match (it.next(), it.next()) {
            (None, _) => Ok(None),
            (Some(' '), None) => Ok(None),
            (Some(c), None) if !c.is_control() => Ok(Some(c)),
            _ => Err(bad()),
        }
    };
    let list: Vec<Option<char>> = match p.get("chars") {
        None => return Ok(Vec::new()),
        Some(Value::String(s)) => s
            .chars()
            .filter(|c| !c.is_control())
            .map(|c| (c != ' ').then_some(c))
            .collect(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().ok_or_else(bad).and_then(one))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(bad()),
    };
    if list.len() > MAX_CHARACTERS {
        return Err(p.invalid(
            "chars",
            format!("must hold at most {MAX_CHARACTERS} entries"),
        ));
    }
    let mut seen: Vec<char> = list.iter().flatten().copied().collect();
    seen.sort_unstable();
    if let Some(w) = seen.windows(2).find(|w| w[0] == w[1]) {
        return Err(p.invalid("chars", format!("assigns {:?} twice", w[0])));
    }
    Ok(list)
}

/// The standard height of the `font` parameter's embroidery font, or 10 mm.
fn cap_height(s: &mut EngineState, p: &Params, required: bool) -> Result<f64, CommandError> {
    if required || p.has("font") {
        Ok(embroidery_mut(s, p)?.cap_height)
    } else {
        Ok(10.0 * MM)
    }
}

fn trace_image(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let cap = cap_height(s, p, false)?;
    let chars = assignments(p)?;
    let outlines = p.bool("outlines", Some(false))?;
    let o = trace_options(p)?;
    let d = detect(s, p)?;
    let at = placement(p, &d, cap)?;
    let rows: Vec<Value> = d
        .groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let t = trace_group(&d, g, &at, cap, &o);
            let b = Rect::enclosing(t.contours.iter().flat_map(Contour::outline_points));
            let mut row = json!({
                "index": i, "row": g.row, "box": [g.x0, g.y0, g.x1, g.y1],
                "components": g.components.len(), "contours": t.contours.len(),
                "advanceMM": t.advance / MM,
                "widthMM": b.map_or(0.0, |b| b.width() / MM),
                "heightMM": b.map_or(0.0, |b| b.height() / MM),
                "char": chars.get(i).copied().flatten().map(String::from),
            });
            if outlines {
                row["outline"] = t
                    .contours
                    .iter()
                    .map(|c| {
                        let pts = c.outline_points();
                        let step = pts.len().div_ceil(MAX_PREVIEW_POINTS).max(1);
                        pts.iter()
                            .step_by(step)
                            .map(|q| json!([q.x / MM, q.y / MM]))
                            .collect::<Value>()
                    })
                    .collect();
            }
            row
        })
        .collect();
    Ok(json!({
        "width": d.w, "height": d.h, "rows": d.rows, "inverted": d.inverted,
        "heightMM": cap / MM, "characters": rows,
    }))
}

fn from_image(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let cap = cap_height(s, p, true)?;
    let chars = assignments(p)?;
    let o = trace_options(p)?;
    let d = detect(s, p)?;
    if chars.len() > d.groups.len() {
        return Err(p.invalid(
            "chars",
            format!(
                "assigns {} characters but the image has {}",
                chars.len(),
                d.groups.len()
            ),
        ));
    }
    if chars.iter().all(Option::is_none) {
        return Err(p.invalid("chars", "assigns no character"));
    }
    let at = placement(p, &d, cap)?;
    let (mut created, mut stored) = (String::new(), Vec::new());
    for (g, ch) in d.groups.iter().zip(&chars) {
        let Some(ch) = *ch else { continue };
        let t = trace_group(&d, g, &at, cap, &o);
        if t.contours.is_empty() {
            return Err(p.failed(format!("character {ch:?} traced to no outline")));
        }
        stored.push(json!({
            "char": ch.to_string(), "advanceMM": t.advance / MM, "contours": t.contours.len(),
        }));
        store_glyph(
            s,
            p,
            EmbroideryGlyph {
                ch,
                advance: t.advance,
                contours: t.contours,
                stitches: None,
            },
        )?;
        created.push(ch);
    }
    let glyphs = embroidery_mut(s, p)?.glyphs.len();
    Ok(json!({
        "created": created, "characters": stored, "found": d.groups.len(),
        "skipped": d.groups.len() - created.chars().count(), "glyphs": glyphs,
        "sewing": "region",
    }))
}
