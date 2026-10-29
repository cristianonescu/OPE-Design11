//! User settings: saved sewing-attribute sets and user thread charts, persisted outside the
//! design.

use openstitch_engine::{Capabilities, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-user-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn attribute_sets_persist_and_apply() {
    let dir = temp_dir();
    let file = dir.join("settings/user.json");
    let mut s = Session::new(&dir);
    s.with_user_settings(&file)
        .expect("missing file starts empty");
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10, "fill": "Red", "outline": "Black"}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "density": 5, "angle": 30}),
    );
    run(&mut s, "object.setLine", json!({"id": 1, "type": "triple"}));
    let design = s.design().clone();
    run(
        &mut s,
        "attributes.save",
        json!({"name": "Denim", "from": 1}),
    );
    assert_eq!(s.design(), &design, "settings do not touch the design");
    assert!(file.exists());
    // Another session (the next app start) sees the set.
    let mut t = Session::new(&dir);
    t.with_user_settings(&file).expect("load");
    let list = run(&mut t, "attributes.list", json!({}));
    assert_eq!(list[0]["name"], json!("Denim"));
    run(
        &mut t,
        "shape.ellipse",
        json!({"cx": 30, "cy": 30, "rx": 5, "fill": "Blue"}),
    );
    run(
        &mut t,
        "attributes.apply",
        json!({"name": "Denim", "id": 1}),
    );
    let o = &t.design().objects[0];
    let r = o.region.clone().expect("region");
    assert_eq!(r.density, 5.0);
    assert_eq!(r.angle, 30.0);
    assert_eq!(r.thread.hex(), "#0a55a3", "colours stay");
    assert!(o.line.is_some());
    run(&mut t, "history.undo", json!({}));
    run(
        &mut t,
        "attributes.apply",
        json!({"name": "Denim", "id": 1, "includeColors": true}),
    );
    assert_eq!(
        t.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .thread
            .hex(),
        "#ed171f"
    );
    // Same name overwrites; delete removes.
    run(
        &mut s,
        "attributes.save",
        json!({"name": "Denim", "from": 1}),
    );
    assert_eq!(s.user_settings().attribute_sets.len(), 1);
    run(&mut s, "attributes.delete", json!({"name": "Denim"}));
    let mut u = Session::new(&dir);
    u.with_user_settings(&file).expect("load");
    assert!(u.user_settings().attribute_sets.is_empty());
    // Settings changes are not undo steps.
    assert!(s.execute("history.undo", None).is_ok());
}

#[test]
fn thread_charts_edit_import_and_remap() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(&mut s, "threads.createChart", json!({"name": "Mine"}));
    let r = run(
        &mut s,
        "threads.addColor",
        json!({"chart": "Mine", "color": "#ff0000", "code": "100", "brand": "Acme", "description": "Poppy"}),
    );
    assert_eq!(r["index"], json!(0));
    run(
        &mut s,
        "threads.addColor",
        json!({"chart": "Mine", "color": "#0000ff"}),
    );
    run(
        &mut s,
        "threads.editColor",
        json!({"chart": "Mine", "index": 1, "description": "Navy"}),
    );
    run(
        &mut s,
        "threads.moveColor",
        json!({"chart": "Mine", "index": 1, "to": 0}),
    );
    std::fs::write(
        dir.join("c.csv"),
        "\u{feff}# code,brand,description,R,G,B\n200,Acme,Leaf,0,200,0\n\n201,Acme,Sun,250,250,0\n",
    )
    .expect("write");
    let r = run(
        &mut s,
        "threads.importCsv",
        json!({"chart": "Mine", "path": "c.csv"}),
    );
    assert_eq!(r["added"], json!(2));
    let charts = run(&mut s, "threads.charts", json!({}));
    let threads = charts[0]["threads"].as_array().expect("threads");
    assert_eq!(threads.len(), 4);
    assert_eq!(threads[0]["description"], json!("Navy"));
    assert_eq!(threads[1]["code"], json!("100"));
    run(
        &mut s,
        "threads.removeColor",
        json!({"chart": "Mine", "index": 3}),
    );
    run(
        &mut s,
        "threads.renameChart",
        json!({"chart": "Mine", "name": "Palette"}),
    );
    // Remap the design to the chart.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10, "fill": "#e01020", "outline": "#102090"}),
    );
    run(&mut s, "threads.remap", json!({"chart": "Palette"}));
    let o = &s.design().objects[0];
    assert_eq!(o.region.as_ref().expect("r").thread.hex(), "#ff0000");
    assert_eq!(
        o.region.as_ref().expect("r").thread.name.as_deref(),
        Some("Poppy")
    );
    assert_eq!(o.line.as_ref().expect("l").thread.hex(), "#0000ff");
    run(&mut s, "history.undo", json!({}));
    assert_eq!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .thread
            .hex(),
        "#e01020"
    );
    run(&mut s, "threads.deleteChart", json!({"chart": "Palette"}));
    assert!(s.user_settings().thread_charts.is_empty());
}

