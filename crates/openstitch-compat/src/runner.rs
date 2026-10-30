//! Runs specs headlessly: copies the inputs into a fresh work directory, runs the script in a
//! session sandboxed to it, writes the exports, reads them back, and checks every measurement
//! at each tier its evidence allows.
//!
//! Tiers are kept apart. An `expected` value is checked at the measurement's tier
//! (internal-regression or oracle). A reference check is a separate check at the reference
//! tier and needs a PE-DESIGN file with a provenance record (see [`crate::provenance`]); when
//! the file does not exist yet the check is pending, never failed, and never passed. A spec's
//! tier is the highest tier among its passing checks, and only when nothing failed.

use crate::compare::{compare_files, first_byte_difference};
use crate::measure::{Measure, Subject};
use crate::provenance::{self, Manifest, Reference};
use crate::report::{Check, CheckStatus, ExportInfo, InvalidSpec, Report, SpecResult, Status};
use crate::sha256;
use crate::spec::{self, MeasurementSpec, Source, Spec, Tier, safe_relative};
use crate::tolerance::{Tolerance, check};
use crate::trace::Trace;
use openstitch_core::{Command, StitchPattern};
use openstitch_engine::Session;
use openstitch_formats::FileType;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the runner reads and writes. Relative paths are taken as they are (from the current
/// directory, normally the repository root).
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Inputs and `against` files resolve here.
    pub repo_root: PathBuf,
    pub specs_dir: PathBuf,
    pub reference_root: PathBuf,
    pub machine_root: PathBuf,
    /// Scratch space: one fresh sub-directory per spec.
    pub work_dir: PathBuf,
    /// Run only specs whose `area/id` contains this (an id, part of one, `area/id` or `area/`).
    /// Matching no spec is an error, so a typo never looks like a passing run.
    pub only: Option<String>,
}

impl Config {
    /// The repository layout under `root`: compat/specs, compat/reference, compat/machine and
    /// target/compat/work.
    pub fn in_repo(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            specs_dir: root.join("compat/specs"),
            reference_root: root.join("compat/reference"),
            machine_root: root.join("compat/machine"),
            work_dir: root.join("target/compat/work"),
            repo_root: root,
            only: None,
        }
    }
}

/// A file a spec exported, read back.
struct Exported {
    bytes: Vec<u8>,
    ty: FileType,
    pattern: StitchPattern,
}

pub fn run(config: &Config) -> Result<Report, String> {
    let loaded = spec::load_dir(&config.specs_dir)?;
    let manifest = provenance::load_manifest(&config.reference_root)?;
    let selected = |path: &str| {
        let name = path.strip_suffix(".json").unwrap_or(path);
        config
            .only
            .as_ref()
            .is_none_or(|o| name.contains(o.as_str()))
    };
    if let Some(only) = &config.only
        && !loaded.iter().any(|l| selected(&l.path))
    {
        return Err(no_match(only, &loaded));
    }
    let mut results = Vec::new();
    let mut invalid = Vec::new();
    for l in loaded {
        match l.spec {
            Ok(s) => {
                if selected(&l.path) {
                    results.push(run_spec(&s, &l.path, config, &manifest));
                }
            }
            Err(problems) => invalid.push(InvalidSpec {
                path: l.path,
                problems,
            }),
        }
    }
    Ok(Report::new(results, invalid))
}

