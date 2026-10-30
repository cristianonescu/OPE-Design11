//! Design Settings commands: machine type, Brother and user hoops, rotation, sewing area,
//! jump-stitch trimming and estimate speed; plus the measure command and the estimates that
//! `design.info` reports.

use crate::commands::{CommandSpec, Effect, ParamDoc};
use crate::params::{CommandError, CommandResult, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_core::StitchPattern;
use openstitch_design::fmath::Det;
use openstitch_design::machine::{self, user_hoop_key};
use openstitch_design::{
    BROTHER_HOOPS, Design, MachineSettings, MachineType, SewingArea, Transform, UserHoop, v2,
};
use serde_json::{Value, json};

const MM: f64 = 10.0;
/// User hoops and custom pages: PE-DESIGN gives no limits; these match `design.new`.
const MIN_SIZE: f64 = 10.0;
const MAX_SIZE: f64 = 2000.0;
const MAX_USER_HOOPS: usize = 100;
/// Largest user-hoop table read, bytes.
const MAX_HOOP_TABLE: u64 = 1 << 20;

fn spec(
    id: &'static str,
    summary: &'static str,
    effect: Effect,
    params: Vec<ParamDoc>,
    run: crate::commands::RunFn,
) -> CommandSpec {
    CommandSpec {
        id,
        summary,
        effect,
        params,
        run,
    }
}

const fn opt(name: &'static str, kind: &'static str, summary: &'static str) -> ParamDoc {
    ParamDoc {
        name,
        kind,
        required: false,
        summary,
    }
}

const fn req(name: &'static str, kind: &'static str, summary: &'static str) -> ParamDoc {
    ParamDoc {
        name,
        kind,
        required: true,
        summary,
    }
}

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "machine.hoops",
            "Machine types, Brother and user hoops, and the current Design Settings",
            Effect::Read,
            vec![],
            |s, _| Ok(hoops(&s.design)),
        ),
        spec(
            "machine.set",
            "Change Design Settings: machine type, hoop (resizes the Design Page, keeping objects centred), rotation, custom size, sewing area, jump-stitch trimming, speed",
            Effect::Edit,
            vec![
                opt("machine", "single|multi", "Machine type"),
                opt(
                    "hoop",
                    "string",
                    "Brother hoop id (see machine.hoops), user-<n> for a user hoop, or custom",
                ),
                opt(
                    "rotate90",
                    "bool",
                    "Rotate the hoop 90° (not for user hoops)",
                ),
                opt("width", "mm", "Custom Design Page width (hoop: custom)"),
                opt("height", "mm", "Custom Design Page height (hoop: custom)"),
                opt(
                    "sewingArea",
                    "page|design",
                    "Needle starts at the Design Page centre (page) or the design centre (design)",
                ),
                opt(
                    "minJumpTrim",
                    "mm",
                    "Multi-needle machines: trim jumps at least this long (0.5-50)",
                ),
                opt(
                    "speed",
                    "int 100-2000",
                    "Stitches per minute for the sewing-time estimate",
                ),
            ],
            set,
        ),
        spec(
            "machine.addUserHoop",
            "Add a user hoop size",
            Effect::Edit,
            vec![
                req("width", "mm", "Width"),
                req("height", "mm", "Height"),
                opt("comment", "string", "Shown beside the size"),
            ],
            |s, p| {
                let m = &mut s.design.machine;
                if m.user_hoops.len() >= MAX_USER_HOOPS {
                    return Err(p.failed(format!("at most {MAX_USER_HOOPS} user hoops")));
                }
                let comment = p.string("comment", Some(""))?;
                if comment.chars().count() > 100 {
                    return Err(p.invalid("comment", "must be at most 100 characters"));
                }
                let hoop = UserHoop {
                    id: m.next_user_hoop_id(),
                    width: p.req("width", MIN_SIZE, MAX_SIZE)?,
                    height: p.req("height", MIN_SIZE, MAX_SIZE)?,
                    comment,
                };
                let key = user_hoop_key(hoop.id);
                m.user_hoops.push(hoop);
                Ok(json!({ "hoop": key }))
            },
        ),
        spec(
            "machine.importUserHoops",
            "Add the user hoops of a table file: per hoop a line width_mm,height_mm,width_in10,height_in10, then a name line (the mm pair is used)",
            Effect::Edit,
            vec![crate::commands::PATH],
            |s, p| {
                let path = s.resolve(&p.string("path", None)?, Access::Read, &p.command)?;
                let meta = std::fs::metadata(&path).map_err(|e| p.failed(e.to_string()))?;
                if meta.len() > MAX_HOOP_TABLE {
                    return Err(p.failed("the file is too large"));
                }
                let bytes = std::fs::read(&path).map_err(|e| p.failed(e.to_string()))?;
                let hoops =
                    parse_user_hoops(&String::from_utf8_lossy(&bytes)).map_err(|e| p.failed(e))?;
                let m = &mut s.design.machine;
                if m.user_hoops.len() + hoops.len() > MAX_USER_HOOPS {
                    return Err(p.failed(format!("at most {MAX_USER_HOOPS} user hoops")));
                }
                let mut keys = Vec::with_capacity(hoops.len());
                for (width, height, comment) in hoops {
                    let id = m.next_user_hoop_id();
                    m.user_hoops.push(UserHoop {
                        id,
                        width,
                        height,
                        comment,
                    });
                    keys.push(user_hoop_key(id));
                }
                Ok(json!({ "hoops": keys }))
            },
        ),
        spec(
            "machine.deleteUserHoop",
            "Delete a user hoop that is not in use",
            Effect::Edit,
            vec![req("hoop", "string", "user-<n>")],
            |s, p| {
                let key = p.string("hoop", None)?;
                let m = &mut s.design.machine;
                let i = user_index(m, &key, p)?;
                if m.hoop.as_deref() == Some(key.as_str()) {
                    return Err(p.failed(format!("{key} is the selected hoop; pick another first")));
                }
                m.user_hoops.remove(i);
                Ok(Value::Null)
            },
        ),
        spec(
            "machine.moveUserHoop",
            "Move a user hoop up or down the list",
            Effect::Edit,
            vec![
                req("hoop", "string", "user-<n>"),
                req("direction", "up|down", "Direction"),
            ],
            |s, p| {
                let key = p.string("hoop", None)?;
                let up = match p.string("direction", None)?.as_str() {
                    "up" => true,
                    "down" => false,
                    _ => return Err(p.invalid("direction", "must be up or down")),
                };
                let m = &mut s.design.machine;
                let i = user_index(m, &key, p)?;
                let j = if up { i.checked_sub(1) } else { Some(i + 1) };
                if let Some(j) = j.filter(|&j| j < m.user_hoops.len()) {
                    m.user_hoops.swap(i, j);
                }
                Ok(Value::Null)
            },
        ),
        spec(
            "design.measure",
            "Distance, offsets and angle between two points (mm; angle in degrees, counter-clockwise from +x)",
            Effect::Read,
            vec![
                req("from", "[x, y] mm", "Start point"),
                req("to", "[x, y] mm", "End point"),
            ],
            |_, p| {
                let a = p.point("from")?;
                let b = p.point("to")?;
                Ok(measure(a.x / MM, a.y / MM, b.x / MM, b.y / MM))
            },
        ),
    ]
}

