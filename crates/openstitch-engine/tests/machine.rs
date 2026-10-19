use openstitch_design::Compiler;
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn session_with_square() -> Session {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 40, "y": 40, "width": 20, "height": 20}),
    );
    s
}

#[test]
fn hoop_list_names_sources_and_current_settings() {
    let mut s = Session::new(std::env::temp_dir());
    let r = run(&mut s, "machine.hoops", json!({}));
    let brother = r["brother"].as_array().expect("brother hoops");
    assert!(brother.len() >= 12);
    assert!(
        brother
            .iter()
            .all(|h| !h["source"].as_str().unwrap_or("").is_empty())
    );
    assert!(brother.iter().any(|h| h["id"] == "130x180"));
    assert_eq!(
        brother
            .iter()
            .filter(|h| h["multiPosition"] == true)
            .count(),
        2
    );
    assert_eq!(r["current"]["hoop"], "custom");
    assert_eq!(r["current"]["machine"], "single");
    assert_eq!(r["current"]["sewingArea"], "page");
}

#[test]
fn selecting_a_hoop_resizes_the_page_and_keeps_objects_centred() {
    let mut s = session_with_square();
    let r = run(&mut s, "machine.set", json!({"hoop": "130x180"}));
    assert_eq!(r["pageWidthMM"], 130.0);
    assert_eq!(r["pageHeightMM"], 180.0);
    let b = s.design().objects[0].bounds().expect("bounds");
    assert_eq!((b.center().x, b.center().y), (650.0, 900.0));

    let r = run(&mut s, "machine.set", json!({"rotate90": true}));
    assert_eq!(
        (r["pageWidthMM"].clone(), r["pageHeightMM"].clone()),
        (json!(180.0), json!(130.0))
    );

    // One undo step per change.
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().page.width, 1300.0);
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().page.width, 1000.0);
    assert!(s.design().machine.is_default());
}

#[test]
fn custom_size_and_user_hoops() {
    let mut s = session_with_square();
    run(&mut s, "machine.set", json!({"width": 150, "height": 120}));
    assert_eq!(
        (s.design().page.width, s.design().page.height),
        (1500.0, 1200.0)
    );
    assert_eq!(s.design().machine.hoop, None);

    let a = run(
        &mut s,
        "machine.addUserHoop",
        json!({"width": 110, "height": 90, "comment": "cap"}),
    );
    let b = run(
        &mut s,
        "machine.addUserHoop",
        json!({"width": 60, "height": 40}),
    );
    assert_eq!(
        (a["hoop"].clone(), b["hoop"].clone()),
        (json!("user-1"), json!("user-2"))
    );
    run(
        &mut s,
        "machine.moveUserHoop",
        json!({"hoop": "user-2", "direction": "up"}),
    );
    let list = run(&mut s, "machine.hoops", json!({}));
    assert_eq!(list["user"][0]["id"], "user-2");
    assert_eq!(list["user"][1]["comment"], "cap");

    run(&mut s, "machine.set", json!({"hoop": "user-1"}));
    assert_eq!(
        (s.design().page.width, s.design().page.height),
        (1100.0, 900.0)
    );
    assert!(
        s.execute("machine.set", Some(&json!({"rotate90": true})))
            .is_err()
    );
    assert!(
        s.execute("machine.deleteUserHoop", Some(&json!({"hoop": "user-1"})))
            .is_err()
    );
    run(&mut s, "machine.deleteUserHoop", json!({"hoop": "user-2"}));
    assert_eq!(s.design().machine.user_hoops.len(), 1);

    // A rotated Brother hoop switching to a user hoop drops the rotation.
    run(
        &mut s,
        "machine.set",
        json!({"hoop": "100x180", "rotate90": true}),
    );
    run(&mut s, "machine.set", json!({"hoop": "user-1"}));
    assert!(!s.design().machine.rotate90);
}

#[test]
fn settings_round_trip_through_osd_and_default_is_omitted() {
    let mut s = session_with_square();
    let json = serde_json::to_string(s.design()).expect("json");
    assert!(
        !json.contains("machine"),
        "default settings must not change .osd files"
    );
    run(
        &mut s,
        "machine.set",
        json!({"machine": "multi", "hoop": "200x300", "minJumpTrim": 5, "sewingArea": "design", "speed": 900}),
    );
    run(
        &mut s,
        "machine.addUserHoop",
        json!({"width": 50, "height": 50}),
    );
    let json = serde_json::to_string(s.design()).expect("json");
    let back: openstitch_design::Design = serde_json::from_str(&json).expect("parse");
    assert_eq!(&back, s.design());
}

