//! `stitchtool compat`: runs the specs and writes the report, the summary and (optionally) the
//! generated status table, or checks that the committed table is current.

use crate::report::{status_markdown, summary_markdown};
use crate::runner::{Config, run_with_matrix};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: stitchtool compat [--specs DIR] [--reference DIR] [--machine DIR] \
[--work DIR] [--report FILE] [--summary FILE] [--matrix FILE] [--status FILE] [--check-status FILE] [--only ID]";

#[derive(Debug, PartialEq)]
pub struct Options {
    pub config: Config,
    pub report: PathBuf,
    pub summary: PathBuf,
    pub matrix: Option<PathBuf>,
    pub status: Option<PathBuf>,
    pub check_status: Option<PathBuf>,
}

/// Parses the arguments after `compat`; paths default to the repository layout under the
/// current directory.
pub fn parse(args: &[&str]) -> Result<Options, String> {
    let mut o = Options {
        config: Config::in_repo("."),
        report: PathBuf::from("target/compat/report.json"),
        summary: PathBuf::from("target/compat/summary.md"),
        matrix: Some(PathBuf::from("docs/FEATURE_MATRIX.md")),
        status: None,
        check_status: None,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .map(|v| PathBuf::from(*v))
                .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))
        };
        match *flag {
            "--specs" => o.config.specs_dir = value()?,
            "--reference" => o.config.reference_root = value()?,
            "--machine" => o.config.machine_root = value()?,
            "--work" => o.config.work_dir = value()?,
            "--report" => o.report = value()?,
            "--summary" => o.summary = value()?,
            "--matrix" => o.matrix = Some(value()?),
            "--status" => o.status = Some(value()?),
            "--check-status" => o.check_status = Some(value()?),
            "--only" => o.config.only = Some(value()?.to_string_lossy().into_owned()),
            _ => return Err(USAGE.into()),
        }
    }
    Ok(o)
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Runs and writes everything. `Ok(true)` when every spec passed, is pending a reference or is
/// skipped, every spec file is valid and the status table (if checked) is current.
pub fn run(o: &Options) -> Result<bool, String> {
    let report = run_with_matrix(&o.config, o.matrix.as_deref())?;
    let summary = summary_markdown(&report);
    write(&o.report, &report.to_json())?;
    write(&o.summary, &summary)?;
    let status = status_markdown(&report);
    if let Some(p) = &o.status {
        write(p, &status)?;
    }
    let mut ok = report.ok();
    if let Some(p) = &o.check_status {
        let committed = std::fs::read_to_string(p).unwrap_or_default();
        if committed.replace("\r\n", "\n") != status {
            eprintln!(
                "{} is out of date: run `stitchtool compat --status {}` and commit it",
                p.display(),
                p.display()
            );
            ok = false;
        }
    }
    // Totals and the per-spec table: the head of the summary.
    for line in summary.lines().take_while(|l| !l.starts_with("## ")) {
        println!("{line}");
    }
    println!(
        "\nReport: {} · summary: {}",
        o.report.display(),
        o.summary.display()
    );
    Ok(ok)
}
