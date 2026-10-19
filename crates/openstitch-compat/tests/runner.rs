//! The runner on synthetic spec directories: pending references, provenance, tier rules,
//! sew-out records, invalid specs, sandboxing, determinism and the generated status table.
//!
//! "Reference" files here are OpenStitch's own exports copied in by the test, standing in for
//! PE-DESIGN output; they exist only in the test's temporary directory.

use openstitch_compat::report::{CheckStatus, Status, status_markdown, summary_markdown};
use openstitch_compat::runner::{Config, run, run_with_matrix};
use openstitch_compat::sha256;
use openstitch_compat::{Report, Tier};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_root(tag: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-compat-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["specs", "reference", "machine", "work"] {
        std::fs::create_dir_all(dir.join(sub)).expect("temp dir");
    }
    dir.canonicalize().expect("canonical")
}

fn config(root: &Path) -> Config {
    Config {
        repo_root: repo_root(),
        specs_dir: root.join("specs"),
        reference_root: root.join("reference"),
        machine_root: root.join("machine"),
        work_dir: root.join("work"),
        only: None,
    }
}

fn write_spec(root: &Path, spec: &Value) {
    let area = spec["area"].as_str().expect("area");
    let id = spec["id"].as_str().expect("id");
    let dir = root.join("specs").join(area);
    std::fs::create_dir_all(&dir).expect("area dir");
    std::fs::write(
        dir.join(format!("{id}.json")),
        serde_json::to_string_pretty(spec).expect("json"),
    )
    .expect("write spec");
}

const RAILS: &str = r#"[[[9, 10], [16, 10]], [[9, 12], [16, 12]]]"#;

/// The audit's satin column: an expected check per measurement, a reference check on some.
fn satin(id: &str) -> Value {
    let rails: Value = serde_json::from_str(RAILS).expect("rails");
    let src = json!({"export": "satin.pes", "align": {"anchor": "boundsMin", "to": [10, 10]}});
    json!({
        "schema": "openstitch-compat/1",
        "id": id,
        "area": "stitches",
        "title": "Satin column",
        "description": "A 5 × 2 mm satin column.",
        "covers": {"audit": [1], "featureMatrix": [{"area": "Region sew types", "feature": "Satin"}]},
        "evidence": [{"kind": "audit", "note": "item 1"}],
        "script": [
            {"command": "design.new", "params": {"width": 100, "height": 100}},
            {"command": "shape.rectangle", "params": {"x": 10, "y": 10, "width": 5, "height": 2}},
            {"command": "object.setRegion", "params": {"id": 1, "type": "satin", "angle": 90, "density": 3}}
        ],
        "exports": [{"path": "satin.pes"}],
        "references": [{"id": "pe-design", "file": format!("stitches/{id}.pes"), "make": "Draw the column."}],
        "measurements": [
            {"id": "stitches", "measure": {"kind": "stitchSegments"}, "source": src, "expected": 15,
             "tolerance": {"count": 1}, "evidence": "audit", "reference": {"tolerance": {"percent": 10}}},
            {"id": "alternation", "measure": {"kind": "sideAlternation", "rails": rails}, "source": src,
             "expected": 1.0, "evidence": "audit", "reference": {}},
            {"id": "versus-reference", "measure": {"kind": "compare", "options": {"origin": "boundsMin"}},
             "source": {"export": "satin.pes"}, "evidence": "PE-DESIGN output", "reference": {}}
        ]
    })
}

fn spec_result<'a>(r: &'a Report, id: &str) -> &'a openstitch_compat::report::SpecResult {
    r.specs.iter().find(|s| s.id == id).expect("spec in report")
}