/// The error for an `--only` that selects nothing: the closest spec names, or all of them.
fn no_match(only: &str, loaded: &[spec::Loaded]) -> String {
    let names: Vec<&str> = loaded
        .iter()
        .map(|l| l.path.strip_suffix(".json").unwrap_or(&l.path))
        .collect();
    let words: Vec<&str> = only
        .split(['/', '-', '_', '.', ' '])
        .filter(|w| w.len() >= 3)
        .collect();
    // The same id under another area first, then the most matched characters.
    let last = only.rsplit('/').next().unwrap_or(only);
    let mut close: Vec<(usize, &str)> = names
        .iter()
        .map(|n| {
            let same_id = n.rsplit('/').next() == Some(last);
            let shared: usize = words
                .iter()
                .filter(|w| n.contains(**w))
                .map(|w| w.len())
                .sum();
            (usize::from(same_id) * 1000 + shared, *n)
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    close.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    let (label, list): (&str, Vec<&str>) = if close.is_empty() {
        ("Specs", names.clone())
    } else {
        ("Close matches", close.iter().take(8).map(|c| c.1).collect())
    };
    format!(
        "--only {only:?} matches no spec. --only takes an id, part of one, area/id or area/ \
         (the files are <specs>/<area>/<id>.json). {label}: {}",
        if list.is_empty() {
            "none (the specs directory is empty)".to_string()
        } else {
            list.join(", ")
        }
    )
}

pub fn run_spec(spec: &Spec, path: &str, config: &Config, manifest: &Manifest) -> SpecResult {
    let mut result = SpecResult {
        id: spec.id.clone(),
        area: spec.area.clone(),
        title: spec.title.clone(),
        path: path.to_string(),
        covers: spec.covers.clone(),
        status: Status::Skipped,
        tier: None,
        tiers: BTreeMap::new(),
        pending: 0,
        error: None,
        skipped: spec.skip.clone(),
        exports: Vec::new(),
        checks: Vec::new(),
    };
    if spec.skip.is_some() {
        return result;
    }
    match execute(spec, config, manifest, &mut result) {
        Ok(()) => settle(&mut result),
        Err(e) => {
            result.status = Status::Error;
            result.error = Some(e);
        }
    }
    result
}

/// Spec status and tier from its checks.
fn settle(r: &mut SpecResult) {
    let has = |s: CheckStatus| r.checks.iter().any(|c| c.status == s);
    r.pending = r
        .checks
        .iter()
        .filter(|c| c.status == CheckStatus::Pending)
        .count();
    r.status = if has(CheckStatus::Error) {
        Status::Error
    } else if has(CheckStatus::Fail) {
        Status::Fail
    } else if has(CheckStatus::Pass) {
        Status::Pass
    } else if has(CheckStatus::Pending) {
        Status::PendingReference
    } else {
        Status::Skipped
    };
    for c in &r.checks {
        let worst = r.tiers.entry(c.tier).or_insert(c.status);
        *worst = (*worst).max(c.status);
    }
    // The highest tier that passed with every tier below it that has checks.
    r.tier = None;
    if r.status == Status::Pass {
        for (tier, status) in &r.tiers {
            if *status != CheckStatus::Pass {
                break;
            }
            r.tier = Some(*tier);
        }
    }
}

fn work_dir(config: &Config, spec: &Spec) -> Result<PathBuf, String> {
    // The id is validated (lower-case letters, digits, -), so this stays inside work_dir.
    let dir = config.work_dir.join(&spec.id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    dir.canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))
}

fn repo_file(config: &Config, rel: &str) -> Result<Vec<u8>, String> {
    let p = config.repo_root.join(safe_relative(rel)?);
    provenance::read_limited(&p)?.ok_or_else(|| format!("{rel}: no such file"))
}

fn file_type(name: &str) -> Result<FileType, String> {
    FileType::from_path(name).ok_or_else(|| format!("{name}: unknown embroidery format"))
}

fn execute(
    spec: &Spec,
    config: &Config,
    manifest: &Manifest,
    result: &mut SpecResult,
) -> Result<(), String> {
    let dir = work_dir(config, spec)?;
    for input in &spec.inputs {
        let bytes = repo_file(config, &input.from)?;
        std::fs::write(dir.join(&input.name), bytes)
            .map_err(|e| format!("input {}: {e}", input.name))?;
    }

    let mut session = Session::sandboxed(&dir);
    let mut replies: BTreeMap<String, Value> = BTreeMap::new();
    for (k, step) in spec.script.iter().enumerate() {
        let reply = session.handle(step);
        if reply["ok"] != json!(true) {
            return Err(format!(
                "script step {k} ({}): {}",
                step["command"].as_str().unwrap_or("?"),
                reply["error"].as_str().unwrap_or("failed")
            ));
        }
        if let Some(label) = step.get("id").and_then(Value::as_str) {
            replies.insert(label.to_string(), reply["result"].clone());
        }
    }

    let mut exports: BTreeMap<String, Exported> = BTreeMap::new();
    for x in &spec.exports {
        let ty = file_type(&x.path)?;
        let target = dir.join(&x.path);
        if x.from_script {
            // Written by the script; read back below.
        } else if let Some(input) = &x.from_input {
            let src = std::fs::read(dir.join(input)).map_err(|e| format!("{input}: {e}"))?;
            let pattern = file_type(input)?
                .read(&src)
                .map_err(|e| format!("{input}: {e}"))?;
            let bytes = match (&x.pes_version, ty) {
                (Some(v), FileType::Pes) => {
                    let o = openstitch_formats::pes::WriteOptions::parse(v)
                        .ok_or_else(|| format!("pesVersion {v}"))?;
                    openstitch_formats::pes::write_with(&pattern, &o)
                }
                _ => ty.write(&pattern),
            }
            .map_err(|e| format!("export {}: {e}", x.path))?;
            std::fs::write(&target, bytes).map_err(|e| format!("export {}: {e}", x.path))?;
        } else {
            let mut params = json!({"path": x.path});
            if let Some(v) = &x.pes_version {
                params["pesVersion"] = json!(v);
            }
            let reply = session.handle(&json!({"command": "design.export", "params": params}));
            if reply["ok"] != json!(true) {
                return Err(format!(
                    "export {}: {}",
                    x.path,
                    reply["error"].as_str().unwrap_or("failed")
                ));
            }
        }
        let bytes = std::fs::read(&target).map_err(|e| format!("export {}: {e}", x.path))?;
        let pattern = ty
            .read(&bytes)
            .map_err(|e| format!("reading back {}: {e}", x.path))?;
        result.exports.push(ExportInfo {
            path: x.path.clone(),
            bytes: bytes.len(),
            sha256: sha256::hex(&bytes),
            stitches: pattern.count(Command::Stitch),
            color_blocks: pattern.color_block_count(),
        });
        exports.insert(x.path.clone(), Exported { bytes, ty, pattern });
    }

    let ctx = Ctx {
        spec,
        config,
        manifest,
        exports: &exports,
        replies: &replies,
    };
    for m in &spec.measurements {
        result.checks.extend(ctx.checks(m, &mut session));
    }
    if let Some(machine) = &spec.machine {
        result
            .checks
            .push(ctx.machine_check(machine, &result.exports));
    }
    Ok(())
}

struct Ctx<'a> {
    spec: &'a Spec,
    config: &'a Config,
    manifest: &'a Manifest,
    exports: &'a BTreeMap<String, Exported>,
    replies: &'a BTreeMap<String, Value>,
}

