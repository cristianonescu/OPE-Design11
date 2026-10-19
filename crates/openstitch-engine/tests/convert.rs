//! Conversions between stitch, text and shape patterns.

use openstitch_design::{ObjectKind, RegionType};
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn session() -> Session {
    let mut s = Session::new(std::env::temp_dir());
    run(&mut s, "design.new", json!({"width": 100, "height": 100}));
    s
}

fn bounds(s: &mut Session, id: u64) -> (f64, f64, f64, f64) {
    let list = run(s, "object.list", json!({}));
    let o = list
        .as_array()
        .expect("list")
        .iter()
        .find(|o| o["id"] == json!(id))
        .expect("object");
    let b = &o["bounds"];
    let f = |k: &str| b[k].as_f64().expect("number");
    (f("x"), f("y"), f("width"), f("height"))
}

#[test]
fn shapes_and_text_become_stitch_patterns() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 30, "height": 20, "fill": "Red"}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "Hi", "size": 10, "x": 50, "y": 70}),
    );
    let before = run(&mut s, "design.generate", json!({}));
    let r = run(&mut s, "object.toStitches", json!({"ids": [1, 2]}));
    assert_eq!(r["ids"], json!([1, 2]));
    for o in &s.design().objects {
        assert_eq!(o.kind, ObjectKind::Stitches);
        assert!(o.region.is_none() && o.contours.is_empty() && o.text.is_none());
    }
    let after = run(&mut s, "design.generate", json!({}));
    assert_eq!(
        after["stitches"], before["stitches"],
        "the same needle points are sewn"
    );
    assert_eq!(after["colors"], before["colors"]);
    run(&mut s, "history.undo", json!({}));
    assert_eq!(
        s.design().objects[0].kind,
        ObjectKind::Shape,
        "one undo step"
    );
}

#[test]
fn groups_convert_together_and_empty_objects_go() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 10, "height": 10}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 30, "y": 10, "width": 10, "height": 10}),
    );
    run(&mut s, "object.group", json!({"ids": [1, 2]}));
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 50, "y": 10, "width": 10, "height": 10}),
    );
    run(&mut s, "object.setRegion", json!({"id": 4, "type": "none"}));
    let r = run(&mut s, "object.toStitches", json!({"ids": [1, 4]}));
    assert_eq!(r["ids"], json!([1, 2]));
    assert_eq!(r["deleted"], json!([4]), "sews nothing, so it is deleted");
    assert_eq!(s.design().objects.len(), 2);
}

#[test]
fn stitch_patterns_trace_back_into_shapes() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 20, "fill": "Red"}),
    );
    run(&mut s, "object.setRegion", json!({"id": 1, "angle": 30}));
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 60, "y": 10, "width": 4, "height": 30, "fill": "Blue"}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 2, "type": "satin", "angle": 0}),
    );
    run(&mut s, "object.toStitches", json!({"ids": [1, 2]}));

    // Parts: one shape per colour, grouped; the stitch patterns are gone.
    let r = run(&mut s, "object.toShapes", json!({"ids": [1]}));
    let ids: Vec<u64> = serde_json::from_value(r["ids"].clone()).expect("ids");
    assert_eq!(ids.len(), 1);
    let o = &s.design().objects[0];
    assert_eq!(o.kind, ObjectKind::Shape);
    let region = o.region.as_ref().expect("fill");
    assert_eq!(region.kind, RegionType::Fill);
    assert_eq!(region.thread.name.as_deref(), Some("Red"));
    assert!(
        (region.angle - 30.0).abs() <= 2.0,
        "direction from the stitches: {}",
        region.angle
    );
    let (x, y, w, h) = bounds(&mut s, ids[0]);
    assert!((x - 10.0).abs() < 1.0 && (y - 10.0).abs() < 1.0, "{x} {y}");
    assert!((w - 40.0).abs() < 1.5 && (h - 20.0).abs() < 1.5, "{w} {h}");

    // Blocks: the narrow satin bar comes back as satin with its stitch direction.
    let r = run(
        &mut s,
        "object.toShapes",
        json!({"id": 2, "mode": "blocks"}),
    );
    let bar = s
        .design()
        .index_of(r["ids"][0].as_u64().expect("id"))
        .expect("bar");
    let region = s.design().objects[bar].region.as_ref().expect("region");
    assert_eq!(region.kind, RegionType::Satin);
    assert_eq!(region.angle, 0.0);
    // Sewing order is kept: red outline first, blue bar second.
    assert_eq!(s.design().objects.len(), 2);
    assert_eq!(bar, 1);
}

