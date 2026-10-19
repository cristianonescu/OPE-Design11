//! Stitch pattern editing commands.

use openstitch_core::Command;
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Two filled squares (Red, Blue) converted into stitch patterns 1 and 2.
fn session() -> Session {
    let mut s = Session::new(std::env::temp_dir());
    run(&mut s, "design.new", json!({"width": 100, "height": 100}));
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
    s
}

fn points(s: &mut Session, id: u64) -> Vec<Value> {
    run(s, "stitches.points", json!({"id": id}))["points"]
        .as_array()
        .expect("points")
        .clone()
}

fn stitch_count(s: &Session) -> usize {
    s.design()
        .objects
        .iter()
        .flat_map(|o| o.stitch_block.iter().flat_map(|b| b.stitches.iter()))
        .filter(|st| st.command == Command::Stitch)
        .count()
}

#[test]
fn points_move_insert_delete() {
    let mut s = session();
    let pts = points(&mut s, 1);
    let k = pts
        .iter()
        .position(|p| p["command"] == "stitch")
        .expect("a stitch")
        + 1;
    let (x, y) = (
        pts[k]["x"].as_f64().expect("x"),
        pts[k]["y"].as_f64().expect("y"),
    );
    run(
        &mut s,
        "stitches.movePoints",
        json!({"id": 1, "indexes": [k], "dx": 1.5, "dy": -2}),
    );
    let moved = &points(&mut s, 1)[k];
    assert!((moved["x"].as_f64().expect("x") - x - 1.5).abs() < 1e-9);
    assert!((moved["y"].as_f64().expect("y") - y + 2.0).abs() < 1e-9);
    run(
        &mut s,
        "stitches.setPoint",
        json!({"id": 1, "index": k, "x": 12, "y": 13}),
    );
    assert_eq!(points(&mut s, 1)[k]["x"], 12.0);

    let before = stitch_count(&s);
    let r = run(&mut s, "stitches.insertPoint", json!({"id": 1, "index": k}));
    assert_eq!(r["index"], json!(k));
    assert_eq!(stitch_count(&s), before + 1);
    run(
        &mut s,
        "stitches.deletePoints",
        json!({"id": 1, "indexes": [k, k + 1]}),
    );
    assert_eq!(stitch_count(&s), before - 1);
    // Each edit is one undo step.
    run(&mut s, "history.undo", json!({}));
    assert_eq!(stitch_count(&s), before + 1);
}

#[test]
fn split_connect_and_divide() {
    let mut s = session();
    let pts = points(&mut s, 1);
    let mid = pts.len() / 2;
    let mid = (mid..pts.len())
        .find(|&i| pts[i]["command"] == "stitch")
        .expect("stitch");
    let before = stitch_count(&s);
    let r = run(&mut s, "stitches.splitAt", json!({"id": 1, "index": mid}));
    let new = r["ids"][1].as_u64().expect("id");
    assert_eq!(s.design().objects.len(), 3);
    assert_eq!(
        s.design().objects[1].id,
        new,
        "the second part follows the first"
    );
    assert_eq!(
        stitch_count(&s),
        before + 1,
        "the split point is in both parts"
    );

    run(&mut s, "stitches.connect", json!({"ids": [new, 1]}));
    assert_eq!(s.design().objects.len(), 2);
    assert_eq!(stitch_count(&s), before + 1);

    // Connect the two colours, then divide them again.
    let r = run(&mut s, "stitches.connect", json!({"ids": [1, 2]}));
    assert_eq!(r["id"], json!(1));
    let b = s.design().objects[0].stitch_block.clone().expect("block");
    assert_eq!(
        b.threads.len(),
        1,
        "the joined pattern takes the first one's colour"
    );
    let stats = run(&mut s, "design.generate", json!({}));
    assert_eq!(stats["colors"].as_array().expect("colors").len(), 1);
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));

    // A two-colour pattern divides into two.
    let mut d = s.design().clone();
    let a = d.objects[0].stitch_block.clone().expect("a");
    let b = d.objects[1].stitch_block.clone().expect("b");
    let mut stitches = a.stitches.clone();
    stitches.push(openstitch_core::Stitch::new(0.0, 0.0, Command::ColorChange));
    stitches.extend(b.stitches);
    d.objects[0].stitch_block = Some(openstitch_design::StitchBlock {
        stitches,
        threads: [a.threads, b.threads].concat(),
    });
    d.objects.truncate(1);
    let path = std::env::temp_dir().join(format!("stitch-edit-{}.osd", std::process::id()));
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
    let r = run(&mut s, "stitches.divideByColor", json!({"id": 1}));
    assert_eq!(r["ids"].as_array().expect("ids").len(), 2);
    let names: Vec<_> = s
        .design()
        .objects
        .iter()
        .map(|o| o.stitch_block.as_ref().expect("b").threads[0].name.clone())
        .collect();
    assert_eq!(names, vec![Some("Red".into()), Some("Blue".into())]);
}