/// Distance and angle between two points in mm. The design's y axis points down, so the
/// angle is negated to read counter-clockwise like a protractor.
pub fn measure(x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let round = |v: f64| (v * 100.0).round() / 100.0;
    let angle = if dx == 0.0 && dy == 0.0 {
        0.0
    } else {
        (-dy).datan2(dx).to_degrees()
    };
    json!({
        "distanceMM": round(dx.dhypot(dy)),
        "dxMM": round(dx),
        "dyMM": round(dy),
        "angleDegrees": round(angle),
    })
}

fn user_index(m: &MachineSettings, key: &str, p: &Params) -> Result<usize, CommandError> {
    let hoop = m
        .user_hoop(key)
        .ok_or_else(|| p.not_found(format!("user hoop {key}")))?;
    let id = hoop.id;
    Ok(m.user_hoops
        .iter()
        .position(|h| h.id == id)
        .expect("found above"))
}

fn set(s: &mut EngineState, p: &Params) -> CommandResult {
    let mut m = s.design.machine.clone();
    if p.has("machine") {
        let v = p.string("machine", None)?;
        m.machine = MachineType::parse(&v)
            .ok_or_else(|| p.invalid("machine", "must be single or multi"))?;
    }
    if p.has("sewingArea") {
        let v = p.string("sewingArea", None)?;
        m.sewing_area = SewingArea::parse(&v)
            .ok_or_else(|| p.invalid("sewingArea", "must be page or design"))?;
    }
    if p.has("speed") {
        m.speed = Some(p.int("speed", None, 100, 2000)? as f64);
    }
    if p.has("rotate90") {
        m.rotate90 = p.bool("rotate90", None)?;
    }
    let custom_size = p.has("width") || p.has("height");
    if p.has("hoop") {
        let key = p.string("hoop", None)?;
        m.hoop = match key.as_str() {
            "custom" => None,
            k if machine::brother_hoop(k).is_some() || m.user_hoop(k).is_some() => Some(key),
            _ => return Err(p.not_found(format!("hoop {key}"))),
        };
    } else if custom_size {
        m.hoop = None;
    }
    let user = m.hoop.as_deref().is_some_and(|k| m.user_hoop(k).is_some());
    if user && m.rotate90 && !p.has("rotate90") {
        m.rotate90 = false;
    }
    if user && m.rotate90 {
        return Err(p.invalid("rotate90", "a user hoop cannot be rotated 90°"));
    }
    if m.hoop.is_none() {
        // A custom page cannot rotate; its width and height are given directly.
        m.rotate90 = false;
    }
    if p.has("minJumpTrim") {
        if m.machine != MachineType::MultiNeedle {
            return Err(p.invalid(
                "minJumpTrim",
                "applies to multi-needle machines only; set machine: multi",
            ));
        }
        m.min_jump_trim = Some(p.req("minJumpTrim", 0.5, 50.0)?);
    }
    let (w, h) = match m.hoop_size() {
        Some(size) => {
            if custom_size {
                return Err(p.invalid("width", "width/height apply to hoop: custom only"));
            }
            size
        }
        None => (
            p.opt("width", s.design.page.width / MM, MIN_SIZE, MAX_SIZE)?,
            p.opt("height", s.design.page.height / MM, MIN_SIZE, MAX_SIZE)?,
        ),
    };
    resize_page(&mut s.design, w * MM, h * MM);
    s.design.machine = m;
    Ok(hoops(&s.design)["current"].clone())
}