#[test]
fn jump_trim_setting_changes_trims_on_multi_needle_only() {
    let mut s = Session::new(std::env::temp_dir());
    // Two squares 4 mm apart: travel between them is trimmed at the 3 mm default.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 10, "height": 10, "outline": "#000000", "fill": "#000000"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 24, "y": 10, "width": 10, "height": 10, "outline": "#000000", "fill": "#000000"}),
    );
    let trims = |s: &Session| {
        Compiler::for_design(s.design())
            .compile(s.design())
            .count(openstitch_core::Command::Trim)
    };
    let before = trims(&s);
    assert!(
        s.execute("machine.set", Some(&json!({"minJumpTrim": 50})))
            .is_err(),
        "single-needle"
    );
    run(
        &mut s,
        "machine.set",
        json!({"machine": "multi", "minJumpTrim": 50}),
    );
    assert!(
        trims(&s) < before,
        "a 50 mm minimum leaves short jumps untrimmed"
    );
    let info = run(&mut s, "machine.hoops", json!({}));
    assert_eq!(info["current"]["minJumpTrimMM"], 50.0);
}

#[test]
fn design_info_reports_thread_length_time_and_sewing_area() {
    let mut s = session_with_square();
    let info = run(&mut s, "design.info", json!({}));
    let e = &info["estimates"];
    let needle = e["needleThreadM"].as_f64().expect("needle");
    assert!(needle > 0.5 && needle < 50.0, "{needle}");
    let bobbin = e["bobbinThreadM"].as_f64().expect("bobbin");
    assert!((bobbin - needle / 3.0).abs() < 0.02);
    let stitches = info["stitchStats"]["stitches"].as_f64().expect("stitches");
    let minutes = e["sewingMinutes"].as_f64().expect("minutes");
    assert!(
        minutes >= stitches / 800.0 - 0.1,
        "{minutes} for {stitches} stitches"
    );
    assert_eq!(info["sewingArea"]["mode"], "page");
    assert_eq!(info["sewingArea"]["startXMM"], 50.0);

    run(
        &mut s,
        "machine.set",
        json!({"speed": 400, "sewingArea": "design"}),
    );
    let slow = run(&mut s, "design.info", json!({}));
    assert!(slow["estimates"]["sewingMinutes"].as_f64() > Some(minutes));
    let area = &slow["sewingArea"];
    assert_eq!(area["mode"], "design");
    assert!(area["widthMM"].as_f64() < Some(25.0));
}

#[test]
fn measure_reports_distance_and_angle() {
    let mut s = Session::new(std::env::temp_dir());
    let r = run(
        &mut s,
        "design.measure",
        json!({"from": [0, 0], "to": [30, -40]}),
    );
    assert_eq!(r["distanceMM"], 50.0);
    assert_eq!(r["dxMM"], 30.0);
    assert_eq!(r["angleDegrees"], 53.13);
}

#[test]
fn machine_bad_parameters_fail_cleanly() {
    let mut s = session_with_square();
    run(
        &mut s,
        "machine.addUserHoop",
        json!({"width": 100, "height": 100}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("machine.set", json!({"machine": "triple"})),
        ("machine.set", json!({"hoop": "999x999"})),
        ("machine.set", json!({"hoop": "user-9"})),
        ("machine.set", json!({"hoop": 130})),
        ("machine.set", json!({"rotate90": "yes"})),
        ("machine.set", json!({"width": 5})),
        ("machine.set", json!({"width": 1e300})),
        ("machine.set", json!({"hoop": "130x180", "width": 100})),
        ("machine.set", json!({"sewingArea": "hoop"})),
        ("machine.set", json!({"speed": 5})),
        ("machine.set", json!({"speed": 900.5})),
        ("machine.set", json!({"machine": "multi", "minJumpTrim": 0})),
        (
            "machine.set",
            json!({"machine": "multi", "minJumpTrim": "5"}),
        ),
        ("machine.addUserHoop", json!({"width": 100})),
        ("machine.addUserHoop", json!({"width": 100, "height": -3})),
        (
            "machine.addUserHoop",
            json!({"width": 100, "height": 100, "comment": "x".repeat(101)}),
        ),
        ("machine.deleteUserHoop", json!({"hoop": "user-7"})),
        ("machine.deleteUserHoop", json!({"hoop": "130x180"})),
        (
            "machine.moveUserHoop",
            json!({"hoop": "user-1", "direction": "left"}),
        ),
        ("design.measure", json!({"from": [0, 0]})),
        ("design.measure", json!({"from": [0], "to": [1, 1]})),
    ];
    for (id, params) in bad {
        let r = s.execute(id, Some(&params));
        assert!(r.is_err(), "{id} {params} should fail");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
}
