//! User settings kept outside designs (PE-DESIGN's "Specifying/Saving Custom Sewing
//! Attributes"): named sewing-attribute sets and user thread charts. A front end that wants
//! them to persist gives the session a settings file ([`crate::Session::with_user_settings`]);
//! every settings command writes it atomically before the change takes effect.

use crate::commands::{
    CommandSpec, Effect, IDS, default_fill, default_line, indexes, opt, req, spec,
};
use crate::params::{CommandError, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_core::{
    HUS_CHART, JEF_CHART, PEC_CHART, SEW_CHART, SHV_CHART, Thread, color_distance,
};
use openstitch_design::{LineSewing, RegionSewing};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub const MAX_SETS: usize = 500;
pub const MAX_CHARTS: usize = 100;
pub const MAX_CHART_COLORS: usize = 2_000;
pub const MAX_NAME: usize = 100;
/// Largest settings or CSV file read, bytes.
pub const MAX_FILE: u64 = 4 << 20;

/// A saved group of sewing attributes. Threads are kept so a set can carry colours too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeSet {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<RegionSewing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineSewing>,
}

/// A user thread chart: threads with code (`catalog_number`), brand and description (`name`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadChart {
    pub name: String,
    pub threads: Vec<Thread>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    #[serde(default)]
    pub attribute_sets: Vec<AttributeSet>,
    #[serde(default)]
    pub thread_charts: Vec<ThreadChart>,
    /// Options ▸ "Optimize Entry/Exit Points": a shape added to the design is entered where it
    /// comes closest to the shape sewn before it, which leaves at that place.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optimize_entry_exit: bool,
}

/// File format marker of the settings file.
pub const FORMAT: &str = "openstitch.user";

impl UserSettings {
    /// Reads a settings file; a missing file gives empty settings.
    pub fn load(path: &Path) -> Result<UserSettings, String> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(UserSettings::default());
            }
            Err(e) => return Err(e.to_string()),
        };
        if meta.len() > MAX_FILE {
            return Err("user settings file is too large".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        Self::decode(&bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<UserSettings, String> {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err("not an OpenStitch user settings file".into());
        }
        let s: UserSettings =
            serde_json::from_value(v.get("settings").cloned().unwrap_or(Value::Null))
                .map_err(|e| e.to_string())?;
        if s.attribute_sets.len() > MAX_SETS
            || s.thread_charts.len() > MAX_CHARTS
            || s.thread_charts
                .iter()
                .any(|c| c.threads.len() > MAX_CHART_COLORS)
        {
            return Err("user settings exceed the limits".into());
        }
        Ok(s)
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&json!({ "format": FORMAT, "version": 1, "settings": self }))
            .unwrap_or_default()
    }

    /// Writes the file atomically (temporary file, then rename).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.encode()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    fn chart_index(&self, name: &str) -> Option<usize> {
        self.thread_charts.iter().position(|c| c.name == name)
    }
}

/// The per-user settings file: `~/Library/Application Support/OpenStitch/user.json` on macOS,
/// `%APPDATA%\OpenStitch\user.json` on Windows, `$XDG_CONFIG_HOME/openstitch/user.json` (or
/// `~/.config/openstitch/user.json`) elsewhere.
pub fn default_path() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support/OpenStitch"))
    } else if cfg!(windows) {
        var("APPDATA").map(|a| a.join("OpenStitch"))
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".config")))
            .map(|c| c.join("openstitch"))
    };
    dir.map(|d| d.join("user.json"))
}

fn name(p: &Params, key: &str) -> Result<String, CommandError> {
    let n = p.string(key, None)?;
    let n = n.trim().to_string();
    if n.is_empty() || n.chars().count() > MAX_NAME || n.chars().any(char::is_control) {
        return Err(p.invalid(key, format!("must be 1-{MAX_NAME} printable characters")));
    }
    Ok(n)
}

fn chart<'a>(s: &'a mut EngineState, p: &Params) -> Result<&'a mut ThreadChart, CommandError> {
    let n = p.string("chart", None)?;
    let i = s
        .user
        .chart_index(&n)
        .ok_or_else(|| p.not_found(format!("thread chart '{n}'")))?;
    Ok(&mut s.user.thread_charts[i])
}