/// Puts OpenStitch's own export of `id` in the reference directory (as a stand-in) and
/// records it in the manifest; returns its bytes.
fn supply_reference(root: &Path, id: &str, bytes: Option<Vec<u8>>, made_by: &str) -> Vec<u8> {
    let bytes = bytes.unwrap_or_else(|| {
        std::fs::read(root.join("work").join(id).join("satin.pes")).expect("export")
    });
    let file = format!("stitches/{id}.pes");
    std::fs::create_dir_all(root.join("reference/stitches")).expect("dir");
    std::fs::write(root.join("reference").join(&file), &bytes).expect("ref");
    let manifest = json!({
        "schema": "openstitch-compat-reference/1",
        "files": [{
            "file": file, "spec": id, "reference": "pe-design", "madeBy": made_by,
            "peDesignVersion": "11.30", "date": "2026-10-19", "os": "test",
            "settings": "defaults", "sha256": sha256::hex(&bytes)
        }]
    });
    std::fs::write(
        root.join("reference/MANIFEST.json"),
        serde_json::to_string_pretty(&manifest).expect("json"),
    )
    .expect("manifest");
    bytes
}

#[test]
fn a_missing_reference_is_pending_not_failed() {
    let root = temp_root("pending");
    write_spec(&root, &satin("satin-a"));
    // A spec whose only check needs the reference.
    let mut only = satin("satin-b");
    only["measurements"] = json!([{"id": "versus-reference", "measure": {"kind": "compare"},
        "source": {"export": "satin.pes"}, "evidence": "PE-DESIGN output", "reference": {}}]);
    write_spec(&root, &only);
    let r = run(&config(&root)).expect("run");
    assert!(r.ok(), "{}", summary_markdown(&r));

    let a = spec_result(&r, "satin-a");
    assert_eq!(a.status, Status::Pass);
    assert_eq!(a.tier, Some(Tier::InternalRegression));
    assert_eq!(a.tiers.get(&Tier::Reference), Some(&CheckStatus::Pending));
    assert_eq!(a.pending, 3);
    let b = spec_result(&r, "satin-b");
    assert_eq!(b.status, Status::PendingReference);
    assert_eq!(b.tier, None);
    assert_eq!(r.summary.pending_reference, 1);
    let md = summary_markdown(&r);
    assert!(md.contains("pending reference 1"), "{md}");
    assert!(md.contains("`compat/reference/stitches/satin-b.pes` (reference, spec `satin-b`)"));
    assert!(!md.contains("## Failures"), "{md}");
}

#[test]
fn a_reference_without_provenance_is_never_used() {
    let root = temp_root("noprov");
    write_spec(&root, &satin("satin-c"));
    run(&config(&root)).expect("first run");
    let bytes = std::fs::read(root.join("work/satin-c/satin.pes")).expect("export");
    std::fs::create_dir_all(root.join("reference/stitches")).expect("dir");
    std::fs::write(root.join("reference/stitches/satin-c.pes"), &bytes).expect("ref");
    let r = run(&config(&root)).expect("run");
    let s = spec_result(&r, "satin-c");
    assert_eq!(s.status, Status::Error);
    assert_eq!(s.tier, None);
    assert!(!r.ok());
    let note = s
        .checks
        .iter()
        .find(|c| c.tier == Tier::Reference)
        .and_then(|c| c.note.clone())
        .unwrap_or_default();
    assert!(note.contains("no provenance entry"), "{note}");

    // Listed but changed since it was recorded.
    supply_reference(&root, "satin-c", Some(bytes.clone()), "Owner");
    let mut changed = bytes;
    let last = changed.len() - 1;
    changed[last] ^= 1;
    std::fs::write(root.join("reference/stitches/satin-c.pes"), &changed).expect("ref");
    let r = run(&config(&root)).expect("run");
    assert_eq!(spec_result(&r, "satin-c").status, Status::Error);

    // Provenance with an empty field is refused too.
    supply_reference(&root, "satin-c", None, " ");
    let r = run(&config(&root)).expect("run");
    assert_eq!(spec_result(&r, "satin-c").status, Status::Error);
}

