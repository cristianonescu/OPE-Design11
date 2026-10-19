//! Stitch Design Factory: sewing-machine stitch patterns kept with the project
//! (`Design::sewing`), in OpenStitch's documented `openstitch.sewing` format. Export writes that
//! format (with the generated needle points), an SVG drawing, or experimental PMV.

use crate::commands::{CommandSpec, Effect, opt, read_file, req, spec};
use crate::params::{CommandError, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_design::sewing::{self, MAX_DOCUMENT, SewingStitch};
use serde_json::{Value, json};

/// Most sewing stitch patterns one project may carry.
pub const MAX_SEWING: usize = 256;

fn define(s: &mut EngineState, p: &Params, stitch: SewingStitch) -> Result<Value, CommandError> {
    let existing = s.design.sewing.iter().position(|q| q.id == stitch.id);
    let id = stitch.id.clone();
    match existing {
        Some(i) => s.design.sewing[i] = stitch,
        None => {
            if s.design.sewing.len() >= MAX_SEWING {
                return Err(p.failed(format!(
                    "a project carries at most {MAX_SEWING} sewing stitch patterns"
                )));
            }
            s.design.sewing.push(stitch);
        }
    }
    Ok(json!({ "id": id, "replaced": existing.is_some() }))
}

fn find<'a>(s: &'a EngineState, p: &Params) -> Result<&'a SewingStitch, CommandError> {
    let id = p.string("id", None)?;
    s.design
        .sewing
        .iter()
        .find(|q| q.id == id)
        .ok_or_else(|| p.not_found(format!("sewing stitch pattern '{id}'")))
}

fn index(s: &EngineState, p: &Params) -> Result<usize, CommandError> {
    let id = p.string("id", None)?;
    s.design
        .sewing
        .iter()
        .position(|q| q.id == id)
        .ok_or_else(|| p.not_found(format!("sewing stitch pattern '{id}'")))
}

fn summary(q: &SewingStitch) -> Value {
    json!({
        "id": q.id, "name": q.name, "kind": q.kind.id(), "widthMM": q.width,
        "lengthMM": (q.length() * 1000.0).round() / 1000.0, "needles": q.needles().len(),
    })
}

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "sewing.define",
            "Add a sewing-machine stitch pattern (openstitch.sewing document) to the project, replacing one with the same id",
            Effect::Edit,
            vec![req(
                "stitch",
                "object",
                "The document: {format, version, kind, id, name, width, points | profile | outline | cells, …}",
            )],
            |s, p| {
                let text = match p.get("stitch") {
                    Some(Value::String(t)) => t.clone(),
                    Some(v @ Value::Object(_)) => v.to_string(),
                    _ => return Err(p.invalid("stitch", "must be an openstitch.sewing document")),
                };
                let stitch = sewing::parse(&text).map_err(|e| p.invalid("stitch", e))?;
                define(s, p, stitch)
            },
        ),
        spec(
            "sewing.delete",
            "Remove a sewing stitch pattern from the project",
            Effect::Edit,
            vec![req("id", "string", "Pattern id")],
            |s, p| {
                let id = p.string("id", None)?;
                let before = s.design.sewing.len();
                s.design.sewing.retain(|q| q.id != id);
                if s.design.sewing.len() == before {
                    return Err(p.not_found(format!("sewing stitch pattern '{id}'")));
                }
                Ok(Value::Null)
            },
        ),
        spec(
            "sewing.swap",
            "Swap the start and end of a sewing stitch pattern: the same needle points sewn in reverse order",
            Effect::Edit,
            vec![req("id", "string", "Pattern id")],
            |s, p| {
                let i = index(s, p)?;
                let q = &mut s.design.sewing[i];
                q.swap();
                Ok(json!({ "id": q.id, "swapped": q.swapped }))
            },
        ),
        spec(
            "sewing.duplicate",
            "Follow a sewing stitch pattern with a copy of itself where it ends along the feed (not decorative satin)",
            Effect::Edit,
            vec![req("id", "string", "Pattern id")],
            |s, p| {
                let i = index(s, p)?;
                let d = s.design.sewing[i].duplicate().map_err(|e| p.failed(e))?;
                let out = summary(&d);
                s.design.sewing[i] = d;
                Ok(out)
            },
        ),
        spec(
            "sewing.list",
            "The project's sewing stitch patterns",
            Effect::Read,
            vec![],
            |s, _| Ok(Value::Array(s.design.sewing.iter().map(summary).collect())),
        ),
        spec(
            "sewing.get",
            "A sewing stitch pattern as its document plus the generated needle points (mm)",
            Effect::Read,
            vec![req("id", "string", "Pattern id")],
            |s, p| {
                let q = find(s, p)?;
                serde_json::from_str(&q.to_json(true)).map_err(|e| p.failed(e.to_string()))
            },
        ),
        spec(
            "sewing.import",
            "Read an openstitch.sewing file into the project",
            Effect::Edit,
            vec![req("path", "string", "Sewing stitch file (.json)")],
            |s, p| {
                let (_, data) = read_file(s, p, MAX_DOCUMENT)?;
                let text =
                    String::from_utf8(data).map_err(|_| p.failed("the file is not UTF-8 text"))?;
                let stitch = sewing::parse(&text).map_err(|e| p.failed(e))?;
                define(s, p, stitch)
            },
        ),
        spec(
            "sewing.export",
            "Write a sewing stitch pattern: openstitch.sewing JSON (with needle points), SVG drawing, or PMV (experimental, at most 100 needle points)",
            Effect::File,
            vec![
                req("id", "string", "Pattern id"),
                req("path", "string", "Output file"),
                opt(
                    "format",
                    "json|svg|pmv",
                    "Format (default from the extension, else json)",
                ),
                opt(
                    "repeats",
                    "int 1-20",
                    "SVG: repeats drawn along the feed (default 3)",
                ),
            ],
            |s, p| {
                let q = find(s, p)?.clone();
                let raw = p.string("path", None)?;
                let ext = std::path::Path::new(&raw)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_ascii_lowercase);
                let format = match p.get("format") {
                    Some(_) => p.string("format", None)?,
                    None => ext.unwrap_or_else(|| "json".into()),
                };
                let repeats = p.int("repeats", Some(3), 1, 20)? as usize;
                let bytes = match format.as_str() {
                    "json" => (q.to_json(true) + "\n").into_bytes(),
                    "svg" => sewing::svg(&q, repeats).into_bytes(),
                    "pmv" => sewing::pmv(&q).map_err(|e| p.failed(e))?,
                    "pmp" | "psf" => {
                        return Err(p.failed(format!(
                            "{format} export is not available: Brother's format is undocumented"
                        )));
                    }
                    _ => return Err(p.invalid("format", "must be json, svg or pmv")),
                };
                let path = s.resolve(&raw, Access::Write, &p.command)?;
                std::fs::write(&path, bytes).map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({ "path": path.display().to_string(), "format": format }))
            },
        ),
    ]
}
