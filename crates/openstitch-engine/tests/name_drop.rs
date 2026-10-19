//! Name Drop: text fitted into a fixed box, list rows applied to the design, and the batch
//! export writing one file per row.

use openstitch_design::Rect;
use openstitch_engine::{Capabilities, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-namedrop-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn bounds(s: &Session, id: u64) -> Rect {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")]
        .bounds()
        .expect("bounds")
}

/// A badge: an arched name over a fixed team line, plus a heart.
fn badge(dir: &PathBuf) -> (Session, u64, u64) {
    let mut s = Session::new(dir);
    run(
        &mut s,
        "shape.preset",
        json!({"kind": "heart", "cx": 50, "cy": 70, "width": 12, "fill": "Red"}),
    );
    let name = run(
        &mut s,
        "text.add",
        json!({"text": "Fernand", "size": 10, "x": 50, "y": 40}),
    )["id"]
        .as_u64()
        .expect("id");
    let team = run(
        &mut s,
        "text.add",
        json!({"text": "Team", "size": 6, "x": 50, "y": 55, "letterSpacing": 1}),
    )["id"]
        .as_u64()
        .expect("id");
    (s, name, team)
}

#[test]
fn replacement_text_fits_the_original_box() {
    let (mut s, name, _) = badge(&temp_dir());
    let original = bounds(&s, name);
    run(&mut s, "text.setNameDrop", json!({"id": name}));
    let on = bounds(&s, name);
    for (a, b) in [
        (on.min_x, original.min_x),
        (on.min_y, original.min_y),
        (on.max_x, original.max_x),
        (on.max_y, original.max_y),
    ] {
        assert!(
            (a - b).abs() < 1e-6,
            "turning it on keeps the text where it is"
        );
    }

    // Shorter: same height, centred.
    run(&mut s, "object.setText", json!({"id": name, "text": "Al"}));
    let short = bounds(&s, name);
    assert!(
        (short.height() - original.height()).abs() < 1.0,
        "{short:?}"
    );
    assert!((short.center().x - original.center().x).abs() < 0.5);
    // Longer: shrunk to the width, middle aligned.
    run(
        &mut s,
        "object.setText",
        json!({"id": name, "text": "Bartholomew-Smith"}),
    );
    let long = bounds(&s, name);
    assert!((long.width() - original.width()).abs() < 0.5, "{long:?}");
    assert!(long.height() < original.height());
    assert!((long.center().y - original.center().y).abs() < 0.5);
    // Left / bottom alignment.
    run(
        &mut s,
        "text.setNameDrop",
        json!({"id": name, "align": "left", "valign": "bottom"}),
    );
    let lb = bounds(&s, name);
    assert!((lb.min_x - original.min_x).abs() < 0.5 && (lb.max_y - original.max_y).abs() < 0.5);
    // Stretched to the box.
    run(
        &mut s,
        "text.setNameDrop",
        json!({"id": name, "maintainAspect": false}),
    );
    run(&mut s, "object.setText", json!({"id": name, "text": "Al"}));
    let st = bounds(&s, name);
    assert!(
        (st.width() - original.width()).abs() < 0.5
            && (st.height() - original.height()).abs() < 0.5
    );
    // The box follows the text when it is moved.
    run(
        &mut s,
        "object.move",
        json!({"id": name, "dx": 10, "dy": 5}),
    );
    run(&mut s, "object.setText", json!({"id": name, "text": "Al"}));
    assert!((bounds(&s, name).min_x - original.min_x - 100.0).abs() < 0.5);
    // Off again: natural layout.
    run(
        &mut s,
        "text.setNameDrop",
        json!({"id": name, "enabled": false}),
    );
    assert!(
        s.design().objects[1]
            .text
            .as_ref()
            .expect("t")
            .name_drop
            .is_none()
    );
}

#[test]
fn turning_name_drop_on_resets_spacing() {
    let (mut s, _, team) = badge(&temp_dir());
    run(&mut s, "text.setNameDrop", json!({"id": team}));
    let t = s.design().objects[2].text.clone().expect("text");
    assert_eq!(t.letter_spacing, 0.0);
    assert!(t.name_drop.is_some());
}

#[test]
fn rows_are_applied_and_exported_one_file_per_row() {
    let dir = temp_dir();
    let (mut s, name, team) = badge(&dir);
    run(&mut s, "text.setNameDrop", json!({"ids": [name, team]}));
    let cols = run(&mut s, "nameDrop.list", json!({}));
    assert_eq!(cols.as_array().expect("cols").len(), 2);
    assert_eq!(cols[0]["text"], "Fernand");

    std::fs::create_dir_all(dir.join("out")).expect("out");
    let before = s.design().clone();
    let rows = json!([
        ["Ann", "Red Team"],
        ["Bob", "Blue Team"],
        ["Céline", "Green"]
    ]);
    let r = run(
        &mut s,
        "nameDrop.export",
        json!({"rows": rows, "dir": "out", "name": "badge"}),
    );
    assert_eq!(r["count"], 3);
    assert_eq!(s.design(), &before, "export does not change the design");
    let mut files: Vec<String> = std::fs::read_dir(dir.join("out"))
        .expect("dir")
        .map(|e| e.expect("e").file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, ["badge001.pes", "badge002.pes", "badge003.pes"]);
    // Each file holds its own row: different stitch counts, all readable.
    let sizes: Vec<u64> = files
        .iter()
        .map(|f| std::fs::metadata(dir.join("out").join(f)).expect("m").len())
        .collect();
    assert!(sizes[0] != sizes[1] && sizes[1] != sizes[2], "{sizes:?}");
    let mut t = Session::new(dir.join("out"));
    run(&mut t, "stitches.import", json!({"path": "badge002.pes"}));

    // A list from CSV, written as DST.
    std::fs::write(
        dir.join("list.csv"),
        "\u{feff}Ann,\"Red, Team\"\r\n\"Bo \"\"B\"\"\",Blue\r\n\r\nCy,Green\n",
    )
    .expect("csv");
    let read = run(&mut s, "nameDrop.readCsv", json!({"path": "list.csv"}));
    assert_eq!(
        read["rows"],
        json!([["Ann", "Red, Team"], ["Bo \"B\"", "Blue"], ["Cy", "Green"]])
    );
    let r = run(
        &mut s,
        "nameDrop.export",
        json!({"csv": "list.csv", "name": "tag-", "format": "dst"}),
    );
    assert_eq!(r["count"], 3);
    assert!(dir.join("tag-003.dst").exists());
    run(
        &mut s,
        "nameDrop.writeCsv",
        json!({"path": "back.csv", "rows": read["rows"]}),
    );
    let again = run(&mut s, "nameDrop.readCsv", json!({"path": "back.csv"}));
    assert_eq!(again["rows"], read["rows"]);

    // OK in the list dialog: the selected row goes into the design (one undo step).
    run(&mut s, "nameDrop.apply", json!({"values": ["Zoë", "Gold"]}));
    assert_eq!(s.design().objects[1].text.as_ref().expect("t").text, "Zoë");
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design(), &before);
}

