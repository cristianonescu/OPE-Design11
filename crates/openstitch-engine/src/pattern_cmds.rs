//! User stitch patterns (the Programmable Stitch Creator's output): define, delete, read,
//! import and export `openstitch.pattern` documents. A design carries the user patterns it
//! uses, so a project sews the same on every machine; bundled patterns cannot be redefined.

use crate::commands::{CommandSpec, Effect, read_file, req, spec};
use crate::params::{CommandError, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_design::patterns::{self, MAX_DOCUMENT, Pattern, PatternKind};
use serde_json::{Value, json};

/// Most user patterns one design may carry.
pub const MAX_USER_PATTERNS: usize = 256;

pub(crate) fn kind_id(k: PatternKind) -> &'static str {
    match k {
        PatternKind::Motif => "motif",
        PatternKind::Fill => "fill",
        PatternKind::Decorative => "decorative",
    }
}

fn kind_param(p: &Params) -> Result<PatternKind, CommandError> {
    match p.string("kind", None)?.as_str() {
        "motif" => Ok(PatternKind::Motif),
        "fill" => Ok(PatternKind::Fill),
        "decorative" => Ok(PatternKind::Decorative),
        _ => Err(p.invalid("kind", "must be motif, fill or decorative")),
    }
}

/// Adds `pattern` to the design (replacing a user pattern of the same kind and id).
fn define(s: &mut EngineState, p: &Params, pattern: Pattern) -> Result<Value, CommandError> {
    if patterns::is_bundled(&pattern.id) {
        return Err(p.failed(format!(
            "'{}' is a bundled pattern id; choose another id",
            pattern.id
        )));
    }
    let existing = s
        .design
        .patterns
        .iter()
        .position(|q| q.kind == pattern.kind && q.id == pattern.id);
    let (kind, id) = (kind_id(pattern.kind), pattern.id.clone());
    match existing {
        Some(i) => s.design.patterns[i] = pattern,
        None => {
            if s.design.patterns.len() >= MAX_USER_PATTERNS {
                return Err(p.failed(format!(
                    "a design carries at most {MAX_USER_PATTERNS} user patterns"
                )));
            }
            s.design.patterns.push(pattern);
        }
    }
    Ok(json!({ "kind": kind, "id": id, "replaced": existing.is_some() }))
}

fn parse_value(p: &Params, key: &str) -> Result<Pattern, CommandError> {
    let text = match p.get(key) {
        None => return Err(p.invalid(key, "is required")),
        Some(Value::String(s)) => s.clone(),
        Some(v @ Value::Object(_)) => v.to_string(),
        Some(_) => return Err(p.invalid(key, "must be an openstitch.pattern document")),
    };
    patterns::parse(&text).map_err(|e| p.invalid(key, e))
}

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "pattern.define",
            "Add a user stitch pattern (openstitch.pattern document) to the design, replacing one with the same kind and id",
            Effect::Edit,
            vec![req(
                "pattern",
                "object",
                "The pattern document: {format, version, kind, id, name, rowShift?, paths}",
            )],
            |s, p| {
                let pattern = parse_value(p, "pattern")?;
                define(s, p, pattern)
            },
        ),
        spec(
            "pattern.delete",
            "Remove a user pattern from the design (refused while objects use it)",
            Effect::Edit,
            vec![
                req("kind", "motif|fill|decorative", "Pattern kind"),
                req("id", "string", "Pattern id"),
            ],
            |s, p| {
                let kind = kind_param(p)?;
                let id = p.string("id", None)?;
                let Some(i) = s
                    .design
                    .patterns
                    .iter()
                    .position(|q| q.kind == kind && q.id == id)
                else {
                    return Err(p.not_found(format!("user pattern '{id}'")));
                };
                let users = s.design.pattern_users(kind, &id);
                if !users.is_empty() {
                    return Err(p.failed(format!("pattern '{id}' is used by objects {users:?}")));
                }
                s.design.patterns.remove(i);
                Ok(Value::Null)
            },
        ),
        spec(
            "pattern.get",
            "A pattern (user or bundled) as an openstitch.pattern document",
            Effect::Read,
            vec![
                req("kind", "motif|fill|decorative", "Pattern kind"),
                req("id", "string", "Pattern id"),
            ],
            |s, p| {
                let kind = kind_param(p)?;
                let id = p.string("id", None)?;
                let pattern = s
                    .design
                    .find_pattern(kind, &id)
                    .ok_or_else(|| p.not_found(format!("pattern '{id}'")))?;
                let doc: Value = serde_json::from_str(&pattern.to_json())
                    .map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({
                    "user": s.design.user_pattern(kind, &id).is_some(),
                    "pattern": doc,
                }))
            },
        ),
        spec(
            "pattern.import",
            "Read an openstitch.pattern file and add it to the design",
            Effect::Edit,
            vec![req("path", "string", "Pattern file (.json)")],
            |s, p| {
                let (_, data) = read_file(s, p, MAX_DOCUMENT)?;
                let text =
                    String::from_utf8(data).map_err(|_| p.failed("the file is not UTF-8 text"))?;
                let pattern = patterns::parse(&text).map_err(|e| p.failed(e))?;
                define(s, p, pattern)
            },
        ),
        spec(
            "pattern.export",
            "Write a pattern (user or bundled) to an openstitch.pattern file",
            Effect::File,
            vec![
                req("kind", "motif|fill|decorative", "Pattern kind"),
                req("id", "string", "Pattern id"),
                req("path", "string", "Output file (.json)"),
            ],
            |s, p| {
                let kind = kind_param(p)?;
                let id = p.string("id", None)?;
                let json = s
                    .design
                    .find_pattern(kind, &id)
                    .ok_or_else(|| p.not_found(format!("pattern '{id}'")))?
                    .to_json();
                let path = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
                std::fs::write(&path, json + "\n").map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({ "path": path.display().to_string() }))
            },
        ),
    ]
}