#[test]
fn a_reference_with_provenance_reaches_the_reference_tier() {
    let root = temp_root("ref");
    write_spec(&root, &satin("satin-d"));
    run(&config(&root)).expect("first run");
    supply_reference(&root, "satin-d", None, "Owner");
    let r = run(&config(&root)).expect("run");
    let s = spec_result(&r, "satin-d");
    assert_eq!(s.status, Status::Pass, "{}", summary_markdown(&r));
    assert_eq!(s.tier, Some(Tier::Reference));
    assert_eq!(s.pending, 0);
    let notes: Vec<_> = s
        .checks
        .iter()
        .filter(|c| c.tier == Tier::Reference)
        .map(|c| c.note.clone().unwrap_or_default())
        .collect();
    assert!(
        notes.iter().all(|n| n.contains("PE-DESIGN 11.30 by Owner")),
        "{notes:?}"
    );

    // A reference that sews one needle point elsewhere fails, with the difference located.
    let mut pattern = openstitch_formats::pes::read(
        &std::fs::read(root.join("work/satin-d/satin.pes")).expect("export"),
    )
    .expect("read");
    let k = pattern
        .stitches
        .iter()
        .rposition(|s| s.command == openstitch_core::Command::Stitch)
        .expect("stitch");
    pattern.stitches[k].x += 7.0;
    let moved = openstitch_formats::pes::write(&pattern).expect("write");
    supply_reference(&root, "satin-d", Some(moved), "Owner");
    let r = run(&config(&root)).expect("run");
    let s = spec_result(&r, "satin-d");
    assert_eq!(s.status, Status::Fail);
    assert_eq!(s.tier, None);
    let cmp = s
        .checks
        .iter()
        .find(|c| c.measurement == "versus-reference")
        .expect("compare check");
    assert_eq!(cmp.status, CheckStatus::Fail);
    assert!(
        cmp.differences
            .iter()
            .any(|d| d.what == "entry.position" && d.block == Some(0)),
        "{:?}",
        cmp.differences
    );
    let md = summary_markdown(&r);
    assert!(md.contains("entry.position (block 0, expected #"), "{md}");
}

#[test]
fn a_failing_lower_tier_blocks_the_higher_ones() {
    let root = temp_root("lower");
    let mut s = satin("satin-e");
    // An internal expectation that does not hold.
    s["measurements"][0]["expected"] = json!(30);
    write_spec(&root, &s);
    run(&config(&root)).expect("first run");
    supply_reference(&root, "satin-e", None, "Owner");
    let r = run(&config(&root)).expect("run");
    let s = spec_result(&r, "satin-e");
    assert_eq!(s.status, Status::Fail);
    assert_eq!(s.tier, None);
    assert_eq!(s.tiers.get(&Tier::Reference), Some(&CheckStatus::Pass));
    assert_eq!(
        s.tiers.get(&Tier::InternalRegression),
        Some(&CheckStatus::Fail)
    );
}

#[test]
fn sew_out_records_reach_the_machine_tier_only_for_the_current_export() {
    let root = temp_root("machine");
    let mut s = satin("satin-f");
    s.as_object_mut().expect("object").remove("references");
    for m in s["measurements"].as_array_mut().expect("measurements") {
        m.as_object_mut().expect("m").remove("reference");
    }
    s["measurements"].as_array_mut().expect("m").pop();
    s["machine"] = json!({"record": "stitches/satin-f.json", "make": "Sew it."});
    write_spec(&root, &s);
    let r = run(&config(&root)).expect("run");
    let res = spec_result(&r, "satin-f");
    assert_eq!(res.tier, Some(Tier::InternalRegression));
    assert_eq!(res.tiers.get(&Tier::Machine), Some(&CheckStatus::Pending));

    let sha = res.exports[0].sha256.clone();
    let record = |sha: &str, result: &str| {
        json!({
            "schema": "openstitch-compat-machine/1", "spec": "satin-f", "result": result,
            "machine": "test machine", "file": "satin.pes", "fileSha256": sha,
            "fabric": "cotton", "stabilizer": "tear-away", "sewnBy": "Owner",
            "date": "2026-10-19", "observations": "clean edges"
        })
    };
    let path = root.join("machine/stitches/satin-f.json");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    let put = |v: Value| std::fs::write(&path, v.to_string()).expect("record");

    put(record(&sha, "pass"));
    let r = run(&config(&root)).expect("run");
    let res = spec_result(&r, "satin-f");
    assert_eq!(res.tier, Some(Tier::Machine), "{}", summary_markdown(&r));

    put(record(&sha, "fail"));
    assert_eq!(
        spec_result(&run(&config(&root)).expect("run"), "satin-f").status,
        Status::Fail
    );

    put(record(&"0".repeat(64), "pass"));
    let r = run(&config(&root)).expect("run");
    let res = spec_result(&r, "satin-f");
    assert_eq!(res.tiers.get(&Tier::Machine), Some(&CheckStatus::Pending));
    assert_eq!(res.tier, Some(Tier::InternalRegression));

    let mut bad = record(&sha, "pass");
    bad["fabric"] = json!("");
    put(bad);
    assert_eq!(
        spec_result(&run(&config(&root)).expect("run"), "satin-f").status,
        Status::Error
    );
}

