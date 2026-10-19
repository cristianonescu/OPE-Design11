use openstitch_engine::{Capabilities, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-sections-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// A 230 × 160 mm custom page with a filled rectangle across the middle.
fn split_session() -> (Session, PathBuf) {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "design.new",
        json!({"name": "Banner", "width": 230, "height": 160}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 20, "y": 60, "width": 190, "height": 40}),
    );
    (s, dir)
}

#[test]
fn split_sections_list_and_export_one_file_each() {
    let (mut s, dir) = split_session();
    let r = run(&mut s, "sections.set", json!({"hoop": "130x180"}));
    let secs = r["sections"].as_array().expect("sections");
    assert_eq!(secs.len(), 2);
    assert_eq!(secs[0]["label"], "Aa");
    assert_eq!(secs[0]["area"]["width"], 115.0);
    assert_eq!(r["settings"]["sectionWidthMM"], 120.0);
    assert_eq!(
        (
            secs[0]["alignmentAfter"].clone(),
            secs[1]["alignmentBefore"].clone()
        ),
        (json!(1), json!(1))
    );

    let out = run(&mut s, "sections.export", json!({"path": "banner.pes"}));
    let files = out["files"].as_array().expect("files");
    assert_eq!(files.len(), 2);
    for (f, label) in files.iter().zip(["Aa", "Ab"]) {
        let path = dir.join(format!("banner_{label}.pes"));
        assert_eq!(f["path"], path.display().to_string());
        let pattern = openstitch_formats::FileType::Pes
            .read(&std::fs::read(&path).expect("file"))
            .expect("readable");
        // Each half stays inside its 115 mm section.
        assert!(pattern.bounds().width() <= 1150.0 + 1.0, "{label}");
    }

    // Moving the split: 100 mm left, 130 mm right is too wide for a 120 mm section.
    assert!(
        s.execute("sections.set", Some(&json!({"splitsX": [100]})))
            .is_err()
    );
    let r = run(&mut s, "sections.set", json!({"splitsX": [112]}));
    assert_eq!(r["sections"][0]["area"]["width"], 112.0);
    // A new section hoop resets the splits.
    let r = run(&mut s, "sections.set", json!({"hoop": "160x260"}));
    assert_eq!(r["settings"]["splitsX"], json!([]));
    // Undo steps back through each change.
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().machine.sections.is_none());
}

#[test]
fn multi_position_and_jumbo_plans() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(&mut s, "machine.set", json!({"hoop": "130x300-multi"}));
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 50, "height": 50}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 240, "width": 50, "height": 50}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 70, "y": 20, "width": 40, "height": 40}),
    );
    let r = run(&mut s, "sections.list", json!({}));
    let labels: Vec<_> = r["sections"]
        .as_array()
        .expect("s")
        .iter()
        .map(|x| x["label"].clone())
        .collect();
    assert_eq!(labels, [json!("1a"), json!("2c"), json!("3a")]);
    let r = run(&mut s, "sections.set", json!({"optimizeHoopChange": true}));
    assert_eq!(r["sections"][0]["objects"], json!([1, 3]));
    let out = run(&mut s, "sections.export", json!({"path": "multi.dst"}));
    assert_eq!(out["files"].as_array().map(Vec::len), Some(2));
    assert!(dir.join("multi_1a.dst").exists() && dir.join("multi_2c.dst").exists());
    // A pattern taller than one area is an error and blocks export.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 250}),
    );
    let r = run(&mut s, "sections.list", json!({}));
    assert_eq!(r["errors"].as_array().map(Vec::len), Some(1));
    assert!(
        s.execute("sections.export", Some(&json!({"path": "multi.dst"})))
            .is_err()
    );

    let r = run(&mut s, "machine.set", json!({"hoop": "360x360"}));
    assert_eq!(r["pageWidthMM"], 350.0);
    let r = run(&mut s, "sections.set", json!({"jumboOverlap": 10}));
    let secs = r["sections"].as_array().expect("s");
    assert_eq!(secs.len(), 2);
    assert_eq!(secs[1]["rotate180"], true);
    assert_eq!(secs[0]["area"]["height"], 180.0);
}

#[test]
fn sections_bad_parameters_fail_cleanly() {
    let (mut s, _) = split_session();
    run(&mut s, "sections.set", json!({"hoop": "130x180"}));
    let before = s.design().clone();
    for params in [
        json!({"hoop": "999x999"}),
        json!({"hoop": "130x300-multi"}),
        json!({"hoop": "360x360"}),
        json!({"hoop": 130}),
        json!({"smaller": "yes"}),
        json!({"splitsX": "100"}),
        json!({"splitsX": ["a"]}),
        json!({"splitsX": [200, 100]}),
        json!({"splitsX": [0]}),
        json!({"splitsX": [500]}),
        json!({"splitsY": vec![5; 40]}),
        json!({"jumboOverlap": -1}),
        json!({"jumboOverlap": 80}),
        json!({"optimizeHoopChange": 1}),
    ] {
        assert!(
            s.execute("sections.set", Some(&params)).is_err(),
            "{params}"
        );
        assert_eq!(s.design(), &before, "{params}");
    }
    for params in [
        json!({}),
        json!({"path": "x.txt"}),
        json!({"path": "x.phc"}),
        json!({"path": ".pes"}),
        json!({"path": "x.pes", "alignment": "no"}),
    ] {
        assert!(
            s.execute("sections.export", Some(&params)).is_err(),
            "{params}"
        );
    }
    // Sections need a custom page.
    run(&mut s, "machine.set", json!({"hoop": "130x180"}));
    assert!(
        s.execute("sections.set", Some(&json!({"hoop": "100x100"})))
            .is_err()
    );
    // Split settings need a section hoop.
    run(&mut s, "sections.set", json!({"hoop": "none"}));
    assert!(
        s.execute("sections.set", Some(&json!({"smaller": true})))
            .is_err()
    );
}

#[test]
fn read_only_clients_can_list_but_not_export() {
    let (mut s, dir) = split_session();
    run(&mut s, "sections.set", json!({"hoop": "130x180"}));
    s.set_capabilities(Capabilities::FULL.read_only());
    run(&mut s, "sections.list", json!({}));
    assert!(
        s.execute("sections.export", Some(&json!({"path": "a.pes"})))
            .is_err()
    );
    assert!(
        s.execute("sections.set", Some(&json!({"smaller": true})))
            .is_err()
    );
    assert!(!dir.join("a_Aa.pes").exists());
}