#[test]
fn whole_and_parts_on_a_two_colour_pattern() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20, "fill": "Red"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 50, "y": 10, "width": 20, "height": 20, "fill": "Blue"}),
    );
    run(&mut s, "object.toStitches", json!({"ids": [1, 2]}));
    // Merge both into one two-colour stitch pattern the way an imported file would be.
    let mut blocks = s
        .design()
        .objects
        .iter()
        .map(|o| o.stitch_block.clone().expect("block"));
    let (a, b) = (blocks.next().expect("a"), blocks.next().expect("b"));
    let mut stitches = a.stitches.clone();
    stitches.push(openstitch_core::Stitch::new(
        0.0,
        0.0,
        openstitch_core::Command::ColorChange,
    ));
    stitches.extend(b.stitches.clone());
    let merged = openstitch_design::StitchBlock {
        stitches,
        threads: [a.threads, b.threads].concat(),
    };
    let mut d = s.design().clone();
    d.objects.truncate(1);
    d.objects[0].stitch_block = Some(merged);
    let path = std::env::temp_dir().join(format!("convert-{}.osd", std::process::id()));
    std::fs::write(
        &path,
        openstitch_engine::project::encode(&d).expect("encode"),
    )
    .expect("write");
    run(
        &mut s,
        "project.open",
        json!({"path": path.display().to_string()}),
    );
    std::fs::remove_file(&path).ok();

    let parts = run(&mut s, "object.toShapes", json!({"id": 1}));
    assert_eq!(parts["ids"].as_array().expect("ids").len(), 2);
    let objects = &s.design().objects;
    assert!(
        objects[0].group.is_some() && objects[0].group == objects[1].group,
        "grouped"
    );
    assert_eq!(
        objects[1]
            .region
            .as_ref()
            .expect("r")
            .thread
            .name
            .as_deref(),
        Some("Blue")
    );
    run(&mut s, "history.undo", json!({}));
    let whole = run(&mut s, "object.toShapes", json!({"id": 1, "mode": "whole"}));
    assert_eq!(whole["ids"].as_array().expect("ids").len(), 1);
    let o = &s.design().objects[0];
    assert_eq!(o.contours.len(), 2, "both squares in one outline object");
    assert_eq!(
        o.region.as_ref().expect("r").thread.name.as_deref(),
        Some("Red"),
        "first colour"
    );
}

#[test]
fn region_becomes_running_line() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20, "fill": "Red"}),
    );
    let r = run(&mut s, "object.regionToLine", json!({"id": 1}));
    assert_eq!(r["ids"], json!([1]));
    let o = &s.design().objects[0];
    assert!(o.region.is_none());
    let line = o.line.as_ref().expect("line");
    assert_eq!(line.kind, openstitch_design::LineType::Running);
    assert_eq!(line.thread.name.as_deref(), Some("Red"));
    // Only the outline is sewn now: far fewer stitches than the fill.
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64().expect("n") < 60, "{stats}");
}

#[test]
fn bad_conversion_parameters_fail_cleanly() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20}),
    );
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[0, 0], [10, 0], [10, 10]], "closed": false, "outline": "Black"}),
    );
    let before = s.design().clone();
    for (id, params) in [
        ("object.toStitches", json!({})),
        ("object.toStitches", json!({"ids": []})),
        ("object.toStitches", json!({"id": 99})),
        ("object.toStitches", json!({"ids": ["1"]})),
        ("object.toShapes", json!({"id": 1})),
        ("object.toShapes", json!({"id": 1, "mode": "sparkle"})),
        ("object.toShapes", json!({"id": 1, "sensitivity": "ultra"})),
        ("object.toShapes", json!({"id": 1, "mode": 3})),
        ("object.regionToLine", json!({"id": 2})),
        ("object.regionToLine", json!({"id": -1})),
    ] {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before, "{id} {params} changed the design");
    }
    // A stitch pattern made only of jumps has nothing to trace.
    run(&mut s, "object.toStitches", json!({"id": 1}));
    let mut d = s.design().clone();
    d.objects[0]
        .stitch_block
        .as_mut()
        .expect("block")
        .stitches
        .retain(|st| st.command != openstitch_core::Command::Stitch);
    let path = std::env::temp_dir().join(format!("convert-jumps-{}.osd", std::process::id()));
    std::fs::write(
        &path,
        openstitch_engine::project::encode(&d).expect("encode"),
    )
    .expect("write");
    run(
        &mut s,
        "project.open",
        json!({"path": path.display().to_string()}),
    );
    std::fs::remove_file(&path).ok();
    let before = s.design().clone();
    assert!(
        s.execute("object.toShapes", Some(&json!({"id": 1})))
            .is_err()
    );
    assert_eq!(s.design(), &before);
}
