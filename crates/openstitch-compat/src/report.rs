//! Run results: the JSON report, its markdown summary, and the generated tier table that
//! docs/FEATURE_MATRIX.md links to (docs/COMPATIBILITY_STATUS.md).
//!
//! Everything here is deterministic: specs in path order, numbers rounded to 4 decimals, no
//! times or absolute paths. The status table holds no measured numbers at all, so it is the
//! same on every platform.

use crate::compare::{Difference, DroppedRepeats};
use crate::spec::{Covers, RowRef, Tier};
use crate::tolerance::{Mismatch, Tolerance};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub const REPORT_SCHEMA: &str = "openstitch-compat-report/1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pass,
    Fail,
    Error,
    PendingReference,
    Skipped,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::Error => "error",
            Status::PendingReference => "pending reference",
            Status::Skipped => "skipped",
        }
    }
}

/// Ordered from best to worst, so the worst of several is their maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Pass,
    /// The reference file or sew-out record does not exist yet.
    Pending,
    Fail,
    Error,
}

impl CheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckStatus::Pass => "pass",
            CheckStatus::Pending => "pending",
            CheckStatus::Fail => "fail",
            CheckStatus::Error => "error",
        }
    }
}

/// One measurement checked at one tier.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub measurement: String,
    pub kind: String,
    pub tier: Tier,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<Tolerance>,
    /// The file the expected value came from (reference, oracle or sew-out record).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mismatches: Vec<Mismatch>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub differences: Vec<Difference>,
    #[serde(skip_serializing_if = "is_zero")]
    pub unlisted_differences: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Zero-length stitches a `dropRepeats` compare left out, per side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropped_repeats: Option<DroppedRepeats>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInfo {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
    pub stitches: usize,
    pub color_blocks: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecResult {
    pub id: String,
    pub area: String,
    pub title: String,
    pub path: String,
    pub covers: Covers,
    pub status: Status,
    /// Highest tier reached: every check of that tier and of every lower tier passed.
    pub tier: Option<Tier>,
    /// The worst check status at each tier that has checks.
    pub tiers: BTreeMap<Tier, CheckStatus>,
    /// Checks waiting for a reference file or a sew-out record.
    pub pending: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    pub exports: Vec<ExportInfo>,
    pub checks: Vec<Check>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InvalidSpec {
    pub path: String,
    pub problems: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub specs: usize,
    pub pass: usize,
    pub fail: usize,
    pub error: usize,
    pub pending_reference: usize,
    pub skipped: usize,
    pub invalid: usize,
    /// Passing specs by the highest tier they reached.
    pub tiers: BTreeMap<Tier, usize>,
    pub checks: usize,
    pub checks_pending: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub schema: String,
    pub summary: Totals,
    pub specs: Vec<SpecResult>,
    pub invalid: Vec<InvalidSpec>,
    /// Feature-matrix rows the specs name that docs/FEATURE_MATRIX.md does not have.
    #[serde(rename = "unknownRows", skip_serializing_if = "Vec::is_empty")]
    pub unknown_rows: Vec<String>,
}

impl Report {
    pub fn new(specs: Vec<SpecResult>, invalid: Vec<InvalidSpec>) -> Self {
        let mut t = Totals {
            specs: specs.len(),
            invalid: invalid.len(),
            ..Totals::default()
        };
        for s in &specs {
            match s.status {
                Status::Pass => t.pass += 1,
                Status::Fail => t.fail += 1,
                Status::Error => t.error += 1,
                Status::PendingReference => t.pending_reference += 1,
                Status::Skipped => t.skipped += 1,
            }
            if let Some(tier) = s.tier {
                *t.tiers.entry(tier).or_default() += 1;
            }
            t.checks += s.checks.len();
            t.checks_pending += s.pending;
        }
        Report {
            schema: REPORT_SCHEMA.into(),
            summary: t,
            specs,
            invalid,
            unknown_rows: Vec::new(),
        }
    }

    /// No spec failed or errored, every spec file is valid and every row exists.
    pub fn ok(&self) -> bool {
        self.summary.fail == 0
            && self.summary.error == 0
            && self.invalid.is_empty()
            && self.unknown_rows.is_empty()
    }

    /// Pretty JSON with numbers rounded to 4 decimals.
    pub fn to_json(&self) -> String {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        let mut s = serde_json::to_string_pretty(&round(v)).unwrap_or_default();
        s.push('\n');
        s
    }
}

/// Numbers rounded to 4 decimals (and -0 to 0), integers kept.
pub fn round(v: Value) -> Value {
    match v {
        Value::Number(n) if n.is_f64() => {
            let f = n.as_f64().unwrap_or(0.0);
            let r = (f * 10_000.0).round() / 10_000.0;
            let r = if r == 0.0 { 0.0 } else { r };
            serde_json::Number::from_f64(r).map_or(Value::Null, Value::Number)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(round).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, round(v))).collect()),
        v => v,
    }
}

