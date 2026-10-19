//! Reference files (PE-DESIGN 11 output, compat/reference/) and sew-out records
//! (compat/machine/), each with the provenance that lets a result claim their tier.
//!
//! A reference file counts only when compat/reference/MANIFEST.json lists it with who made
//! it, the PE-DESIGN version, the date and the settings, and its SHA-256 matches. A file
//! without a manifest entry, or an entry whose file is missing or changed, is an error, never
//! a pass: nothing is promoted to the reference tier without a provenance record.

use crate::sha256;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MANIFEST_SCHEMA: &str = "openstitch-compat-reference/1";
pub const MACHINE_SCHEMA: &str = "openstitch-compat-machine/1";
pub const MANIFEST: &str = "MANIFEST.json";
/// Largest reference file or record read.
pub const MAX_FILE_BYTES: u64 = 32 << 20;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    #[serde(default)]
    pub files: Vec<Provenance>,
}

/// Where one reference file came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    /// Path under compat/reference/.
    pub file: String,
    /// Spec id and reference id it answers.
    pub spec: String,
    pub reference: String,
    pub made_by: String,
    /// e.g. "11.30".
    pub pe_design_version: String,
    /// YYYY-MM-DD.
    pub date: String,
    /// Operating system / VM it ran on.
    pub os: String,
    /// Every non-default setting, and the Save As options (PES version).
    pub settings: String,
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
}

/// One sew-out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MachineRecord {
    pub schema: String,
    pub spec: String,
    /// `pass` or `fail`.
    pub result: String,
    /// Model and firmware.
    pub machine: String,
    /// The file that was sewn and its SHA-256.
    pub file: String,
    pub file_sha256: String,
    pub fabric: String,
    pub stabilizer: String,
    pub sewn_by: String,
    pub date: String,
    pub observations: String,
}

fn date_ok(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn hash_ok(h: &str) -> bool {
    h.len() == 64
        && h.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

impl Provenance {
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        for (k, v) in [
            ("madeBy", &self.made_by),
            ("peDesignVersion", &self.pe_design_version),
            ("os", &self.os),
            ("settings", &self.settings),
        ] {
            if v.trim().is_empty() {
                p.push(format!("{k} is empty"));
            }
        }
        if !date_ok(&self.date) {
            p.push("date must be YYYY-MM-DD".into());
        }
        if !hash_ok(&self.sha256) {
            p.push("sha256 must be 64 lower-case hex digits".into());
        }
        p
    }
}

impl MachineRecord {
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.schema != MACHINE_SCHEMA {
            p.push(format!("schema must be {MACHINE_SCHEMA:?}"));
        }
        if self.result != "pass" && self.result != "fail" {
            p.push("result must be pass or fail".into());
        }
        for (k, v) in [
            ("machine", &self.machine),
            ("file", &self.file),
            ("fabric", &self.fabric),
            ("stabilizer", &self.stabilizer),
            ("sewnBy", &self.sewn_by),
            ("observations", &self.observations),
        ] {
            if v.trim().is_empty() {
                p.push(format!("{k} is empty"));
            }
        }
        if !date_ok(&self.date) {
            p.push("date must be YYYY-MM-DD".into());
        }
        if !hash_ok(&self.file_sha256) {
            p.push("fileSha256 must be 64 lower-case hex digits".into());
        }
        p
    }
}

/// A regular file (not a symlink) no larger than [`MAX_FILE_BYTES`]; `Ok(None)` if absent.
pub fn read_limited(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if !meta.is_file() {
        return Err(format!("{}: not a regular file", path.display()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{}: larger than {MAX_FILE_BYTES} bytes",
            path.display()
        ));
    }
    std::fs::read(path)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads compat/reference/MANIFEST.json (an absent manifest is an empty one).
pub fn load_manifest(reference_root: &Path) -> Result<Manifest, String> {
    let Some(bytes) = read_limited(&reference_root.join(MANIFEST))? else {
        return Ok(Manifest {
            schema: MANIFEST_SCHEMA.into(),
            files: Vec::new(),
        });
    };
    let m: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("{MANIFEST}: {e}"))?;
    if m.schema != MANIFEST_SCHEMA {
        return Err(format!("{MANIFEST}: schema must be {MANIFEST_SCHEMA:?}"));
    }
    Ok(m)
}

/// What is known about one reference file.
#[derive(Clone, Debug, PartialEq)]
pub enum Reference {
    /// Neither the file nor a manifest entry exists yet.
    Pending,
    /// Present with a valid provenance record.
    Available {
        bytes: Vec<u8>,
        provenance: Box<Provenance>,
    },
    /// Present without provenance, listed but missing, or changed since it was recorded.
    Invalid(String),
}

pub fn resolve(
    reference_root: &Path,
    manifest: &Manifest,
    file: &str,
    spec: &str,
    reference: &str,
) -> Reference {
    let path = match crate::spec::safe_relative(file) {
        Ok(p) => reference_root.join(p),
        Err(e) => return Reference::Invalid(e),
    };
    let entry = manifest.files.iter().find(|p| p.file == file);
    let bytes = match read_limited(&path) {
        Ok(b) => b,
        Err(e) => return Reference::Invalid(e),
    };
    match (bytes, entry) {
        (None, None) => Reference::Pending,
        (None, Some(_)) => {
            Reference::Invalid(format!("{MANIFEST} lists {file} but the file is missing"))
        }
        (Some(_), None) => Reference::Invalid(format!(
            "{file} has no provenance entry in {MANIFEST}; it is not used until it has one"
        )),
        (Some(bytes), Some(p)) => {
            let mut problems = p.problems();
            if p.spec != spec || p.reference != reference {
                problems.push(format!(
                    "the entry is for {}/{}, not {spec}/{reference}",
                    p.spec, p.reference
                ));
            }
            let actual = sha256::hex(&bytes);
            if hash_ok(&p.sha256) && actual != p.sha256 {
                problems.push(format!(
                    "sha256 is {actual}, the manifest says {}",
                    p.sha256
                ));
            }
            if problems.is_empty() {
                Reference::Available {
                    bytes,
                    provenance: Box::new(p.clone()),
                }
            } else {
                Reference::Invalid(format!("{file}: {}", problems.join("; ")))
            }
        }
    }
}

/// A sew-out record: `Ok(None)` when it does not exist yet.
pub fn load_machine_record(path: &Path, spec: &str) -> Result<Option<MachineRecord>, String> {
    let Some(bytes) = read_limited(path)? else {
        return Ok(None);
    };
    let r: MachineRecord =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut problems = r.problems();
    if r.spec != spec {
        problems.push(format!("record is for {}, not {spec}", r.spec));
    }
    if problems.is_empty() {
        Ok(Some(r))
    } else {
        Err(format!("{}: {}", path.display(), problems.join("; ")))
    }
}