#[test]
fn invalid_specs_are_listed_not_run() {
    let root = temp_root("invalid");
    let mut cases = Vec::new();
    let mut unknown = satin("unknown-field");
    unknown["colour"] = json!("red");
    cases.push(unknown);
    let mut schema = satin("bad-schema");
    schema["schema"] = json!("openstitch-compat/0");
    cases.push(schema);
    let mut inputs = satin("escaping-input");
    inputs["inputs"] = json!([{"from": "../secret.pes", "name": "x.pes"}]);
    cases.push(inputs);
    let mut export = satin("escaping-export");
    export["exports"] = json!([{"path": "../satin.pes"}]);
    cases.push(export);
    let mut needs_file = satin("step-source");
    needs_file["script"]
        .as_array_mut()
        .expect("script")
        .push(json!({"id": "pts", "command": "design.info"}));
    needs_file["measurements"] = json!([{"id": "v", "measure": {"kind": "pesVersion"},
        "source": {"step": "pts"}, "expected": "0060", "evidence": "x"}]);
    cases.push(needs_file);
    let mut tier = satin("claimed-tier");
    tier["measurements"][0]["tier"] = json!("reference");
    cases.push(tier);
    let mut nothing = satin("no-expectation");
    nothing["measurements"] = json!([{"id": "n", "measure": {"kind": "stitchCount"},
        "source": {"export": "satin.pes"}, "evidence": "x"}]);
    cases.push(nothing);
    for c in &cases {
        write_spec(&root, c);
    }
    // The file name must match the id.
    std::fs::write(
        root.join("specs/stitches/renamed.json"),
        satin("other-id").to_string(),
    )
    .expect("write");
    let r = run(&config(&root)).expect("run");
    assert!(r.specs.is_empty());
    assert_eq!(r.invalid.len(), cases.len() + 1);
    assert!(!r.ok());
    let problems = |path: &str| {
        r.invalid
            .iter()
            .find(|i| i.path == path)
            .map(|i| i.problems.join("; "))
            .unwrap_or_default()
    };
    assert!(problems("stitches/unknown-field.json").contains("unknown field"));
    assert!(problems("stitches/escaping-input.json").contains("without . or .."));
    assert!(problems("stitches/escaping-export.json").contains("export"));
    assert!(problems("stitches/step-source.json").contains("needs an export source"));
    assert!(problems("stitches/claimed-tier.json").contains("come from files"));
    assert!(problems("stitches/no-expectation.json").contains("needs `expected`"));
    assert!(problems("stitches/renamed.json").contains("renamed.json"));
}

#[test]
fn script_failures_and_paths_outside_the_work_directory_are_errors() {
    let root = temp_root("script");
    let mut unknown = satin("unknown-command");
    unknown["script"]
        .as_array_mut()
        .expect("script")
        .push(json!({"command": "no.such"}));
    write_spec(&root, &unknown);
    let mut outside = satin("outside");
    let target = root.join("escaped.pes");
    outside["script"]
        .as_array_mut()
        .expect("script")
        .push(json!({"command": "design.export", "params": {"path": target.to_string_lossy()}}));
    write_spec(&root, &outside);
    let r = run(&config(&root)).expect("run");
    let u = spec_result(&r, "unknown-command");
    assert_eq!(u.status, Status::Error);
    assert!(
        u.error
            .as_deref()
            .unwrap_or("")
            .contains("script step 3 (no.such)")
    );
    let o = spec_result(&r, "outside");
    assert_eq!(o.status, Status::Error, "{:?}", o.error);
    assert!(!target.exists(), "the sandbox let the script write outside");
}