fn short(v: &Value) -> String {
    let s = round(v.clone()).to_string();
    if s.chars().count() > 120 {
        let cut: String = s.chars().take(117).collect();
        format!("{cut}…")
    } else {
        s
    }
}

fn tier_str(t: Option<Tier>) -> &'static str {
    t.map_or("—", Tier::as_str)
}

fn covers_str(c: &Covers) -> String {
    let mut parts: Vec<String> = c.audit.iter().map(|n| format!("audit {n}")).collect();
    parts.extend(
        c.feature_matrix
            .iter()
            .map(|r| format!("{} / {}", r.area, r.feature)),
    );
    parts.join("; ")
}

/// Short markdown summary: totals, one row per spec, then every difference.
pub fn summary_markdown(r: &Report) -> String {
    let t = &r.summary;
    let mut s = String::new();
    let _ = writeln!(s, "# OpenStitch compatibility report\n");
    let _ = writeln!(
        s,
        "Specs: {} · pass {} · fail {} · error {} · pending reference {} · skipped {} · invalid {}\n",
        t.specs, t.pass, t.fail, t.error, t.pending_reference, t.skipped, t.invalid
    );
    let tiers: Vec<String> = Tier::ALL
        .iter()
        .map(|tier| format!("{} {}", tier.as_str(), t.tiers.get(tier).unwrap_or(&0)))
        .collect();
    let _ = writeln!(
        s,
        "Highest tier reached by passing specs: {}. Checks: {}, waiting for a reference or sew-out: {}.\n",
        tiers.join(" · "),
        t.checks,
        t.checks_pending
    );
    let _ = writeln!(s, "| Spec | Covers | Status | Tier | Pending |");
    let _ = writeln!(s, "|---|---|---|---|---|");
    for spec in &r.specs {
        let _ = writeln!(
            s,
            "| `{}/{}` | {} | {} | {} | {} |",
            spec.area,
            spec.id,
            covers_str(&spec.covers),
            spec.status.as_str(),
            tier_str(spec.tier),
            spec.pending
        );
    }
    for inv in &r.invalid {
        let _ = writeln!(s, "\n**Invalid spec `{}`:**", inv.path);
        for p in &inv.problems {
            let _ = writeln!(s, "- {p}");
        }
    }
    for row in &r.unknown_rows {
        let _ = writeln!(s, "\n**Unknown FEATURE_MATRIX row:** {row}");
    }
    let mut details = String::new();
    for spec in &r.specs {
        let failing: Vec<&Check> = spec
            .checks
            .iter()
            .filter(|c| matches!(c.status, CheckStatus::Fail | CheckStatus::Error))
            .collect();
        if failing.is_empty() && spec.error.is_none() {
            continue;
        }
        let _ = writeln!(details, "\n### `{}` — {}\n", spec.id, spec.status.as_str());
        if let Some(e) = &spec.error {
            let _ = writeln!(details, "- error: {e}");
        }
        for c in failing {
            let _ = write!(
                details,
                "- `{}` ({}, {}): ",
                c.measurement,
                c.kind,
                c.tier.as_str()
            );
            match (&c.expected, &c.actual) {
                (Some(e), Some(a)) => {
                    let _ = write!(details, "expected {}, actual {}", short(e), short(a));
                }
                (None, Some(a)) => {
                    let _ = write!(details, "actual {}", short(a));
                }
                _ => {}
            }
            if let Some(n) = &c.note {
                let _ = write!(details, " — {n}");
            }
            let _ = writeln!(details);
            for m in &c.mismatches {
                let _ = writeln!(
                    details,
                    "  - at `{}`: expected {}, actual {}",
                    if m.at.is_empty() { "/" } else { &m.at },
                    short(&m.expected),
                    short(&m.actual)
                );
            }
            for d in &c.differences {
                let mut loc = Vec::new();
                if let Some(b) = d.block {
                    loc.push(format!("block {b}"));
                }
                if let Some(i) = d.expected_index {
                    loc.push(format!("expected #{i}"));
                }
                if let Some(i) = d.actual_index {
                    loc.push(format!("actual #{i}"));
                }
                let _ = writeln!(
                    details,
                    "  - {}{}: expected {}, actual {}",
                    d.what,
                    if loc.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", loc.join(", "))
                    },
                    short(&d.expected),
                    short(&d.actual)
                );
            }
            if c.unlisted_differences > 0 {
                let _ = writeln!(details, "  - … {} more", c.unlisted_differences);
            }
        }
    }
    if !details.is_empty() {
        let _ = writeln!(s, "\n## Failures and differences");
        s.push_str(&details);
    }
    let dropped: Vec<(&SpecResult, &Check, DroppedRepeats)> = r
        .specs
        .iter()
        .flat_map(|sp| {
            sp.checks
                .iter()
                .filter_map(move |c| c.dropped_repeats.map(|d| (sp, c, d)))
        })
        .collect();
    if !dropped.is_empty() {
        let _ = writeln!(
            s,
            "\n## Zero-length stitches left out by `dropRepeats`\n\n| Spec | Measurement | Status | Reference | OpenStitch |\n|---|---|---|---|---|"
        );
        for (sp, c, d) in dropped {
            let _ = writeln!(
                s,
                "| `{}` | `{}` | {} | {} | {} |",
                sp.id,
                c.measurement,
                c.status.as_str(),
                d.expected,
                d.actual
            );
        }
    }
    let pending: Vec<(&SpecResult, &Check)> = r
        .specs
        .iter()
        .flat_map(|sp| sp.checks.iter().map(move |c| (sp, c)))
        .filter(|(_, c)| c.status == CheckStatus::Pending)
        .collect();
    if !pending.is_empty() {
        let files: BTreeSet<(String, &str, Tier)> = pending
            .iter()
            .map(|(sp, c)| (c.file.clone().unwrap_or_default(), sp.id.as_str(), c.tier))
            .collect();
        let _ = writeln!(s, "\n## Waiting for files\n");
        for (file, id, tier) in files {
            let _ = writeln!(s, "- `{file}` ({}, spec `{id}`)", tier.as_str());
        }
    }
    s
}