#[test]
fn split_off_an_area() {
    let mut s = session();
    let before = stitch_count(&s);
    // The left half of the red square (x 10-20 mm).
    let r = run(
        &mut s,
        "stitches.splitArea",
        json!({"id": 1, "points": [[0, 0], [20, 0], [20, 40], [0, 40]]}),
    );
    let new = r["ids"][0].as_u64().expect("id");
    assert_eq!(
        s.design().objects.last().map(|o| o.id),
        Some(new),
        "sewn last"
    );
    let inside = &s.design().objects[2];
    assert!(
        inside
            .stitch_block
            .as_ref()
            .expect("block")
            .stitches
            .iter()
            .all(|st| st.x <= 200.0)
    );
    assert_eq!(stitch_count(&s), before);
}

#[test]
fn bad_stitch_edit_parameters_fail_cleanly() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 50, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    let n = points(&mut s, 1).len();
    let trim = points(&mut s, 1)
        .iter()
        .position(|p| p["command"] == "trim");
    let mut bad: Vec<(&str, Value)> = vec![
        ("stitches.points", json!({"id": 3})),
        ("stitches.points", json!({"id": 99})),
        (
            "stitches.movePoints",
            json!({"id": 1, "indexes": [], "dx": 1}),
        ),
        (
            "stitches.movePoints",
            json!({"id": 1, "indexes": [n], "dx": 1}),
        ),
        ("stitches.movePoints", json!({"id": 1, "indexes": [-1]})),
        ("stitches.movePoints", json!({"id": 1, "indexes": "0"})),
        (
            "stitches.movePoints",
            json!({"id": 1, "indexes": [1], "dx": 1e9}),
        ),
        ("stitches.movePoints", json!({"id": 3, "indexes": [0]})),
        ("stitches.setPoint", json!({"id": 1, "index": 1, "x": 1})),
        (
            "stitches.setPoint",
            json!({"id": 1, "index": 1, "x": "a", "y": 1}),
        ),
        ("stitches.insertPoint", json!({"id": 1, "index": 0})),
        ("stitches.insertPoint", json!({"id": 1, "index": 2, "x": 1})),
        (
            "stitches.insertPoint",
            json!({"id": 1, "index": 2, "jump": "yes"}),
        ),
        (
            "stitches.deletePoints",
            json!({"id": 1, "indexes": (0..n).collect::<Vec<_>>()}),
        ),
        ("stitches.splitAt", json!({"id": 1, "index": 0})),
        ("stitches.splitAt", json!({"id": 1, "index": n - 1})),
        ("stitches.connect", json!({"ids": [1]})),
        ("stitches.connect", json!({"ids": [1, 1]})),
        ("stitches.connect", json!({"ids": [1, 3]})),
        ("stitches.connect", json!({"ids": [1, 2, 3]})),
        ("stitches.divideByColor", json!({"id": 3})),
        (
            "stitches.splitArea",
            json!({"id": 1, "points": [[0, 0], [1, 1]]}),
        ),
        (
            "stitches.splitArea",
            json!({"id": 1, "points": [[90, 90], [95, 90], [95, 95]]}),
        ),
        (
            "stitches.splitArea",
            json!({"id": 1, "points": [[-100, -100], [500, -100], [500, 500], [-100, 500]]}),
        ),
    ];
    if let Some(t) = trim {
        bad.push((
            "stitches.movePoints",
            json!({"id": 1, "indexes": [t], "dx": 1}),
        ));
    }
    for (id, params) in bad {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before, "{id} {params} changed the design");
    }
}
