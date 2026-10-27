//! Compatibility test specifications: `compat/specs/<area>/<id>.json`, schema
//! [`SCHEMA`]. See docs/COMPATIBILITY_TESTS.md for the format.

use crate::measure::Measure;
use crate::tolerance::Tolerance;
use crate::trace::Anchor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

pub const SCHEMA: &str = "openstitch-compat/1";
/// Largest spec file read.
pub const MAX_SPEC_BYTES: u64 = 1 << 20;
pub const MAX_STEPS: usize = 10_000;

/// Evidence tiers, weakest first. A result reaches a tier only with that tier's evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Expected values from OpenStitch's own earlier output or from the guide's description.
    InternalRegression,
    /// An independent implementation (pyembroidery, libembroidery).
    Oracle,
    /// Output of PE-DESIGN 11, supplied by the owner (compat/reference/).
    Reference,
    /// A sew-out on a Brother machine (compat/machine/).
    Machine,
}

impl Tier {
    pub const ALL: [Tier; 4] = [
        Tier::InternalRegression,
        Tier::Oracle,
        Tier::Reference,
        Tier::Machine,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Tier::InternalRegression => "internal-regression",
            Tier::Oracle => "oracle",
            Tier::Reference => "reference",
            Tier::Machine => "machine",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spec {
    pub schema: String,
    pub id: String,
    pub area: String,
    pub title: String,
    pub description: String,
    pub covers: Covers,
    pub evidence: Vec<Evidence>,
    /// Repository files copied into the work directory before the script runs.
    #[serde(default)]
    pub inputs: Vec<Input>,
    /// `stitchtool exec` requests: `{"command", "params"?, "id"?}`; `id` labels a step whose
    /// reply a measurement reads.
    pub script: Vec<Value>,
    #[serde(default)]
    pub exports: Vec<Export>,
    /// PE-DESIGN 11 output this spec can be checked against (compat/reference/).
    #[serde(default)]
    pub references: Vec<ReferenceSpec>,
    /// Sew-out record (compat/machine/).
    #[serde(default)]
    pub machine: Option<MachineSpec>,
    pub measurements: Vec<MeasurementSpec>,
    /// Reason the spec is not run.
    #[serde(default)]
    pub skip: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Covers {
    /// Item numbers of docs/COMPATIBILITY_AUDIT.md.
    #[serde(default)]
    pub audit: Vec<u32>,
    /// docs/FEATURE_MATRIX.md rows: the Area cell and the start of the Feature cell.
    #[serde(default)]
    pub feature_matrix: Vec<RowRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RowRef {
    pub area: String,
    pub feature: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    /// `guide`, `spec`, `audit`, `measured` or `oracle`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

const EVIDENCE_KINDS: [&str; 5] = ["guide", "spec", "audit", "measured", "oracle"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    /// Repository-relative path.
    pub from: String,
    /// File name in the work directory.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Export {
    /// File name in the work directory; the extension picks the format.
    pub path: String,
    /// PES only: `1t`, `1`, `6` or `6t` (default 6).
    #[serde(default)]
    pub pes_version: Option<String>,
    /// Write the named input file again through the codecs (read, then write) instead of
    /// exporting the design.
    #[serde(default)]
    pub from_input: Option<String>,
    /// The script writes this file itself (e.g. `library.convert`); it is only read back.
    #[serde(default)]
    pub from_script: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferenceSpec {
    pub id: String,
    /// Path under compat/reference/.
    pub file: String,
    /// What to make in PE-DESIGN 11 and how to save it.
    pub make: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MachineSpec {
    /// Path under compat/machine/ of the sew-out record.
    pub record: String,
    /// What to sew and what to look at.
    pub make: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasurementSpec {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub measure: Measure,
    #[serde(default)]
    pub source: Source,
    /// Expected value (see [`crate::tolerance::check`]).
    #[serde(default)]
    pub expected: Option<Value>,
    #[serde(default)]
    pub tolerance: Tolerance,
    /// Tier of the expected value: internal-regression or oracle.
    #[serde(default = "default_tier")]
    pub tier: Tier,
    /// Where the expected value comes from (audit item, guide page, oracle fixture).
    pub evidence: String,
    /// Also measure the reference file and compare.
    #[serde(default)]
    pub reference: Option<ReferenceCheck>,
}

fn default_tier() -> Tier {
    Tier::InternalRegression
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferenceCheck {
    /// Reference id (may be left out when the spec has one reference).
    #[serde(default)]
    pub id: Option<String>,
    /// Tolerance against the reference value (default: the measurement's).
    #[serde(default)]
    pub tolerance: Option<Tolerance>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    /// An export's file name: the file is read back with OpenStitch's reader.
    #[serde(default)]
    pub export: Option<String>,
    /// A labelled script step (its reply: a `stitches.points` result or any JSON).
    #[serde(default)]
    pub step: Option<String>,
    /// Only this colour block (0-based).
    #[serde(default)]
    pub color_block: Option<usize>,
    /// Move the trace so the anchor lands on `to` (mm) before measuring.
    #[serde(default)]
    pub align: Option<Align>,
    /// Leave stitches at the start and end out of the measurement (applied after
    /// `colorBlock` and `align`, to OpenStitch's file and the reference alike).
    #[serde(default)]
    pub exclude: Option<Exclude>,
}

/// Stitches a measurement leaves out: the first and last `first` / `last` stitch segments
/// (needle penetration to needle penetration, as `stitchSegments` counts them), e.g. a
/// tie-in and an end lock. A spec that uses it says why in the measurement's evidence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Exclude {
    #[serde(default)]
    pub first: usize,
    #[serde(default)]
    pub last: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Align {
    pub anchor: Anchor,
    pub to: [f64; 2],
}

/// A repository-relative path with only plain components (no `..`, `.`, root, prefix,
/// backslash or NUL).
pub fn safe_relative(p: &str) -> Result<PathBuf, String> {
    if p.is_empty() || p.contains('\0') || p.contains('\\') {
        return Err(format!("path {p:?}: empty, NUL or backslash"));
    }
    let path = Path::new(p);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("path {p:?}: must be relative, without . or .."));
    }
    Ok(path.to_path_buf())
}

/// A plain file name (one component).
pub fn plain_name(p: &str) -> Result<(), String> {
    let path = safe_relative(p)?;
    if path.components().count() != 1 || p.contains('/') {
        return Err(format!("{p:?}: must be a file name without directories"));
    }
    Ok(())
}

fn id_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Spec {
    /// Every reference id resolved for a measurement.
    pub fn reference_for(&self, check: &ReferenceCheck) -> Option<&ReferenceSpec> {
        match &check.id {
            Some(id) => self.references.iter().find(|r| &r.id == id),
            None if self.references.len() == 1 => self.references.first(),
            None => None,
        }
    }

    /// Step labels (`id` of script steps) in order.
    pub fn step_labels(&self) -> Vec<&str> {
        self.script
            .iter()
            .filter_map(|s| s.get("id").and_then(Value::as_str))
            .collect()
    }

    /// Checks everything that does not need running; every problem is listed.
    pub fn validate(&self) -> Vec<String> {
        let mut e = Vec::new();
        let mut err = |m: String| e.push(m);
        if self.schema != SCHEMA {
            err(format!("schema must be {SCHEMA:?}"));
        }
        if !id_ok(&self.id) {
            err("id: lower-case letters, digits and -, at most 64".into());
        }
        if !id_ok(&self.area) {
            err("area: lower-case letters, digits and -".into());
        }
        if self.title.trim().is_empty() || self.description.trim().is_empty() {
            err("title and description must not be empty".into());
        }
        if self.covers.audit.is_empty() && self.covers.feature_matrix.is_empty() {
            err("covers: at least one audit item or FEATURE_MATRIX row".into());
        }
        if self.evidence.is_empty() {
            err("evidence: at least one source".into());
        }
        for ev in &self.evidence {
            if !EVIDENCE_KINDS.contains(&ev.kind.as_str()) {
                err(format!(
                    "evidence kind {:?}: use {EVIDENCE_KINDS:?}",
                    ev.kind
                ));
            }
            if let Some(u) = &ev.url
                && !u.starts_with("https://")
            {
                err(format!("evidence url {u:?} must be https"));
            }
            if ev.url.is_none() && ev.note.is_none() {
                err("evidence needs a url or a note".into());
            }
        }
        let mut names = BTreeSet::new();
        for i in &self.inputs {
            if let Err(m) = safe_relative(&i.from) {
                err(format!("input {m}"));
            }
            if let Err(m) = plain_name(&i.name) {
                err(format!("input name {m}"));
            }
            if !names.insert(i.name.clone()) {
                err(format!("input name {:?} twice", i.name));
            }
        }
        if self.script.len() > MAX_STEPS {
            err(format!("script: at most {MAX_STEPS} steps"));
        }
        let mut labels = BTreeSet::new();
        for (k, s) in self.script.iter().enumerate() {
            if s.get("command").and_then(Value::as_str).is_none() {
                err(format!("script step {k}: needs a string \"command\""));
            }
            if let Some(id) = s.get("id")
                && !id.as_str().is_some_and(|id| labels.insert(id.to_string()))
            {
                err(format!("script step {k}: \"id\" must be a unique string"));
            }
        }
        let mut exports = BTreeSet::new();
        for x in &self.exports {
            if let Err(m) = plain_name(&x.path) {
                err(format!("export {m}"));
            }
            match openstitch_formats::FileType::from_path(&x.path) {
                Some(t) if t.can_write() => {
                    if x.pes_version.is_some() && t != openstitch_formats::FileType::Pes {
                        err(format!("export {}: pesVersion is for PES only", x.path));
                    }
                }
                _ => err(format!("export {}: not a writable format", x.path)),
            }
            if let Some(v) = &x.pes_version
                && openstitch_formats::pes::WriteOptions::parse(v).is_none()
            {
                err(format!("export {}: pesVersion 1t, 1, 6 or 6t", x.path));
            }
            if let Some(i) = &x.from_input
                && !self.inputs.iter().any(|n| &n.name == i)
            {
                err(format!("export {}: no input named {i:?}", x.path));
            }
            if x.from_script && (x.from_input.is_some() || x.pes_version.is_some()) {
                err(format!(
                    "export {}: fromScript takes no fromInput or pesVersion",
                    x.path
                ));
            }
            if !exports.insert(x.path.clone()) || names.contains(&x.path) {
                err(format!("export {}: name used twice", x.path));
            }
        }
        let mut refs = BTreeSet::new();
        for r in &self.references {
            if !id_ok(&r.id) || !refs.insert(r.id.clone()) {
                err(format!("reference id {:?}: unique, lower-case", r.id));
            }
            if let Err(m) = safe_relative(&r.file) {
                err(format!("reference file {m}"));
            }
            if openstitch_formats::FileType::from_path(&r.file).is_none() {
                err(format!(
                    "reference file {}: not an embroidery format",
                    r.file
                ));
            }
            if r.make.trim().is_empty() {
                err(format!("reference {}: `make` must say what to make", r.id));
            }
        }
        if let Some(m) = &self.machine {
            if let Err(x) = safe_relative(&m.record) {
                err(format!("machine record {x}"));
            }
            if m.make.trim().is_empty() {
                err("machine: `make` must say what to sew".into());
            }
        }
        if self.measurements.is_empty() {
            err("measurements: at least one".into());
        }
        let mut ids = BTreeSet::new();
        for m in &self.measurements {
            let mut merr = |x: String| err(format!("measurement {}: {x}", m.id));
            if !id_ok(&m.id) || !ids.insert(m.id.clone()) {
                merr("id must be unique, lower-case letters, digits and -".into());
            }
            if let Err(x) = m.measure.validate() {
                merr(x);
            }
            if let Err(x) = m.tolerance.validate() {
                merr(x);
            }
            if !matches!(m.tier, Tier::InternalRegression | Tier::Oracle) {
                merr("tier of an expected value is internal-regression or oracle; reference and machine come from files".into());
            }
            if m.evidence.trim().is_empty() {
                merr("evidence must say where the expected value comes from".into());
            }
            let has_against = matches!(
                &m.measure,
                Measure::Compare {
                    against: Some(_),
                    ..
                } | Measure::BytesIdentical { against: Some(_) }
            );
            if let Measure::Compare {
                against: Some(a), ..
            }
            | Measure::BytesIdentical { against: Some(a) } = &m.measure
                && let Err(x) = safe_relative(a)
            {
                merr(format!("against {x}"));
            }
            let comparison = matches!(
                m.measure,
                Measure::Compare { .. } | Measure::BytesIdentical { .. }
            );
            if comparison && !has_against && m.reference.is_none() {
                merr("compares with nothing: give `against` or `reference`".into());
            }
            if !comparison && m.expected.is_none() && m.reference.is_none() {
                merr("needs `expected`, `reference` or both".into());
            }
            let src = &m.source;
            match &m.measure {
                Measure::Refused { .. } => {
                    if src.export.is_some() || src.step.is_some() {
                        merr("refused runs on the session: no source".into());
                    }
                }
                Measure::ReplyValue { .. } => {
                    if src.step.is_none() || src.export.is_some() {
                        merr("replyValue needs source.step".into());
                    }
                }
                other => match (&src.export, &src.step) {
                    (Some(_), Some(_)) | (None, None) => {
                        merr("source: exactly one of export or step".into())
                    }
                    (None, Some(_)) if other.needs_file() => {
                        merr(format!("{} needs an export source", other.kind()))
                    }
                    _ => {}
                },
            }
            if let Some(x) = &src.export
                && !exports.contains(x)
            {
                merr(format!("source export {x:?} is not in exports"));
            }
            if let Some(s) = &src.step
                && !labels.contains(s)
            {
                merr(format!("source step {s:?} labels no script step"));
            }
            if let Some(r) = &m.reference {
                if src.export.is_none() {
                    merr("a reference check needs an export source".into());
                }
                if self.reference_for(r).is_none() {
                    merr("reference: no such reference (name it when there are several)".into());
                }
                if let Some(t) = &r.tolerance
                    && let Err(x) = t.validate()
                {
                    merr(x);
                }
            }
        }
        e
    }
}

/// One spec file and what was found reading it.
pub struct Loaded {
    /// Path relative to the specs directory, `/`-separated.
    pub path: String,
    pub spec: Result<Spec, Vec<String>>,
}

/// Reads every `<area>/<id>.json` under `dir`, sorted by path. Directory and file names must
/// match the spec's `area` and `id`.
pub fn load_dir(dir: &Path) -> Result<Vec<Loaded>, String> {
    let mut out = Vec::new();
    let mut areas: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    areas.sort();
    for area in areas {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&area)
            .map_err(|e| format!("{}: {e}", area.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        for f in files {
            let area_name = area
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stem = f
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let path = format!("{area_name}/{stem}.json");
            out.push(Loaded {
                spec: load_file(&f, &area_name, &stem),
                path,
            });
        }
    }
    Ok(out)
}

fn load_file(f: &Path, area: &str, stem: &str) -> Result<Spec, Vec<String>> {
    let meta = std::fs::symlink_metadata(f).map_err(|e| vec![e.to_string()])?;
    if !meta.is_file() {
        return Err(vec!["not a regular file".into()]);
    }
    if meta.len() > MAX_SPEC_BYTES {
        return Err(vec![format!("larger than {MAX_SPEC_BYTES} bytes")]);
    }
    let text = std::fs::read_to_string(f).map_err(|e| vec![e.to_string()])?;
    parse(&text, Some((area, stem)))
}

/// Parses and validates one spec; `place` is the expected `(area, id)` from its path.
pub fn parse(text: &str, place: Option<(&str, &str)>) -> Result<Spec, Vec<String>> {
    let spec: Spec = serde_json::from_str(text).map_err(|e| vec![format!("JSON: {e}")])?;
    let mut problems = spec.validate();
    if let Some((area, id)) = place {
        if spec.area != area {
            problems.push(format!(
                "area {:?} but the directory is {area:?}",
                spec.area
            ));
        }
        if spec.id != id {
            problems.push(format!("id {:?} but the file is {id}.json", spec.id));
        }
    }
    if problems.is_empty() {
        Ok(spec)
    } else {
        Err(problems)
    }
}