#[test]
fn reports_and_the_status_table_are_deterministic() {
    let root = temp_root("determinism");
    write_spec(&root, &satin("satin-g"));
    write_spec(&root, &satin("satin-h"));
    let a = run(&config(&root)).expect("run");
    let b = run(&config(&root)).expect("run");
    assert_eq!(a.to_json(), b.to_json());
    assert_eq!(status_markdown(&a), status_markdown(&b));
    assert!(
        !a.to_json().contains(&*root.to_string_lossy()),
        "no absolute paths"
    );
    let table = status_markdown(&a);
    assert!(
        table.contains("| Region sew types | Satin… | internal-regression; reference pending |")
    );
    assert!(
        table.contains("| 1 | internal-regression; reference pending | `satin-g`, `satin-h` |")
    );
}

#[test]
fn feature_matrix_rows_must_exist_and_the_status_check_catches_stale_tables() {
    let root = temp_root("matrix");
    write_spec(&root, &satin("satin-i"));
    let matrix = root.join("matrix.md");
    std::fs::write(
        &matrix,
        "| Area | Feature | Status |\n|---|---|---|\n| Region sew types | Satin: a zigzag | Implemented |\n",
    )
    .expect("matrix");
    let r = run_with_matrix(&config(&root), Some(&matrix)).expect("run");
    assert!(r.unknown_rows.is_empty(), "{:?}", r.unknown_rows);
    std::fs::write(
        &matrix,
        "| Area | Feature | Status |\n|---|---|---|\n| Files | PES | Implemented |\n",
    )
    .expect("matrix");
    let r = run_with_matrix(&config(&root), Some(&matrix)).expect("run");
    assert_eq!(r.unknown_rows, ["Region sew types / Satin: no rows match"]);
    assert!(!r.ok());

    // The CLI writes the table, then finds it current; a hand edit makes it stale.
    std::fs::write(
        &matrix,
        "| Area | Feature | Status |\n|---|---|---|\n| Region sew types | Satin | Implemented |\n",
    )
    .expect("matrix");
    let status = root.join("STATUS.md");
    let args = |flag: &str| {
        vec![
            "--specs".to_string(),
            root.join("specs").to_string_lossy().into_owned(),
            "--reference".into(),
            root.join("reference").to_string_lossy().into_owned(),
            "--machine".into(),
            root.join("machine").to_string_lossy().into_owned(),
            "--work".into(),
            root.join("work").to_string_lossy().into_owned(),
            "--report".into(),
            root.join("out/report.json").to_string_lossy().into_owned(),
            "--summary".into(),
            root.join("out/summary.md").to_string_lossy().into_owned(),
            "--matrix".into(),
            matrix.to_string_lossy().into_owned(),
            flag.into(),
            status.to_string_lossy().into_owned(),
        ]
    };
    let cli = |flag: &str| {
        let a = args(flag);
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        let o = openstitch_compat::cli::parse(&refs).expect("parse");
        openstitch_compat::cli::run(&o).expect("cli run")
    };
    assert!(cli("--status"));
    assert!(cli("--check-status"));
    let edited = std::fs::read_to_string(&status)
        .expect("status")
        .replace("internal-regression", "reference");
    std::fs::write(&status, edited).expect("edit");
    assert!(!cli("--check-status"));
    let report: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("out/report.json")).expect("report"),
    )
    .expect("json");
    assert_eq!(report["schema"], "openstitch-compat-report/1");
    assert!(openstitch_compat::cli::parse(&["--bogus"]).is_err());
    assert!(openstitch_compat::cli::parse(&["--specs"]).is_err());
}
