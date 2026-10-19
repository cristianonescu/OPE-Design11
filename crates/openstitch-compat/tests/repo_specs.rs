//! The repository's own specs (compat/specs): all valid, all passing, every FEATURE_MATRIX
//! row they name exists, no reference or machine tier is claimed without files, and
//! docs/COMPATIBILITY_STATUS.md is the table this run generates.

use openstitch_compat::Tier;
use openstitch_compat::report::{Status, status_markdown, summary_markdown};
use openstitch_compat::runner::{Config, run_with_matrix};
use std::path::Path;

#[test]
fn repository_specs_pass_and_the_status_table_is_current() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let work = std::env::temp_dir().join(format!("openstitch-compat-repo-{}", std::process::id()));
    let config = Config {
        work_dir: work.clone(),
        ..Config::in_repo(&root)
    };
    let report = run_with_matrix(&config, Some(&root.join("docs/FEATURE_MATRIX.md"))).expect("run");
    let summary = summary_markdown(&report);
    assert!(report.ok(), "{summary}");
    assert!(report.specs.len() >= 20, "{summary}");
    assert!(
        report
            .specs
            .iter()
            .all(|s| s.status == Status::Pass && s.tier.is_some()),
        "{summary}"
    );
    // Every audit item the harness was built for has a spec.
    for item in [1, 2, 3, 7, 14, 15, 16, 17, 18, 19] {
        assert!(
            report.specs.iter().any(|s| s.covers.audit.contains(&item)),
            "audit item {item} has no spec"
        );
    }
    // No PE-DESIGN file or sew-out record is in the repository yet, so nothing is at those tiers.
    let has_files = |dir: &str| {
        std::fs::read_dir(root.join(dir)).is_ok_and(|d| {
            d.filter_map(Result::ok)
                .any(|e| !matches!(e.file_name().to_str(), Some("README.md" | "MANIFEST.json")))
        })
    };
    if !has_files("compat/reference") && !has_files("compat/machine") {
        assert!(
            report.specs.iter().all(|s| s.tier < Some(Tier::Reference)),
            "{summary}"
        );
    }
    let committed = std::fs::read_to_string(root.join("docs/COMPATIBILITY_STATUS.md"))
        .expect("docs/COMPATIBILITY_STATUS.md")
        .replace("\r\n", "\n");
    assert_eq!(
        committed,
        status_markdown(&report),
        "docs/COMPATIBILITY_STATUS.md is stale: run `cargo run -p stitchtool -- compat --status docs/COMPATIBILITY_STATUS.md`"
    );
    let _ = std::fs::remove_dir_all(work);
}