#[test]
fn read_only_clients_cannot_change_settings() {
    let mut s = Session::new(temp_dir());
    s.set_capabilities(Capabilities::FULL.read_only());
    assert!(
        s.execute("threads.createChart", Some(&json!({"name": "X"})))
            .is_err()
    );
    assert!(s.execute("threads.charts", None).is_ok());
    let mut s = Session::new(temp_dir());
    run(&mut s, "threads.createChart", json!({"name": "X"}));
    s.set_capabilities(Capabilities::FULL.without_files());
    assert!(
        s.execute(
            "threads.importCsv",
            Some(&json!({"chart": "X", "path": "c.csv"}))
        )
        .is_err()
    );
}

#[test]
fn bad_user_settings_inputs_fail_cleanly() {
    let dir = temp_dir();
    std::fs::write(dir.join("bad.csv"), "1,a,b,1,2\n").expect("write");
    std::fs::write(dir.join("big.csv"), vec![b'1'; 5 << 20]).expect("write");
    std::fs::write(dir.join("bin.csv"), [0xffu8, 0xfe, 0, 1, 2]).expect("write");
    let file = dir.join("user.json");
    std::fs::write(&file, "{not json").expect("write");
    let mut s = Session::new(&dir);
    assert!(s.with_user_settings(&file).is_err());
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    run(&mut s, "threads.createChart", json!({"name": "C"}));
    let before = (s.design().clone(), s.user_settings().clone());
    for (cmd, params) in [
        ("attributes.save", json!({"name": "", "from": 1})),
        (
            "attributes.save",
            json!({"name": "x".repeat(101), "from": 1}),
        ),
        ("attributes.save", json!({"name": "a\nb", "from": 1})),
        ("attributes.save", json!({"name": "A", "from": 99})),
        ("attributes.apply", json!({"name": "nope", "id": 1})),
        ("attributes.delete", json!({"name": "nope"})),
        ("threads.createChart", json!({"name": "C"})),
        ("threads.createChart", json!({"name": 3})),
        ("threads.renameChart", json!({"chart": "nope", "name": "D"})),
        ("threads.deleteChart", json!({"chart": "nope"})),
        ("threads.addColor", json!({"chart": "C"})),
        ("threads.addColor", json!({"chart": "C", "color": "red"})),
        (
            "threads.addColor",
            json!({"chart": "C", "color": "#ff0000", "code": "12a"}),
        ),
        (
            "threads.addColor",
            json!({"chart": "nope", "color": "#ff0000"}),
        ),
        ("threads.editColor", json!({"chart": "C", "index": 0})),
        ("threads.removeColor", json!({"chart": "C", "index": 0})),
        (
            "threads.moveColor",
            json!({"chart": "C", "index": 0, "to": 1}),
        ),
        (
            "threads.importCsv",
            json!({"chart": "C", "path": "bad.csv"}),
        ),
        (
            "threads.importCsv",
            json!({"chart": "C", "path": "big.csv"}),
        ),
        (
            "threads.importCsv",
            json!({"chart": "C", "path": "bin.csv"}),
        ),
        (
            "threads.importCsv",
            json!({"chart": "C", "path": "missing.csv"}),
        ),
        ("threads.remap", json!({"chart": "C"})),
        ("threads.remap", json!({"chart": "nope"})),
    ] {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(
            (s.design().clone(), s.user_settings().clone()),
            before,
            "{cmd} changed something"
        );
    }
}