fn text(p: &Params, key: &str) -> Result<Option<String>, CommandError> {
    if !p.has(key) {
        return Ok(None);
    }
    let t = p.string(key, None)?;
    if t.chars().count() > MAX_NAME || t.chars().any(char::is_control) {
        return Err(p.invalid(
            key,
            format!("must be at most {MAX_NAME} printable characters"),
        ));
    }
    Ok(Some(t))
}

/// A chart thread from `color` and the optional `code`, `brand`, `description`.
fn chart_thread(p: &Params, base: Option<Thread>) -> Result<Thread, CommandError> {
    let mut t = match (p.has("color"), base) {
        (true, _) => {
            let c = p.string("color", None)?;
            Thread::from_hex(c.trim()).ok_or_else(|| p.invalid("color", "must be #rrggbb"))?
        }
        (false, Some(b)) => b,
        (false, None) => return Err(p.invalid("color", "is required")),
    };
    if let Some(code) = text(p, "code")? {
        if !code.is_empty() && !code.chars().all(|c| c.is_ascii_digit()) {
            return Err(p.invalid("code", "must be digits only"));
        }
        t.catalog_number = Some(Cow::Owned(code));
    }
    if let Some(b) = text(p, "brand")? {
        t.brand = Some(Cow::Owned(b));
    }
    if let Some(d) = text(p, "description")? {
        t.name = Some(Cow::Owned(d));
    }
    Ok(t)
}

/// Columns of OpenStitch's thread-chart CSV, in order (docs/FILE_FORMATS.md, "Thread chart
/// CSV"). PE-DESIGN 11.50 has no thread-chart CSV: its user charts are binary `.btc2` files in
/// the program folder and are only imported through its own dialog (issue #136), so this
/// schema is OpenStitch's own.
pub const CSV_COLUMNS: [&str; 6] = ["code", "brand", "description", "R", "G", "B"];

/// The fields of one CSV record on one line. A field in double quotes may hold commas, and `""`
/// inside it is one quote (RFC 4180); records do not span lines. Fields are trimmed.
fn csv_fields(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let (mut quoted, mut was_quoted) = (false, false);
    loop {
        match chars.next() {
            None if quoted => return Err("a quoted field is not closed".into()),
            None => {
                out.push(field.trim().to_string());
                return Ok(out);
            }
            Some('"') if quoted => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            }
            Some('"') if !was_quoted && field.trim().is_empty() => {
                field.clear();
                (quoted, was_quoted) = (true, true);
            }
            Some(',') if !quoted => {
                out.push(field.trim().to_string());
                field.clear();
                was_quoted = false;
            }
            Some(c) => field.push(c),
        }
    }
}

/// Whether `line` is the column-name row `code,brand,description,R,G,B` (any case).
fn csv_header(fields: &[String]) -> bool {
    fields.len() == CSV_COLUMNS.len()
        && fields
            .iter()
            .zip(CSV_COLUMNS)
            .all(|(f, c)| f.eq_ignore_ascii_case(c))
}

/// One field written for the CSV: quoted when it holds a comma, a quote or edge spaces;
/// control characters (line breaks) dropped, as the reader would.
fn csv_field(s: &str) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    if s.contains([',', '"']) || s.trim() != s {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// The CSV record of a thread: `code,brand,description,R,G,B`.
fn csv_record(t: &Thread) -> String {
    format!(
        "{},{},{},{},{},{}",
        csv_field(t.catalog_number.as_deref().unwrap_or("")),
        csv_field(t.brand.as_deref().unwrap_or("")),
        csv_field(t.name.as_deref().unwrap_or("")),
        t.red,
        t.green,
        t.blue
    )
}

/// A whole chart as CSV: a `#` comment line naming it, the column row, one record per colour;
/// CRLF line ends (RFC 4180), UTF-8 without BOM.
pub fn chart_csv<'a>(name: &str, threads: impl IntoIterator<Item = &'a Thread>) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    let mut out = format!(
        "# OpenStitch thread chart: {name}\r\n{}\r\n",
        CSV_COLUMNS.join(",")
    );
    for t in threads {
        out.push_str(&csv_record(t));
        out.push_str("\r\n");
    }
    out
}