/// Sets the Design Page size, moving every object so it keeps its offset from the page centre.
fn resize_page(d: &mut Design, width: f64, height: f64) {
    let before = d.page.center();
    d.page.width = width;
    d.page.height = height;
    let after = d.page.center();
    let t = Transform::translation(after.x - before.x, after.y - before.y);
    if after != before {
        for o in &mut d.objects {
            o.apply(&t);
        }
    }
}

fn hoops(d: &Design) -> Value {
    let m = &d.machine;
    json!({
        "machines": ["single", "multi"],
        "brother": BROTHER_HOOPS.iter().map(|h| json!({
            "id": h.id, "widthMM": h.width, "heightMM": h.height, "label": h.label,
            "multiPosition": h.multi_position, "source": h.source,
        })).collect::<Vec<_>>(),
        "user": m.user_hoops.iter().map(|h| json!({
            "id": user_hoop_key(h.id), "widthMM": h.width, "heightMM": h.height,
            "comment": h.comment,
        })).collect::<Vec<_>>(),
        "current": {
            "machine": m.machine.as_str(),
            "hoop": m.hoop.as_deref().unwrap_or("custom"),
            "rotate90": m.rotate90,
            "pageWidthMM": d.page.width / MM,
            "pageHeightMM": d.page.height / MM,
            "sewingArea": m.sewing_area.as_str(),
            "minJumpTrimMM": m.trim_distance(openstitch_design::Compiler::default().trim_distance) / MM,
            "stitchesPerMinute": m.speed(),
        },
    })
}

/// Thread length and sewing time of a compiled design, for `design.info`.
pub fn estimates(d: &Design, pattern: &StitchPattern) -> Value {
    let e = machine::estimate(pattern, &d.machine);
    let metres = |mm: f64| (mm / 10.0).round() / 100.0;
    json!({
        "needleThreadM": metres(e.needle_total()),
        "bobbinThreadM": metres(e.bobbin_thread),
        "threadPerColorM": e.needle_thread.iter().map(|&mm| metres(mm)).collect::<Vec<_>>(),
        "sewingMinutes": (e.seconds / 6.0).round() / 10.0,
        "stitchesPerMinute": d.machine.speed(),
    })
}