/// OpenStitch's thread-chart CSV (issue #136; PE-DESIGN 11.50 has no CSV for its charts, so
/// the schema is OpenStitch's own): the built-in charts are listed, any chart exports as
/// `code,brand,description,R,G,B` with a header row and quoting, and an export imports back.
#[test]
fn thread_chart_csv_exports_and_imports_back() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    let builtin = run(&mut s, "threads.builtinCharts", json!({}));
    let ids: Vec<&str> = builtin
        .as_array()
        .expect("charts")
        .iter()
        .map(|c| c["id"].as_str().expect("id"))
        .collect();
    assert_eq!(
        ids,
        [
            "brother",
            "janomeJef",
            "janomeSew",
            "husqvarnaHus",
            "husqvarnaShv"
        ]
    );
    let brother = builtin[0]["threads"].as_array().expect("threads");
    assert_eq!(brother.len(), 64);
    assert_eq!(brother[0]["index"], json!(1));
    assert_eq!(brother[4]["color"], json!("#ed171f"));
    let basic = brother.iter().filter(|t| t["basic"] == json!(true)).count();
    assert_eq!(basic, 61);

    // A built-in chart exports and imports back into a user chart unchanged.
    let r = run(
        &mut s,
        "threads.exportCsv",
        json!({"builtin": "brother", "path": "brother.csv"}),
    );
    assert_eq!(r["colors"], json!(64));
    let text = std::fs::read_to_string(dir.join("brother.csv")).expect("csv");
    assert!(text.starts_with("# OpenStitch thread chart: Brother (PEC / PES chart)\r\ncode,brand,description,R,G,B\r\n007,EMBROIDERY,Prussian Blue,14,31,124\r\n"));
    // Brother entries 1–61 carry PE-DESIGN's BROTHER EMBROIDERY codes; 62–64 keep their number.
    assert_eq!(brother[4]["code"], json!("800"));
    assert_eq!(brother[4]["brand"], json!("EMBROIDERY"));
    assert_eq!(brother[61]["code"], json!("62"));
    assert_eq!(brother[61]["brand"], json!("Brother"));
    run(&mut s, "threads.createChart", json!({"name": "Copy"}));
    let r = run(
        &mut s,
        "threads.importCsv",
        json!({"chart": "Copy", "path": "brother.csv"}),
    );
    assert_eq!(r["added"], json!(64));
    let charts = run(&mut s, "threads.charts", json!({}));
    let copied: Vec<&Value> = charts[0]["threads"]
        .as_array()
        .expect("threads")
        .iter()
        .collect();
    for (a, b) in copied.iter().zip(brother) {
        for k in ["color", "code", "brand", "description"] {
            assert_eq!(a[k], b[k], "{k}");
        }
    }

    // A user chart with commas and quotes in its strings round-trips.
    run(&mut s, "threads.createChart", json!({"name": "Odd"}));
    run(
        &mut s,
        "threads.addColor",
        json!({"chart": "Odd", "color": "#123456", "code": "7", "brand": "Acme, Inc.", "description": "Say \"hi\""}),
    );
    run(
        &mut s,
        "threads.exportCsv",
        json!({"chart": "Odd", "path": "odd.csv"}),
    );
    run(&mut s, "threads.createChart", json!({"name": "Odd2"}));
    run(
        &mut s,
        "threads.importCsv",
        json!({"chart": "Odd2", "path": "odd.csv"}),
    );
    let charts = run(&mut s, "threads.charts", json!({}));
    assert_eq!(charts[1]["threads"], charts[2]["threads"]);

    for bad in [
        json!({"path": "x.csv"}),
        json!({"chart": "Odd", "builtin": "brother", "path": "x.csv"}),
        json!({"builtin": "madeira", "path": "x.csv"}),
        json!({"chart": "Nope", "path": "x.csv"}),
    ] {
        assert!(s.execute("threads.exportCsv", Some(&bad)).is_err(), "{bad}");
    }
}