/// Brother chart entries PE-DESIGN 11.50 matches a colour to: 1–61 (the owner's swatch capture
/// never picked 62–64; see `openstitch_core` and issue #119).
const BROTHER_BASIC: usize = 61;

/// Thread charts built into OpenStitch: id, display name, entries. The index of an entry is the
/// format's colour number; `None` marks a number the format leaves unused.
pub fn builtin_charts() -> [(&'static str, &'static str, &'static [Option<Thread>]); 5] {
    [
        ("brother", "Brother (PEC / PES chart)", &PEC_CHART),
        ("janomeJef", "Janome (JEF chart)", &JEF_CHART),
        ("janomeSew", "Janome (SEW chart)", &SEW_CHART),
        ("husqvarnaHus", "Husqvarna Viking (HUS chart)", &HUS_CHART),
        ("husqvarnaShv", "Husqvarna Viking (SHV chart)", &SHV_CHART),
    ]
}

/// One CSV line `code,brand,description,R,G,B` (OpenStitch's thread-chart CSV, [`CSV_COLUMNS`]).
fn csv_thread(line: &str) -> Result<Thread, String> {
    let f = csv_fields(line)?;
    let f: Vec<&str> = f.iter().map(String::as_str).collect();
    if f.len() != 6 {
        return Err("expected 6 fields: code,brand,description,R,G,B".into());
    }
    let c = |s: &str| s.parse::<u8>().map_err(|_| format!("'{s}' is not 0-255"));
    let (r, g, b) = (c(f[3])?, c(f[4])?, c(f[5])?);
    if !f[0].chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("code '{}' must be digits", f[0]));
    }
    let clip = |s: &str| {
        s.chars()
            .filter(|c| !c.is_control())
            .take(MAX_NAME)
            .collect::<String>()
    };
    let mut t = Thread::rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b));
    t.catalog_number = (!f[0].is_empty()).then(|| Cow::Owned(f[0].to_string()));
    t.brand = (!f[1].is_empty()).then(|| Cow::Owned(clip(f[1])));
    t.name = (!f[2].is_empty()).then(|| Cow::Owned(clip(f[2])));
    Ok(t)
}

fn describe_thread(t: &Thread) -> Value {
    json!({
        "color": t.hex(),
        "code": t.catalog_number,
        "brand": t.brand,
        "description": t.name,
    })
}

pub fn commands() -> Vec<CommandSpec> {
    let mut v = attribute_commands();
    v.extend(chart_commands());
    v.extend(option_commands());
    v
}

fn options_json(u: &UserSettings) -> Value {
    json!({ "optimizeEntryExit": u.optimize_entry_exit })
}

/// Persistent options (PE-DESIGN's Options dialog) kept in the user settings.
fn option_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "options.get",
            "The persistent user options",
            Effect::Read,
            vec![],
            |s, _| Ok(options_json(&s.user)),
        ),
        spec(
            "options.set",
            "Change persistent user options (user settings)",
            Effect::Settings,
            vec![opt(
                "optimizeEntryExit",
                "bool",
                "Optimize Entry/Exit Points: shapes added later are entered where they come closest to the shape sewn before (default off)",
            )],
            |s, p| {
                let u = &mut s.user;
                u.optimize_entry_exit = p.bool("optimizeEntryExit", Some(u.optimize_entry_exit))?;
                Ok(options_json(u))
            },
        ),
    ]
}

