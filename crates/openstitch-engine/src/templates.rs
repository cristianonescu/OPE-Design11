//! Templates and the Template Wizard (PE-DESIGN Tutorial 2, Template Feature): a template is a
//! design whose Name Drop texts are its editable slots. Templates are `.ostemplate` files
//! (a design plus a name and a category); OpenStitch ships a few of its own, and
//! `template.list` also reads a folder of the user's, one subfolder per category.

use crate::CommandSpec;
use crate::commands::{Effect, MM, opt, req, spec};
use crate::lettering::{apply_row, name_drop_columns};
use crate::params::{CommandError, Params};
use crate::project;
use crate::sandbox::Access;
use openstitch_design::Design;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const FORMAT: &str = "openstitch.template";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "ostemplate";
/// Most template files `template.list` reads from a folder.
const MAX_LISTED: usize = 500;

/// OpenStitch's own templates (made with the engine's commands; see `examples/templates/`).
static BUILTIN: [(&str, &str); 4] = [
    (
        "team-badge",
        include_str!("../templates/team-badge.ostemplate"),
    ),
    ("name-tag", include_str!("../templates/name-tag.ostemplate")),
    (
        "initials-ring",
        include_str!("../templates/initials-ring.ostemplate"),
    ),
    ("baby-bib", include_str!("../templates/baby-bib.ostemplate")),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Template {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub category: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub design: Design,
}

/// A template file's template, checked like a design file.
pub fn decode(data: &[u8]) -> Result<Template, String> {
    if data.len() > project::MAX_SIZE {
        return Err("file too large".into());
    }
    #[derive(Deserialize)]
    struct Header {
        format: String,
        version: u32,
    }
    let h: Header =
        serde_json::from_slice(data).map_err(|_| "not an OpenStitch template".to_string())?;
    if h.format != FORMAT {
        return Err(format!("unexpected format '{}'", h.format));
    }
    if h.version > VERSION {
        return Err(format!(
            "template version {} is newer than supported ({VERSION})",
            h.version
        ));
    }
    let t: Template = serde_json::from_slice(data).map_err(|e| format!("damaged template: {e}"))?;
    project::validate(&t.design).map_err(|e| e.to_string())?;
    if !(1..=64).contains(&t.name.chars().count())
        || !(1..=64).contains(&t.category.chars().count())
    {
        return Err("template name and category must be 1-64 characters".into());
    }
    if name_drop_columns(&t.design).is_empty() {
        return Err("the template has no Name Drop text to edit".into());
    }
    Ok(t)
}

/// What the wizard lists about a template.
fn summary(id: &str, t: &Template, source: &str) -> Value {
    json!({
        "id": id, "name": t.name, "category": t.category, "description": t.description,
        "source": source, "slots": slots(&t.design),
        "widthMM": t.design.page.width / MM, "heightMM": t.design.page.height / MM,
    })
}

/// The editable texts: object id, label (or "Text N") and current text.
fn slots(d: &Design) -> Vec<Value> {
    name_drop_columns(d)
        .into_iter()
        .enumerate()
        .map(|(k, i)| {
            let o = &d.objects[i];
            let t = o.text.as_ref().expect("text");
            let label = t
                .name_drop
                .as_ref()
                .and_then(|n| n.label.clone())
                .unwrap_or_else(|| format!("Text {}", k + 1));
            json!({ "id": o.id, "label": label, "text": t.text })
        })
        .collect()
}

fn builtin(id: &str) -> Option<Template> {
    BUILTIN
        .iter()
        .find(|(k, _)| *k == id)
        .and_then(|(_, data)| decode(data.as_bytes()).ok())
}

/// A short single-line string parameter.
fn label_param(p: &Params, key: &str, default: Option<&str>) -> Result<String, CommandError> {
    let v = p.string(key, default)?.trim().to_string();
    if !(1..=64).contains(&v.chars().count()) || v.chars().any(char::is_control) {
        return Err(p.invalid(key, "must be 1-64 characters"));
    }
    Ok(v)
}

pub(crate) fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "template.save",
            "Save the design as a template: its Name Drop texts become the editable slots",
            Effect::File,
            vec![
                req("path", "string", "File path (.ostemplate)"),
                opt("name", "string", "Template name (default the design name)"),
                opt("category", "string", "Category (default \"My Templates\")"),
                opt("description", "string", "One line shown in the wizard"),
            ],
            |s, p| {
                if name_drop_columns(&s.design).is_empty() {
                    return Err(p.failed(
                        "the design has no Name Drop text; give the editable texts the Name Drop attribute first",
                    ));
                }
                let name = label_param(p, "name", Some(&s.design.name.clone()))?;
                let category = label_param(p, "category", Some("My Templates"))?;
                let description = p.string("description", Some(""))?;
                if description.chars().count() > 200 {
                    return Err(p.invalid("description", "at most 200 characters"));
                }
                let t = Template {
                    format: FORMAT.into(),
                    version: VERSION,
                    name,
                    category,
                    description,
                    design: s.design.clone(),
                };
                let path = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
                let bytes = serde_json::to_vec_pretty(&t).map_err(|e| p.failed(e.to_string()))?;
                std::fs::write(&path, bytes).map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({ "path": path.display().to_string(), "slots": slots(&s.design) }))
            },
        ),
        spec(
            "template.list",
            "Templates for the Template Wizard: OpenStitch's own, plus .ostemplate files in a folder (one subfolder per category)",
            Effect::Read,
            vec![opt(
                "dir",
                "string",
                "Template folder; files directly in it use their own category",
            )],
            |s, p| {
                let mut out: Vec<Value> = BUILTIN
                    .iter()
                    .filter_map(|(id, data)| {
                        decode(data.as_bytes())
                            .ok()
                            .map(|t| summary(&format!("builtin:{id}"), &t, "builtin"))
                    })
                    .collect();
                let mut skipped = Vec::new();
                if p.has("dir") {
                    let dir = s.resolve(&p.string("dir", None)?, Access::Read, &p.command)?;
                    let mut files = Vec::new();
                    let entries = std::fs::read_dir(&dir)
                        .map_err(|_| p.failed(format!("cannot read {}", dir.display())))?;
                    for e in entries.flatten().take(MAX_LISTED) {
                        let path = e.path();
                        if path.is_dir() {
                            if let Ok(sub) = std::fs::read_dir(&path) {
                                files.extend(sub.flatten().take(MAX_LISTED).map(|f| f.path()));
                            }
                        } else {
                            files.push(path);
                        }
                    }
                    files.retain(|f| {
                        f.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
                    });
                    files.sort();
                    for f in files.into_iter().take(MAX_LISTED) {
                        let shown = f.display().to_string();
                        // Each file is checked through the sandbox like any other read.
                        let read = s
                            .resolve(&shown, Access::Read, &p.command)
                            .map_err(|e| e.to_string())
                            .and_then(|path| {
                                let m = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                                if m.len() > project::MAX_SIZE as u64 {
                                    return Err("file too large".into());
                                }
                                std::fs::read(&path).map_err(|e| e.to_string())
                            })
                            .and_then(|data| decode(&data));
                        match read {
                            Ok(t) => out.push(summary(&shown, &t, "file")),
                            Err(reason) => skipped.push(json!({ "path": shown, "reason": reason })),
                        }
                    }
                }
                Ok(json!({ "templates": out, "skipped": skipped }))
            },
        ),
        spec(
            "template.open",
            "Open a template as the design (Template Wizard step 2); returns its slots",
            Effect::Edit,
            vec![req(
                "template",
                "string",
                "builtin:<id> from template.list, or a .ostemplate path",
            )],
            |s, p| {
                let which = p.string("template", None)?;
                let t = match which.strip_prefix("builtin:") {
                    Some(id) => builtin(id)
                        .ok_or_else(|| p.not_found(format!("built-in template '{id}'")))?,
                    None => {
                        let path = s.resolve(&which, Access::Read, &p.command)?;
                        let meta = std::fs::metadata(&path)
                            .map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
                        if meta.len() > project::MAX_SIZE as u64 {
                            return Err(p.failed("file too large"));
                        }
                        let data = std::fs::read(&path)
                            .map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
                        decode(&data).map_err(|e| p.failed(e))?
                    }
                };
                s.design = t.design.clone();
                Ok(json!({ "name": t.name, "category": t.category, "slots": slots(&s.design) }))
            },
        ),
        spec(
            "template.fill",
            "Template Wizard step 3: set the slots' texts, by label or in order",
            Effect::Edit,
            vec![req(
                "values",
                "{label: string} | [string]",
                "New texts: an object keyed by slot label (others keep their text), or one per slot in order",
            )],
            |s, p| {
                let current = slots(&s.design);
                if current.is_empty() {
                    return Err(p.failed("the design has no slots (Name Drop texts)"));
                }
                let mut values: Vec<String> = current
                    .iter()
                    .map(|v| v["text"].as_str().unwrap_or_default().to_string())
                    .collect();
                match p.get("values") {
                    Some(Value::Object(map)) => {
                        for (label, v) in map {
                            let k = current
                                .iter()
                                .position(|c| c["label"] == *label)
                                .ok_or_else(|| p.invalid("values", format!("no slot '{label}'")))?;
                            values[k] = v
                                .as_str()
                                .ok_or_else(|| p.invalid("values", "texts must be strings"))?
                                .to_string();
                        }
                    }
                    Some(Value::Array(a)) => {
                        if a.len() != values.len() {
                            return Err(p.invalid(
                                "values",
                                format!("needs {} texts, one per slot", values.len()),
                            ));
                        }
                        for (k, v) in a.iter().enumerate() {
                            values[k] = v
                                .as_str()
                                .ok_or_else(|| p.invalid("values", "texts must be strings"))?
                                .to_string();
                        }
                    }
                    _ => return Err(p.invalid("values", "must be an object or an array")),
                }
                apply_row(s, p, &values)?;
                Ok(json!({ "slots": slots(&s.design) }))
            },
        ),
    ]
}