/// The sewing area as exported: the whole Design Page or the stitches' extent, with the point
/// the needle starts from (mm, page coordinates).
/// Parses a user-hoop table (issue #125): for each hoop a line
/// `width_mm,height_mm,width_in10,height_in10,` (the inch tenths and a trailing comma are
/// optional and ignored: the mm pair is authoritative) followed by a name line. A name line may
/// be missing before the next size line or at the end. Blank lines are skipped. Returns
/// (width mm, height mm, name) per hoop.
pub(crate) fn parse_user_hoops(text: &str) -> Result<Vec<(f64, f64, String)>, String> {
    let lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_start_matches('\u{feff}').trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    let size = |l: &str| -> Option<Vec<f64>> {
        let fields: Vec<&str> = l.split(',').map(str::trim).collect();
        let fields = match fields.split_last() {
            Some((&"", rest)) => rest,
            _ => &fields[..],
        };
        if !(2..=4).contains(&fields.len()) {
            return None;
        }
        fields
            .iter()
            .map(|f| f.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect()
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (n, line) = lines[i];
        let v = size(line).ok_or_else(|| {
            format!("line {n}: expected width_mm,height_mm,width_in10,height_in10")
        })?;
        let (width, height) = (v[0], v[1]);
        for (name, mm) in [("width", width), ("height", height)] {
            if !(MIN_SIZE..=MAX_SIZE).contains(&mm) {
                return Err(format!(
                    "line {n}: {name} {mm} mm is outside {MIN_SIZE}–{MAX_SIZE} mm"
                ));
            }
        }
        i += 1;
        let mut comment = String::new();
        if let Some(&(_, next)) = lines.get(i)
            && size(next).is_none()
        {
            comment = next.chars().take(100).collect();
            i += 1;
        }
        out.push((width, height, comment));
    }
    if out.is_empty() {
        return Err("no hoops in the file".into());
    }
    Ok(out)
}

/// What an export reports about the design's size (issue #125). PE-DESIGN 11.50 neither
/// rejects nor promotes a design larger than its Design Page: Save As warns that the data
/// cannot be sent to the machine and saves on Yes, Export writes it silently
/// (`compat/reference/machine/hoop-oversize-100.dst`, a 129 × 105 mm design on a 100 × 100 mm
/// page). OpenStitch also writes it, and says so: `warnings` gets one line when the stitches
/// lie outside the Design Page, `smallestHoop` names the smallest single-position Brother hoop
/// the design fits (either orientation), `hoopMM` is the size written to the PES header.
pub fn export_check(d: &Design, pattern: &StitchPattern) -> Value {
    let b = pattern.bounds();
    let (pw, ph) = (d.page.width, d.page.height);
    // Half a unit: coordinates are rounded to 0.1 mm when written.
    let tol = 0.5;
    let fits_page =
        b.min_x >= -tol && b.min_y >= -tol && b.max_x <= pw + tol && b.max_y <= ph + tol;
    let (dw, dh) = (b.width() / MM, b.height() / MM);
    let smallest = BROTHER_HOOPS
        .iter()
        .filter(|h| !h.multi_position)
        .filter(|h| (dw <= h.width && dh <= h.height) || (dw <= h.height && dh <= h.width))
        .min_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)));
    let mut warnings = Vec::new();
    if !fits_page {
        let hint = smallest.map_or_else(
            || " It is larger than every single-position Brother hoop.".to_string(),
            |h| format!(" The smallest Brother hoop it fits is {}.", h.label),
        );
        warnings.push(format!(
            "The design ({dw:.1} × {dh:.1} mm) does not lie within the {:.1} × {:.1} mm Design \
             Page, so a machine with this hoop may refuse it. It was written anyway, as \
             PE-DESIGN 11 writes it.{hint}",
            pw / MM,
            ph / MM
        ));
    }
    let (hw, hh) = hoop_mm(d);
    json!({
        "fitsPage": fits_page,
        "designMM": [dw, dh],
        "hoopMM": [hw, hh],
        "smallestHoop": smallest.map(|h| h.id),
        "warnings": warnings,
    })
}

/// Hoop size written to the PES header, mm: the selected hoop (rotation applied), else the
/// custom Design Page.
pub fn hoop_mm(d: &Design) -> (u16, u16) {
    let (w, h) = d
        .machine
        .hoop_size()
        .unwrap_or((d.page.width / MM, d.page.height / MM));
    let mm = |v: f64| v.round().clamp(1.0, f64::from(u16::MAX)) as u16;
    (mm(w), mm(h))
}

pub fn sewing_area(d: &Design, pattern: &StitchPattern) -> Value {
    let (w, h, c) = match d.machine.sewing_area {
        SewingArea::DesignPage => (d.page.width, d.page.height, d.page.center()),
        SewingArea::DesignArea => {
            let b = pattern.bounds();
            (
                b.width(),
                b.height(),
                v2((b.min_x + b.max_x) / 2.0, (b.min_y + b.max_y) / 2.0),
            )
        }
    };
    json!({
        "mode": d.machine.sewing_area.as_str(),
        "widthMM": w / MM, "heightMM": h / MM,
        "startXMM": c.x / MM, "startYMM": c.y / MM,
    })
}