fn attribute_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "attributes.save",
            "Save an object's sewing attributes under a name (user settings; same name overwrites)",
            Effect::Settings,
            vec![
                req("name", "string", "Set name"),
                req("from", "int", "Object to take the attributes from"),
            ],
            |s, p| {
                let n = name(p, "name")?;
                let id = p.int("from", None, 0, i64::MAX)? as u64;
                let o = s
                    .design
                    .index_of(id)
                    .map(|i| &s.design.objects[i])
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                if !o.is_outline() {
                    return Err(p.failed(format!("object {id} is a stitch pattern")));
                }
                // Positions belong to the object's outline, not to the set.
                let region = o.region.clone().map(|mut r| {
                    r.entry = None;
                    r.exit = None;
                    r
                });
                let line = o.line.clone().map(|mut l| {
                    l.entry = None;
                    l
                });
                let set = AttributeSet {
                    name: n.clone(),
                    region,
                    line,
                };
                let sets = &mut s.user.attribute_sets;
                match sets.iter().position(|x| x.name == n) {
                    Some(i) => sets[i] = set,
                    None if sets.len() >= MAX_SETS => {
                        return Err(p.failed(format!("at most {MAX_SETS} attribute sets")));
                    }
                    None => sets.push(set),
                }
                Ok(json!({ "name": n }))
            },
        ),
        spec(
            "attributes.list",
            "Saved sewing-attribute sets",
            Effect::Read,
            vec![],
            |s, _| {
                Ok(Value::Array(
                    s.user
                        .attribute_sets
                        .iter()
                        .map(|a| {
                            json!({
                                "name": a.name,
                                "region": a.region.as_ref().map(|r| serde_json::to_value(r).unwrap_or(Value::Null)),
                                "line": a.line.as_ref().map(|l| serde_json::to_value(l).unwrap_or(Value::Null)),
                            })
                        })
                        .collect(),
                ))
            },
        ),
        spec(
            "attributes.apply",
            "Apply a saved attribute set to objects (their colours stay unless includeColors)",
            Effect::Edit,
            vec![
                req("name", "string", "Set name"),
                IDS,
                opt(
                    "includeColors",
                    "bool",
                    "Also apply the set's threads (default false)",
                ),
            ],
            |s, p| {
                let n = p.string("name", None)?;
                let set = s
                    .user
                    .attribute_sets
                    .iter()
                    .find(|a| a.name == n)
                    .cloned()
                    .ok_or_else(|| p.not_found(format!("attribute set '{n}'")))?;
                let colours = p.bool("includeColors", Some(false))?;
                for i in indexes(p, &s.design)? {
                    let o = &mut s.design.objects[i];
                    if !o.is_outline() {
                        return Err(p.failed(format!("object {} is a stitch pattern", o.id)));
                    }
                    if o.has_closed_contour() {
                        let old = o.region.clone();
                        o.region = set.region.clone().map(|mut r| {
                            if !colours {
                                r.thread =
                                    old.as_ref().map_or_else(default_fill, |x| x.thread.clone());
                            }
                            if let Some(x) = &old {
                                r.entry = x.entry;
                                r.exit = x.exit;
                            }
                            r
                        });
                        if o.region.is_none() {
                            o.hole_sewing = false;
                            o.hole_inner = None;
                        }
                    }
                    let old = o.line.clone();
                    o.line = set.line.clone().map(|mut l| {
                        if !colours {
                            l.thread = old.as_ref().map_or_else(default_line, |x| x.thread.clone());
                        }
                        l.entry = old.as_ref().and_then(|x| x.entry);
                        l
                    });
                }
                Ok(json!(null))
            },
        ),
        spec(
            "attributes.delete",
            "Delete a saved attribute set",
            Effect::Settings,
            vec![req("name", "string", "Set name")],
            |s, p| {
                let n = p.string("name", None)?;
                let before = s.user.attribute_sets.len();
                s.user.attribute_sets.retain(|a| a.name != n);
                if s.user.attribute_sets.len() == before {
                    return Err(p.not_found(format!("attribute set '{n}'")));
                }
                Ok(json!(null))
            },
        ),
    ]
}