#[test]
fn a_bad_row_writes_nothing() {
    let dir = temp_dir();
    let (mut s, name, _) = badge(&dir);
    run(&mut s, "text.setNameDrop", json!({"id": name}));
    let r = s.execute(
        "nameDrop.export",
        Some(&json!({"rows": ["Ann", "", "Cy"], "name": "x"})),
    );
    assert!(r.is_err());
    let r = s.execute(
        "nameDrop.export",
        Some(&json!({"rows": ["Ann", "\u{4E2D}\u{6587}"], "name": "x"})),
    );
    assert!(r.is_err(), "a row the font cannot draw");
    assert!(!dir.join("x001.pes").exists(), "validated before writing");
}

#[test]
fn name_drop_bad_input_fails_cleanly() {
    let dir = temp_dir();
    let (mut s, name, _) = badge(&dir);
    let circle = run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 30, "ry": 30}),
    )["id"]
        .as_u64()
        .expect("id");
    let on_path = run(&mut s, "text.add", json!({"text": "Round", "path": circle}))["id"]
        .as_u64()
        .expect("id");
    let mono = run(&mut s, "monogram.add", json!({"letters": "AB"}));
    let mono_letter = mono["ids"][0].as_u64().expect("id");
    let before = s.design().clone();
    let many: Vec<String> = (0..501).map(|i| format!("n{i}")).collect();
    std::fs::write(dir.join("bad.csv"), "\"unterminated\n").expect("csv");
    std::fs::write(dir.join("bin.csv"), [0xff, 0xfe, 0x00]).expect("csv");
    let bad: Vec<(&str, Value)> = vec![
        ("text.setNameDrop", json!({"id": 1})),
        ("text.setNameDrop", json!({"id": on_path})),
        ("text.setNameDrop", json!({"id": mono_letter})),
        ("text.setNameDrop", json!({"id": name, "align": "justify"})),
        ("text.setNameDrop", json!({"id": name, "valign": "up"})),
        ("text.setNameDrop", json!({"id": name, "label": ""})),
        ("text.setNameDrop", json!({"ids": []})),
        ("nameDrop.apply", json!({"values": ["x"]})),
        ("nameDrop.export", json!({"rows": ["A"], "name": "x"})),
        ("nameDrop.readCsv", json!({"path": "bad.csv"})),
        ("nameDrop.readCsv", json!({"path": "bin.csv"})),
        ("nameDrop.readCsv", json!({"path": "missing.csv"})),
    ];
    for (id, params) in &bad {
        assert!(s.execute(id, Some(params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
    run(&mut s, "text.setNameDrop", json!({"id": name}));
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("nameDrop.apply", json!({"values": ["a", "b"]})),
        ("nameDrop.apply", json!({"values": [""]})),
        ("nameDrop.apply", json!({"values": [1]})),
        ("nameDrop.export", json!({"rows": ["A"]})),
        ("nameDrop.export", json!({"rows": [], "name": "x"})),
        ("nameDrop.export", json!({"rows": many, "name": "x"})),
        ("nameDrop.export", json!({"rows": ["A"], "name": "../x"})),
        ("nameDrop.export", json!({"rows": ["A"], "name": ".hidden"})),
        (
            "nameDrop.export",
            json!({"rows": ["A"], "name": "x", "format": "svg"}),
        ),
        (
            "nameDrop.export",
            json!({"rows": ["A"], "name": "x", "format": "phc"}),
        ),
        (
            "nameDrop.export",
            json!({"rows": ["A"], "name": "x", "dir": "missing"}),
        ),
        (
            "nameDrop.export",
            json!({"rows": ["A"], "csv": "x.csv", "name": "x"}),
        ),
        (
            "nameDrop.export",
            json!({"rows": [["A", "B"]], "name": "x"}),
        ),
    ];
    for (id, params) in &bad {
        assert!(s.execute(id, Some(params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
    // Sandbox and capabilities.
    let mut t = Session::sandboxed(&dir);
    t.execute("project.open", None).ok();
    assert!(
        t.execute("nameDrop.readCsv", Some(&json!({"path": "/etc/passwd"})))
            .is_err()
    );
    t.set_capabilities(Capabilities::FULL.without_files());
    assert!(
        t.execute("nameDrop.readCsv", Some(&json!({"path": "bad.csv"})))
            .is_err()
    );
}