/// A trace with the source's colour block and alignment applied.
fn shaped(mut trace: Trace, source: &Source) -> Trace {
    if let Some(b) = source.color_block {
        trace = trace.block(b);
    }
    if let Some(a) = source.align {
        trace.align(a.anchor, (a.to[0], a.to[1]));
    }
    if let Some(x) = source.exclude {
        trace = trace.excluding(x.first, x.last);
    }
    trace
}

fn base_check(m: &MeasurementSpec, tier: Tier) -> Check {
    Check {
        measurement: m.id.clone(),
        kind: m.measure.kind().into(),
        tier,
        status: CheckStatus::Error,
        expected: None,
        actual: None,
        tolerance: None,
        file: None,
        mismatches: Vec::new(),
        differences: Vec::new(),
        unlisted_differences: 0,
        note: None,
        dropped_repeats: None,
    }
}

impl Ctx<'_> {
    fn export(&self, m: &MeasurementSpec) -> Result<&Exported, String> {
        let name = m.source.export.as_deref().ok_or("no export source")?;
        self.exports
            .get(name)
            .ok_or_else(|| format!("export {name} was not written"))
    }

    /// The measurement taken on OpenStitch's output.
    fn ours(&self, m: &MeasurementSpec, session: &mut Session) -> Result<Value, String> {
        match &m.measure {
            Measure::ReplyValue { pointer, pluck } => {
                let step = m.source.step.as_deref().ok_or("no step source")?;
                let reply = self.replies.get(step).ok_or("no such step")?;
                let v = reply
                    .pointer(pointer)
                    .ok_or_else(|| format!("{pointer} is not in the reply of {step}"))?;
                Ok(match pluck {
                    Some(k) => Value::Array(
                        v.as_array()
                            .ok_or("pluck needs an array")?
                            .iter()
                            .map(|e| e.get(k).cloned().unwrap_or(Value::Null))
                            .collect(),
                    ),
                    None => v.clone(),
                })
            }
            Measure::Refused {
                command,
                params,
                error_contains,
            } => {
                let p = (!params.is_null()).then_some(params);
                Ok(json!(match session.execute(command, p) {
                    Ok(_) => false,
                    Err(e) => error_contains
                        .as_ref()
                        .is_none_or(|c| e.to_string().contains(c.as_str())),
                }))
            }
            other => {
                if let Some(step) = &m.source.step {
                    let reply = self.replies.get(step).ok_or("no such step")?;
                    let trace = shaped(Trace::from_points_reply(reply)?, &m.source);
                    other.evaluate(&Subject {
                        trace: &trace,
                        file: None,
                    })
                } else {
                    let x = self.export(m)?;
                    let trace = shaped(Trace::from_pattern(&x.pattern), &m.source);
                    other.evaluate(&Subject {
                        trace: &trace,
                        file: Some((&x.bytes, x.ty, &x.pattern)),
                    })
                }
            }
        }
    }

    fn checks(&self, m: &MeasurementSpec, session: &mut Session) -> Vec<Check> {
        let mut out = Vec::new();
        let comparison = matches!(
            m.measure,
            Measure::Compare { .. } | Measure::BytesIdentical { .. }
        );
        if comparison {
            if let Measure::Compare {
                against: Some(a), ..
            }
            | Measure::BytesIdentical { against: Some(a) } = &m.measure
            {
                let mut c = base_check(m, m.tier);
                c.file = Some(a.clone());
                match repo_file(self.config, a)
                    .and_then(|b| Ok((file_type(a)?, b)))
                    .and_then(|(ty, b)| self.compare_with(m, &b, ty, &mut c))
                {
                    Ok(()) => {}
                    Err(e) => c.note = Some(e),
                }
                out.push(c);
            }
        } else if let Some(expected) = &m.expected {
            let mut c = base_check(m, m.tier);
            c.expected = Some(expected.clone());
            c.tolerance = Some(m.tolerance);
            match self.ours(m, session) {
                Ok(actual) => {
                    c.mismatches = check(expected, &actual, &m.tolerance);
                    c.status = if c.mismatches.is_empty() {
                        CheckStatus::Pass
                    } else {
                        CheckStatus::Fail
                    };
                    c.actual = Some(actual);
                }
                Err(e) => c.note = Some(e),
            }
            out.push(c);
        }
        if let Some(rc) = &m.reference {
            out.push(self.reference_check(m, rc, session));
        }
        out
    }

    /// Compares an export with `expected` bytes (a repository or reference file).
    fn compare_with(
        &self,
        m: &MeasurementSpec,
        expected: &[u8],
        ty: FileType,
        c: &mut Check,
    ) -> Result<(), String> {
        let x = self.export(m)?;
        match &m.measure {
            Measure::Compare { options, .. } => {
                let cmp = compare_files((expected, ty), (&x.bytes, x.ty), options)?;
                c.expected = Some(json!(0));
                c.actual = Some(json!(cmp.total()));
                c.unlisted_differences = cmp.unlisted;
                c.differences = cmp.differences;
                c.dropped_repeats = cmp.dropped_repeats;
                c.status = if c.unlisted_differences == 0 && c.differences.is_empty() {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                };
            }
            Measure::BytesIdentical { .. } => {
                let d = first_byte_difference(expected, &x.bytes);
                c.expected = Some(json!(true));
                c.actual = Some(json!(d.is_none()));
                c.status = if d.is_none() {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                };
                c.differences.extend(d);
            }
            _ => return Err("not a comparison".into()),
        }
        Ok(())
    }

    fn reference_check(
        &self,
        m: &MeasurementSpec,
        rc: &spec::ReferenceCheck,
        session: &mut Session,
    ) -> Check {
        let mut c = base_check(m, Tier::Reference);
        let Some(r) = self.spec.reference_for(rc) else {
            c.note = Some("no such reference".into());
            return c;
        };
        c.file = Some(format!("compat/reference/{}", r.file));
        let (bytes, provenance) = match provenance::resolve(
            &self.config.reference_root,
            self.manifest,
            &r.file,
            &self.spec.id,
            &r.id,
        ) {
            Reference::Pending => {
                c.status = CheckStatus::Pending;
                c.note = Some(format!("pending reference: {}", r.make));
                return c;
            }
            Reference::Invalid(e) => {
                c.note = Some(e);
                return c;
            }
            Reference::Available { bytes, provenance } => (bytes, provenance),
        };
        let provenance_note = format!(
            "PE-DESIGN {} by {} on {}",
            provenance.pe_design_version, provenance.made_by, provenance.date
        );
        let ty = match file_type(&r.file) {
            Ok(t) => t,
            Err(e) => {
                c.note = Some(e);
                return c;
            }
        };
        if matches!(
            m.measure,
            Measure::Compare { .. } | Measure::BytesIdentical { .. }
        ) {
            match self.compare_with(m, &bytes, ty, &mut c) {
                Ok(()) => c.note = Some(provenance_note),
                Err(e) => {
                    c.status = CheckStatus::Error;
                    c.note = Some(e);
                }
            }
            return c;
        }
        let pattern = match ty.read(&bytes) {
            Ok(p) => p,
            Err(e) => {
                c.note = Some(format!("reading the reference: {e}"));
                return c;
            }
        };
        let trace = shaped(Trace::from_pattern(&pattern), &m.source);
        let reference_value = m.measure.evaluate(&Subject {
            trace: &trace,
            file: Some((&bytes, ty, &pattern)),
        });
        let tolerance: Tolerance = rc.tolerance.unwrap_or(m.tolerance);
        c.tolerance = Some(tolerance);
        match (reference_value, self.ours(m, session)) {
            (Ok(e), Ok(a)) => {
                c.mismatches = check(&e, &a, &tolerance);
                c.status = if c.mismatches.is_empty() {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                };
                c.expected = Some(e);
                c.actual = Some(a);
                c.note = Some(provenance_note);
            }
            (Err(e), _) => c.note = Some(format!("on the reference: {e}")),
            (_, Err(e)) => c.note = Some(e),
        }
        c
    }

    fn machine_check(&self, machine: &spec::MachineSpec, exports: &[ExportInfo]) -> Check {
        let mut c = Check {
            measurement: "sew-out".into(),
            kind: "machineRecord".into(),
            tier: Tier::Machine,
            status: CheckStatus::Error,
            expected: Some(json!("pass")),
            actual: None,
            tolerance: None,
            file: Some(format!("compat/machine/{}", machine.record)),
            mismatches: Vec::new(),
            differences: Vec::new(),
            unlisted_differences: 0,
            note: None,
            dropped_repeats: None,
        };
        let path = match safe_relative(&machine.record) {
            Ok(p) => self.config.machine_root.join(p),
            Err(e) => {
                c.note = Some(e);
                return c;
            }
        };
        match provenance::load_machine_record(&path, &self.spec.id) {
            Ok(None) => {
                c.status = CheckStatus::Pending;
                c.note = Some(format!("pending sew-out: {}", machine.make));
            }
            Ok(Some(r)) => {
                let current = exports
                    .iter()
                    .find(|x| x.path == r.file)
                    .map(|x| x.sha256.as_str());
                if current != Some(r.file_sha256.as_str()) {
                    c.status = CheckStatus::Pending;
                    c.note = Some(format!(
                        "the record is for another version of {}; sew the current export",
                        r.file
                    ));
                } else {
                    c.actual = Some(json!(r.result));
                    c.status = if r.result == "pass" {
                        CheckStatus::Pass
                    } else {
                        CheckStatus::Fail
                    };
                    c.note = Some(format!("{} by {} on {}", r.machine, r.sewn_by, r.date));
                }
            }
            Err(e) => c.note = Some(e),
        }
        c
    }
}

/// Runs specs from a directory into a report, with the matrix check when `matrix` is given.
pub fn run_with_matrix(config: &Config, matrix: Option<&Path>) -> Result<Report, String> {
    let mut report = run(config)?;
    if let Some(m) = matrix {
        let text = std::fs::read_to_string(m).map_err(|e| format!("{}: {e}", m.display()))?;
        report.unknown_rows = crate::report::unknown_rows(&report, &text);
    }
    Ok(report)
}
