//! Stitch Design Factory commands: sewing.* on the project, files, hostile input.

use openstitch_design::sewing::{SewingKind, SewingStitch};
use openstitch_engine::{Session, project};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-sewing-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn doc(kind: SewingKind) -> Value {
    serde_json::from_str(&SewingStitch::new(kind).to_json(false)).expect("json")
}

#[test]
fn sewing_stitches_are_defined_listed_exported_and_undone() {
    let root = temp_dir();
    let mut s = Session::sandboxed(&root);
    for kind in SewingKind::ALL {
        let r = run(&mut s, "sewing.define", json!({ "stitch": doc(kind) }));
        assert_eq!(r["replaced"], false);
    }
    let list = run(&mut s, "sewing.list", json!({}));
    assert_eq!(list.as_array().map(Vec::len), Some(4));
    assert!(list[1]["needles"].as_u64().is_some_and(|n| n > 10));
    // Replace keeps one entry per id.
    let mut wider = doc(SewingKind::Satin);
    wider["profile"] = json!([[0, -3, 3], [6, -3, 3]]);
    assert_eq!(
        run(&mut s, "sewing.define", json!({ "stitch": wider }))["replaced"],
        true
    );
    assert_eq!(s.design().sewing.len(), 4);
    let got = run(&mut s, "sewing.get", json!({"id": "my-satin"}));
    assert_eq!(got["profile"], json!([[0.0, -3.0, 3.0], [6.0, -3.0, 3.0]]));
    assert!(got["needles"].as_array().is_some_and(|n| n.len() == 25));

    for (file, format) in [
        ("satin.json", "json"),
        ("satin.svg", "svg"),
        ("satin.pmv", "pmv"),
    ] {
        let r = run(
            &mut s,
            "sewing.export",
            json!({"id": "my-satin", "path": file}),
        );
        assert_eq!(r["format"], format);
    }
    let svg = std::fs::read_to_string(root.join("satin.svg")).expect("svg");
    assert!(svg.starts_with("<svg"));
    assert!(
        std::fs::read(root.join("satin.pmv"))
            .expect("pmv")
            .starts_with(b"#PMV0001")
    );
    // The exported JSON (with needles) imports again.
    run(&mut s, "sewing.delete", json!({"id": "my-satin"}));
    assert_eq!(s.design().sewing.len(), 3);
    run(&mut s, "sewing.import", json!({"path": "satin.json"}));
    assert_eq!(s.design().sewing.len(), 4);
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().sewing.len(), 3);

    // Kept in the project; designs without them serialize as before.
    let bytes = project::encode(s.design()).expect("encode");
    assert_eq!(&project::decode(&bytes).expect("decode"), s.design());
    let empty = Session::new(temp_dir());
    let text = String::from_utf8(project::encode(empty.design()).expect("encode")).expect("utf8");
    assert!(!text.contains("sewing"));
}