/// The rows of docs/FEATURE_MATRIX.md: `(line, area, feature)`.
pub fn matrix_rows(matrix: &str) -> Vec<(usize, String, String)> {
    matrix
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            (l.starts_with('|')
                && cells.len() > 3
                && cells[1] != "Area"
                && !cells[1].starts_with("---"))
            .then(|| (i + 1, cells[1].to_string(), cells[2].to_string()))
        })
        .collect()
}

/// Row references that match no row, or several, in the matrix.
pub fn unknown_rows(report: &Report, matrix: &str) -> Vec<String> {
    let rows = matrix_rows(matrix);
    let mut refs: BTreeSet<&RowRef> = BTreeSet::new();
    for s in &report.specs {
        refs.extend(&s.covers.feature_matrix);
    }
    refs.into_iter()
        .filter_map(|r| {
            let n = rows
                .iter()
                .filter(|(_, a, f)| *a == r.area && f.starts_with(&r.feature))
                .count();
            (n != 1).then(|| {
                format!(
                    "{} / {}: {} rows match",
                    r.area,
                    r.feature,
                    if n == 0 {
                        "no".to_string()
                    } else {
                        n.to_string()
                    }
                )
            })
        })
        .collect()
}

/// Weakest result among the specs covering one row or item.
fn combined(specs: &[&SpecResult]) -> String {
    if specs
        .iter()
        .any(|s| matches!(s.status, Status::Fail | Status::Error))
    {
        return "**failing**".into();
    }
    let passing: Vec<Tier> = specs.iter().filter_map(|s| s.tier).collect();
    let pending = specs.iter().any(|s| s.pending > 0);
    let mut out = if passing.len() == specs.len() && !passing.is_empty() {
        passing.iter().min().map_or("—", |t| t.as_str()).to_string()
    } else if !passing.is_empty() {
        format!(
            "{} (partly)",
            passing.iter().min().map_or("—", |t| t.as_str())
        )
    } else {
        "—".into()
    };
    if pending {
        out.push_str("; reference pending");
    }
    out
}