fn chart_commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "threads.charts",
            "User thread charts and their colours",
            Effect::Read,
            vec![],
            |s, _| {
                Ok(Value::Array(
                    s.user
                        .thread_charts
                        .iter()
                        .map(|c| {
                            json!({
                                "name": c.name,
                                "threads": c.threads.iter().map(describe_thread).collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ))
            },
        ),
        spec(
            "threads.createChart",
            "Create an empty user thread chart",
            Effect::Settings,
            vec![req("name", "string", "Chart name")],
            |s, p| {
                let n = name(p, "name")?;
                if s.user.chart_index(&n).is_some() {
                    return Err(p.failed(format!("a chart named '{n}' exists")));
                }
                if s.user.thread_charts.len() >= MAX_CHARTS {
                    return Err(p.failed(format!("at most {MAX_CHARTS} charts")));
                }
                s.user.thread_charts.push(ThreadChart {
                    name: n,
                    threads: Vec::new(),
                });
                Ok(json!(null))
            },
        ),
        spec(
            "threads.renameChart",
            "Rename a user thread chart",
            Effect::Settings,
            vec![
                req("chart", "string", "Chart"),
                req("name", "string", "New name"),
            ],
            |s, p| {
                let n = name(p, "name")?;
                if s.user.chart_index(&n).is_some() {
                    return Err(p.failed(format!("a chart named '{n}' exists")));
                }
                chart(s, p)?.name = n;
                Ok(json!(null))
            },
        ),
        spec(
            "threads.deleteChart",
            "Delete a user thread chart",
            Effect::Settings,
            vec![req("chart", "string", "Chart")],
            |s, p| {
                let n = p.string("chart", None)?;
                let i = s
                    .user
                    .chart_index(&n)
                    .ok_or_else(|| p.not_found(format!("thread chart '{n}'")))?;
                s.user.thread_charts.remove(i);
                Ok(json!(null))
            },
        ),
        spec(
            "threads.addColor",
            "Add a colour to a user thread chart",
            Effect::Settings,
            vec![
                req("chart", "string", "Chart"),
                req("color", "#rrggbb", "Colour"),
                opt("code", "digits", "Thread code"),
                opt("brand", "string", "Brand"),
                opt("description", "string", "Description"),
            ],
            |s, p| {
                let t = chart_thread(p, None)?;
                let c = chart(s, p)?;
                if c.threads.len() >= MAX_CHART_COLORS {
                    return Err(p.failed(format!("at most {MAX_CHART_COLORS} colours per chart")));
                }
                c.threads.push(t);
                Ok(json!({ "index": c.threads.len() - 1 }))
            },
        ),
        spec(
            "threads.editColor",
            "Change a colour of a user thread chart",
            Effect::Settings,
            vec![
                req("chart", "string", "Chart"),
                req("index", "int", "Colour index"),
                opt("color", "#rrggbb", "Colour"),
                opt("code", "digits", "Thread code"),
                opt("brand", "string", "Brand"),
                opt("description", "string", "Description"),
            ],
            |s, p| {
                let c = chart(s, p)?;
                let n = c.threads.len();
                if n == 0 {
                    return Err(p.failed("the chart is empty"));
                }
                let i = p.int("index", None, 0, n as i64 - 1)? as usize;
                c.threads[i] = chart_thread(p, Some(c.threads[i].clone()))?;
                Ok(json!(null))
            },
        ),
        spec(
            "threads.removeColor",
            "Remove a colour from a user thread chart",
            Effect::Settings,
            vec![
                req("chart", "string", "Chart"),
                req("index", "int", "Colour index"),
            ],
            |s, p| {
                let c = chart(s, p)?;
                let n = c.threads.len();
                if n == 0 {
                    return Err(p.failed("the chart is empty"));
                }
                let i = p.int("index", None, 0, n as i64 - 1)? as usize;
                c.threads.remove(i);
                Ok(json!(null))
            },
        ),
        spec(
            "threads.moveColor",
            "Move a colour up or down in a user thread chart",
            Effect::Settings,
            vec![
                req("chart", "string", "Chart"),
                req("index", "int", "Colour index"),
                req("to", "int", "New index"),
            ],
            |s, p| {
                let c = chart(s, p)?;
                let n = c.threads.len();
                if n == 0 {
                    return Err(p.failed("the chart is empty"));
                }
                let i = p.int("index", None, 0, n as i64 - 1)? as usize;
                let to = p.int("to", None, 0, n as i64 - 1)? as usize;
                let t = c.threads.remove(i);
                c.threads.insert(to, t);
                Ok(json!(null))
            },
        ),
        spec(
            "threads.importCsv",
            "Add the colours of a CSV file (code,brand,description,R,G,B per line) to a user chart",
            Effect::Settings,
            vec![req("chart", "string", "Chart"), crate::commands::PATH],
            |s, p| {
                let path = s.resolve(&p.string("path", None)?, Access::Read, &p.command)?;
                let meta = std::fs::metadata(&path).map_err(|e| p.failed(e.to_string()))?;
                if meta.len() > MAX_FILE {
                    return Err(p.failed("the file is too large"));
                }
                let bytes = std::fs::read(&path).map_err(|e| p.failed(e.to_string()))?;
                let text = String::from_utf8_lossy(&bytes);
                let mut threads = Vec::new();
                for (k, line) in text.lines().enumerate() {
                    let line = line.trim_start_matches('\u{feff}').trim();
                    if line.is_empty()
                        || line.starts_with('#')
                        || csv_fields(line).is_ok_and(|f| csv_header(&f))
                    {
                        continue;
                    }
                    threads.push(
                        csv_thread(line).map_err(|e| p.failed(format!("line {}: {e}", k + 1)))?,
                    );
                }
                let c = chart(s, p)?;
                if c.threads.len() + threads.len() > MAX_CHART_COLORS {
                    return Err(p.failed(format!("at most {MAX_CHART_COLORS} colours per chart")));
                }
                let added = threads.len();
                c.threads.extend(threads);
                Ok(json!({ "added": added }))
            },
        ),
        spec(
            "threads.builtinCharts",
            "The thread charts built into OpenStitch (Brother, Janome JEF and SEW, Husqvarna Viking HUS and SHV) with each entry's number, colour, code, brand and name",
            Effect::Read,
            vec![],
            |_, _| {
                Ok(Value::Array(
                    builtin_charts()
                        .iter()
                        .map(|(id, name, chart)| {
                            let threads: Vec<Value> = chart
                                .iter()
                                .enumerate()
                                .filter_map(|(i, t)| {
                                    let t = t.as_ref()?;
                                    let mut v = describe_thread(t);
                                    v["index"] = json!(i);
                                    if *id == "brother" {
                                        // Matched to (1–61) or only kept from files (62–64).
                                        v["basic"] = json!(i <= BROTHER_BASIC);
                                    }
                                    Some(v)
                                })
                                .collect();
                            json!({ "id": id, "name": name, "threads": threads })
                        })
                        .collect(),
                ))
            },
        ),
        spec(
            "threads.exportCsv",
            "Write a user chart (chart) or a built-in chart (builtin) as OpenStitch's thread-chart CSV (code,brand,description,R,G,B)",
            Effect::File,
            vec![
                opt("chart", "string", "User chart name"),
                opt(
                    "builtin",
                    "string",
                    "Built-in chart id (see threads.builtinCharts)",
                ),
                crate::commands::PATH,
            ],
            |s, p| {
                let (name, threads): (String, Vec<Thread>) =
                    match (p.has("chart"), p.has("builtin")) {
                        (true, false) => {
                            let n = p.string("chart", None)?;
                            let c = s
                                .user
                                .chart_index(&n)
                                .map(|i| &s.user.thread_charts[i])
                                .ok_or_else(|| p.not_found(format!("thread chart '{n}'")))?;
                            (c.name.clone(), c.threads.clone())
                        }
                        (false, true) => {
                            let id = p.string("builtin", None)?;
                            let (_, name, chart) = builtin_charts()
                                .into_iter()
                                .find(|(i, _, _)| *i == id)
                                .ok_or_else(|| {
                                    let ids: Vec<&str> =
                                        builtin_charts().iter().map(|c| c.0).collect();
                                    p.invalid("builtin", format!("use one of {}", ids.join(", ")))
                                })?;
                            (name.to_string(), chart.iter().flatten().cloned().collect())
                        }
                        _ => return Err(p.invalid("chart", "give either chart or builtin")),
                    };
                let path = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
                std::fs::write(&path, chart_csv(&name, &threads))
                    .map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({ "path": path.display().to_string(), "colors": threads.len() }))
            },
        ),
        spec(
            "threads.remap",
            "Change every thread of the design (or of the given objects) to the nearest colour of a user chart",
            Effect::Edit,
            vec![
                req("chart", "string", "Chart"),
                opt("ids", "[int]", "Objects (default all)"),
            ],
            |s, p| {
                let n = p.string("chart", None)?;
                let chart = s
                    .user
                    .chart_index(&n)
                    .map(|i| s.user.thread_charts[i].threads.clone())
                    .ok_or_else(|| p.not_found(format!("thread chart '{n}'")))?;
                if chart.is_empty() {
                    return Err(p.failed("the chart is empty"));
                }
                let idx: Vec<usize> = if p.has("ids") || p.has("id") {
                    indexes(p, &s.design)?
                } else {
                    (0..s.design.objects.len()).collect()
                };
                let nearest = |t: &Thread| {
                    chart
                        .iter()
                        .min_by_key(|c| color_distance(t, c))
                        .cloned()
                        .unwrap_or_else(|| t.clone())
                };
                for i in idx {
                    let o = &mut s.design.objects[i];
                    if let Some(r) = &mut o.region {
                        r.thread = nearest(&r.thread);
                    }
                    if let Some(l) = &mut o.line {
                        l.thread = nearest(&l.thread);
                    }
                    if let Some(b) = &mut o.stitch_block {
                        for t in &mut b.threads {
                            *t = nearest(t);
                        }
                    }
                }
                Ok(json!(null))
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quoting_round_trips() {
        let mut t = Thread::rgb(0x0a14ff);
        t.catalog_number = Some(Cow::Borrowed("0042"));
        t.brand = Some(Cow::Borrowed("Acme, Inc."));
        t.name = Some(Cow::Borrowed("Sky \"blue\""));
        let line = csv_record(&t);
        assert_eq!(line, "0042,\"Acme, Inc.\",\"Sky \"\"blue\"\"\",10,20,255");
        assert_eq!(csv_thread(&line), Ok(t));
        assert_eq!(
            csv_fields(" \"a,b\" , c ,\"\"").expect("ok"),
            ["a,b", "c", ""]
        );
        assert!(csv_fields("\"open").is_err());
        assert!(csv_header(
            &csv_fields("Code,Brand,Description,r,g,b").expect("ok")
        ));
        assert!(!csv_header(&csv_fields("1,a,b,1,2,3").expect("ok")));
        let csv = chart_csv("Mine\n", [&Thread::rgb(0xff0000)]);
        assert_eq!(
            csv,
            "# OpenStitch thread chart: Mine\r\ncode,brand,description,R,G,B\r\n,,,255,0,0\r\n"
        );
    }

    #[test]
    fn csv_lines() {
        let t = csv_thread("123, Acme ,Sky blue,10,20,255").expect("ok");
        assert_eq!(t.hex(), "#0a14ff");
        assert_eq!(t.catalog_number.as_deref(), Some("123"));
        assert_eq!(t.brand.as_deref(), Some("Acme"));
        assert_eq!(t.name.as_deref(), Some("Sky blue"));
        for bad in [
            "1,2,3",
            "1,a,b,256,0,0",
            "1,a,b,-1,0,0",
            "x1,a,b,1,2,3",
            "1,a,b,1,2,3,4",
            "",
        ] {
            assert!(csv_thread(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn settings_file_round_trip_and_hostile_files() {
        let mut s = UserSettings::default();
        s.thread_charts.push(ThreadChart {
            name: "Mine".into(),
            threads: vec![Thread::rgb(0x123456)],
        });
        assert_eq!(UserSettings::decode(&s.encode()), Ok(s.clone()));
        for bad in [
            &b""[..],
            b"[]",
            b"{\"format\":\"other\"}",
            b"{\"format\":\"openstitch.user\",\"settings\":{\"threadCharts\":7}}",
            b"\xff\xfe",
        ] {
            assert!(UserSettings::decode(bad).is_err());
        }
        assert_eq!(
            UserSettings::decode(b"{\"format\":\"openstitch.user\",\"settings\":{}}"),
            Ok(UserSettings::default())
        );
    }

    #[test]
    fn default_path_ends_in_user_json() {
        if let Some(p) = default_path() {
            assert!(p.ends_with("user.json"));
        }
    }
}