#[test]
fn hostile_sewing_input_fails_cleanly() {
    let root = temp_dir();
    let mut s = Session::sandboxed(&root);
    run(
        &mut s,
        "sewing.define",
        json!({ "stitch": doc(SewingKind::Decorative) }),
    );
    let before = s.design().clone();
    let bad_doc = |f: &dyn Fn(&mut Value)| {
        let mut d = doc(SewingKind::Decorative);
        f(&mut d);
        json!({ "stitch": d })
    };
    let cases = vec![
        ("sewing.define", json!({})),
        ("sewing.define", json!({"stitch": 3})),
        ("sewing.define", json!({"stitch": "{"})),
        ("sewing.define", bad_doc(&|d| d["width"] = json!(12))),
        (
            "sewing.define",
            bad_doc(&|d| d["points"] = json!([[0, 0], [20, 0]])),
        ),
        (
            "sewing.define",
            bad_doc(&|d| d["points"] = json!([[0, 0], [1, 9]])),
        ),
        ("sewing.define", bad_doc(&|d| d["id"] = json!("../x"))),
        (
            "sewing.define",
            bad_doc(&|d| d["kind"] = json!("embroidery")),
        ),
        ("sewing.delete", json!({"id": "nope"})),
        ("sewing.get", json!({"id": "nope"})),
        ("sewing.export", json!({"id": "nope", "path": "x.json"})),
        (
            "sewing.export",
            json!({"id": "my-decorative", "path": "x.pmp"}),
        ),
        (
            "sewing.export",
            json!({"id": "my-decorative", "path": "x.psf"}),
        ),
        (
            "sewing.export",
            json!({"id": "my-decorative", "path": "x.bin", "format": "dst"}),
        ),
        (
            "sewing.export",
            json!({"id": "my-decorative", "path": "../escape.json"}),
        ),
        (
            "sewing.export",
            json!({"id": "my-decorative", "path": "x.svg", "repeats": 50}),
        ),
        ("sewing.import", json!({"path": "missing.json"})),
        ("sewing.swap", json!({"id": "nope"})),
        ("sewing.swap", json!({})),
        ("sewing.duplicate", json!({"id": "nope"})),
        ("sewing.duplicate", json!({"id": 7})),
    ];
    for (id, params) in cases {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
    std::fs::write(root.join("bin.json"), [0xff, 0x00]).expect("write");
    std::fs::write(root.join("big.json"), vec![b' '; (1 << 20) + 1]).expect("write");
    for f in ["bin.json", "big.json"] {
        assert!(
            s.execute("sewing.import", Some(&json!({"path": f})))
                .is_err()
        );
    }
    // PMV holds at most 100 needle points.
    let mut long = doc(SewingKind::Satin);
    long["id"] = json!("long");
    long["profile"] = json!([[0, -3, 3], [60, -3, 3]]);
    run(&mut s, "sewing.define", json!({ "stitch": long }));
    assert!(
        s.execute(
            "sewing.export",
            Some(&json!({"id": "long", "path": "long.pmv"}))
        )
        .is_err()
    );
    // At most 256 per project.
    for i in 0..254 {
        let mut d = doc(SewingKind::Decorative);
        d["id"] = json!(format!("s{i}"));
        run(&mut s, "sewing.define", json!({ "stitch": d }));
    }
    let mut d = doc(SewingKind::Decorative);
    d["id"] = json!("one-too-many");
    assert!(
        s.execute("sewing.define", Some(&json!({ "stitch": d })))
            .is_err()
    );
    // Damaged project: duplicate ids.
    let mut v: Value =
        serde_json::from_slice(&project::encode(s.design()).expect("encode")).expect("json");
    let first = v["design"]["sewing"][0].clone();
    v["design"]["sewing"] = json!([first.clone(), first]);
    assert!(project::decode(&serde_json::to_vec(&v).expect("bytes")).is_err());
}

#[test]
fn swap_and_duplicate_edit_project_patterns_with_one_undo_each() {
    let mut s = Session::sandboxed(temp_dir());
    for kind in SewingKind::ALL {
        run(&mut s, "sewing.define", json!({ "stitch": doc(kind) }));
    }
    let r = run(&mut s, "sewing.swap", json!({"id": "my-decorative"}));
    assert_eq!(r["swapped"], true);
    let got = run(&mut s, "sewing.get", json!({"id": "my-decorative"}));
    assert_eq!(got["swapped"], true);
    assert_eq!(
        got["needles"][0],
        json!([8.0, 0.0]),
        "starts at the old end"
    );

    let r = run(&mut s, "sewing.duplicate", json!({"id": "my-satin"}));
    assert_eq!(r["lengthMM"], 16.0);
    let before = s.design().clone();
    assert!(
        s.execute(
            "sewing.duplicate",
            Some(&json!({"id": "my-decorativesatin"}))
        )
        .is_err(),
        "one outline holds no copy"
    );
    assert_eq!(s.design(), &before);
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().sewing[1], SewingStitch::new(SewingKind::Satin));
    run(&mut s, "history.undo", json!({}));
    assert!(!s.design().sewing[0].swapped);
}