/// docs/COMPATIBILITY_STATUS.md: one row per FEATURE_MATRIX row and per audit item.
pub fn status_markdown(report: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Compatibility status\n");
    let _ = writeln!(
        s,
        "<!-- Generated by `stitchtool compat --status docs/COMPATIBILITY_STATUS.md` from \
         compat/specs. Do not edit by hand; CI checks it is current. -->\n"
    );
    let _ = writeln!(
        s,
        "The tier each [FEATURE_MATRIX.md](FEATURE_MATRIX.md) row and \
         [audit](COMPATIBILITY_AUDIT.md) item has reached in the compatibility harness \
         ([COMPATIBILITY_TESTS.md](COMPATIBILITY_TESTS.md)). A row is only as far as the weakest \
         spec covering it. Tiers, weakest first: internal-regression (our own earlier output or \
         the guide's description), oracle (an independent implementation), reference (PE-DESIGN \
         11 output supplied by the owner), machine (a sew-out record). \"Reference pending\" \
         means a spec is ready for a PE-DESIGN file that does not exist yet.\n"
    );
    let t = &report.summary;
    let _ = writeln!(
        s,
        "Specs: {} · pass {} · fail {} · error {} · pending reference {} · skipped {}.\n",
        t.specs, t.pass, t.fail, t.error, t.pending_reference, t.skipped
    );
    let mut rows: BTreeMap<&RowRef, Vec<&SpecResult>> = BTreeMap::new();
    let mut items: BTreeMap<u32, Vec<&SpecResult>> = BTreeMap::new();
    for spec in &report.specs {
        for r in &spec.covers.feature_matrix {
            rows.entry(r).or_default().push(spec);
        }
        for n in &spec.covers.audit {
            items.entry(*n).or_default().push(spec);
        }
    }
    let ids = |v: &[&SpecResult]| {
        v.iter()
            .map(|s| format!("`{}`", s.id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let _ = writeln!(s, "## By FEATURE_MATRIX row\n");
    let _ = writeln!(s, "| Area | Feature | Tier | Specs |");
    let _ = writeln!(s, "|---|---|---|---|");
    for (r, specs) in &rows {
        let _ = writeln!(
            s,
            "| {} | {}… | {} | {} |",
            r.area,
            r.feature,
            combined(specs),
            ids(specs)
        );
    }
    let _ = writeln!(s, "\n## By audit item\n");
    let _ = writeln!(s, "| Item | Tier | Specs |");
    let _ = writeln!(s, "|---|---|---|");
    for (n, specs) in &items {
        let _ = writeln!(s, "| {n} | {} | {} |", combined(specs), ids(specs));
    }
    let _ = writeln!(s, "\n## By spec\n");
    let _ = writeln!(
        s,
        "| Spec | Status | Tier | Internal regression | Oracle | Reference | Machine |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|");
    for spec in &report.specs {
        let per_tier: Vec<&str> = Tier::ALL
            .iter()
            .map(|t| spec.tiers.get(t).map_or("—", |c| c.as_str()))
            .collect();
        let _ = writeln!(
            s,
            "| `{}/{}` | {} | {} | {} |",
            spec.area,
            spec.id,
            spec.status.as_str(),
            tier_str(spec.tier),
            per_tier.join(" | ")
        );
    }
    s
}
