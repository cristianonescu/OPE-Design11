//! Manual Punch shapes and several direction lines.

use openstitch_core::Command;
use openstitch_design::{Compiler, RegionType, v2};
use openstitch_engine::{Session, project};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn bend() -> Value {
    json!([
        [[10, 10], [10, 20]],
        [[40, 10], [35, 20]],
        [[50, 35], [40, 35]]
    ])
}

#[test]
fn column_shape_sews_along_its_lines() {
    let mut s = Session::new(std::env::temp_dir());
    let r = run(
        &mut s,
        "shape.column",
        json!({"lines": bend(), "fill": "Red", "outline": "Black"}),
    );
    assert_eq!(r["id"], json!(1));
    let o = &s.design().objects[0];
    assert_eq!(o.contours[0].points.len(), 6);
    let region = o.region.clone().expect("region");
    assert_eq!(region.kind, RegionType::Satin);
    assert_eq!(region.direction_lines.len(), 3);
    let p = Compiler::default().compile(s.design());
    assert!(p.count(Command::Stitch) > 50);
    // The last satin stitch is the last direction line (before the outline).
    let pts: Vec<_> = p
        .stitches
        .iter()
        .filter(|x| x.command == Command::Stitch)
        .map(|x| v2(x.x, x.y))
        .collect();
    assert!(pts.contains(&v2(500.0, 350.0)) && pts.contains(&v2(400.0, 350.0)));
    // Fill along the lines; transforms and .osd keep the lines.
    run(&mut s, "history.undo", json!({}));
    run(
        &mut s,
        "shape.column",
        json!({"lines": bend(), "type": "fill"}),
    );
    run(&mut s, "object.move", json!({"id": 1, "dx": 10}));
    let lines = &s.design().objects[0]
        .region
        .as_ref()
        .expect("r")
        .direction_lines;
    assert_eq!(lines[0], [v2(200.0, 100.0), v2(200.0, 200.0)]);
    let back = project::decode(&project::encode(s.design()).expect("encode")).expect("decode");
    assert_eq!(&back, s.design());
}

#[test]
fn direction_lines_on_an_existing_shape() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 40, "height": 10}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin"}),
    );
    let plain = Compiler::default().compile(s.design());
    run(
        &mut s,
        "object.setDirectionLines",
        json!({"id": 1, "lines": [[[0, 0], [0, 10]], [[20, 0], [15, 10]], [[40, 0], [40, 10]]]}),
    );
    assert_ne!(Compiler::default().compile(s.design()), plain);
    run(
        &mut s,
        "object.setDirectionLines",
        json!({"id": 1, "lines": null}),
    );
    assert_eq!(Compiler::default().compile(s.design()), plain);
}

#[test]
fn bad_punch_parameters_fail_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 40, "height": 10}),
    );
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 70, "cy": 70, "rx": 5}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 2, "type": "radial"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 50, "width": 10, "height": 10}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 3, "y": 54, "width": 4, "height": 2}),
    );
    run(
        &mut s,
        "object.setHoleSewing",
        json!({"ids": [3, 4], "enabled": true}),
    );
    let good = json!([[[0, 0], [0, 10]], [[20, 0], [20, 10]]]);
    let before = s.design().clone();
    for (cmd, params) in [
        ("shape.column", json!({})),
        ("shape.column", json!({"lines": [[[0, 0], [0, 10]]]})),
        (
            "shape.column",
            json!({"lines": [[[0, 0], [0, 10]], [[0, 5]]]}),
        ),
        (
            "shape.column",
            json!({"lines": [[[0, 0], [0, 10]], [[5, 5], [5, 5]]]}),
        ),
        (
            "shape.column",
            json!({"lines": [[[0, 0], [0, 10]], [[-5, 5], [5, 5]]]}),
        ),
        (
            "shape.column",
            json!({"lines": [[[0, 0], [0, 10]], [[20, 10], [20, 0]]]}),
        ),
        ("shape.column", json!({"lines": "x"})),
        ("shape.column", json!({"lines": good, "type": "radial"})),
        ("shape.column", json!({"lines": good, "fill": "#zz"})),
        ("object.setDirectionLines", json!({"id": 1})),
        ("object.setDirectionLines", json!({"id": 2, "lines": good})),
        ("object.setDirectionLines", json!({"id": 3, "lines": good})),
        ("object.setDirectionLines", json!({"id": 99, "lines": good})),
    ] {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd}");
    }
}

#[test]
fn skeleton_satin_sews_across_its_centre_line() {
    let mut s = Session::new(std::env::temp_dir());
    let r = run(
        &mut s,
        "shape.skeletonSatin",
        json!({"points": [[10, 10], [40, 10], [40, 40]], "widths": [2, 4, 1], "density": 4, "fill": "Red"}),
    );
    assert_eq!(r["id"], json!(1));
    let o = &s.design().objects[0];
    let region = o.region.clone().expect("region");
    assert_eq!(region.kind, RegionType::Satin);
    let sk = region.skeleton.clone().expect("skeleton");
    assert_eq!(sk.widths, vec![2.0, 4.0, 1.0]);
    assert_eq!(sk.points[1], v2(400.0, 100.0));
    assert!(o.contours[0].points.len() > 10);
    let p = Compiler::default().compile(s.design());
    // 60 mm of centre line at 4 lines per mm.
    assert!(
        p.count(Command::Stitch) >= 240,
        "{}",
        p.count(Command::Stitch)
    );
    // Transforms and .osd keep the skeleton; scaling scales the widths.
    run(&mut s, "object.move", json!({"id": 1, "dx": 10}));
    let moved = s.design().objects[0]
        .region
        .clone()
        .expect("r")
        .skeleton
        .expect("s");
    assert_eq!(moved.points[0], v2(200.0, 100.0));
    let back = project::decode(&project::encode(s.design()).expect("encode")).expect("decode");
    assert_eq!(&back, s.design());
    // Files written before skeletons existed still load.
    let mut old =
        serde_json::from_slice::<Value>(&project::encode(s.design()).expect("e")).expect("j");
    old["design"]["objects"][0]["region"]
        .as_object_mut()
        .expect("region")
        .remove("skeleton");
    let plain = project::decode(&serde_json::to_vec(&old).expect("v")).expect("old file");
    assert!(
        plain.objects[0]
            .region
            .as_ref()
            .expect("r")
            .skeleton
            .is_none()
    );
}

#[test]
fn skeleton_on_an_existing_satin_shape() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 40, "height": 10}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin"}),
    );
    let plain = Compiler::default().compile(s.design());
    let outline = s.design().objects[0].contours.clone();
    run(
        &mut s,
        "object.setSkeleton",
        json!({"id": 1, "points": [[0, 5], [20, 2], [40, 5]], "width": 4}),
    );
    assert_ne!(Compiler::default().compile(s.design()), plain);
    assert_eq!(s.design().objects[0].contours, outline, "outline kept");
    run(
        &mut s,
        "object.setSkeleton",
        json!({"id": 1, "points": [[0, 5], [40, 5]], "widths": [1, 3], "reshape": true}),
    );
    assert_ne!(s.design().objects[0].contours, outline);
    run(
        &mut s,
        "object.setSkeleton",
        json!({"id": 1, "points": null}),
    );
    assert!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .skeleton
            .is_none()
    );
}

#[test]
fn bad_skeleton_parameters_fail_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 40, "height": 10}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 50, "width": 10, "height": 10}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 3, "y": 54, "width": 4, "height": 2}),
    );
    run(
        &mut s,
        "object.setHoleSewing",
        json!({"ids": [2, 3], "enabled": true}),
    );
    let good = json!([[0, 0], [20, 0]]);
    let before = s.design().clone();
    for (cmd, params) in [
        ("shape.skeletonSatin", json!({})),
        ("shape.skeletonSatin", json!({"points": [[0, 0]]})),
        (
            "shape.skeletonSatin",
            json!({"points": [[0, 0], [0.01, 0]]}),
        ),
        ("shape.skeletonSatin", json!({"points": [[0, 0], [5, "x"]]})),
        (
            "shape.skeletonSatin",
            json!({"points": good, "widths": [3]}),
        ),
        (
            "shape.skeletonSatin",
            json!({"points": good, "widths": [3, 31]}),
        ),
        (
            "shape.skeletonSatin",
            json!({"points": good, "widths": [3, -1]}),
        ),
        (
            "shape.skeletonSatin",
            json!({"points": good, "widths": [0, 0]}),
        ),
        (
            "shape.skeletonSatin",
            json!({"points": good, "widths": [3, "w"]}),
        ),
        ("shape.skeletonSatin", json!({"points": good, "width": 0})),
        (
            "shape.skeletonSatin",
            json!({"points": good, "width": 3, "widths": [3, 3]}),
        ),
        ("shape.skeletonSatin", json!({"points": good, "density": 0})),
        (
            "shape.skeletonSatin",
            json!({"points": good, "fill": "#zz"}),
        ),
        ("object.setSkeleton", json!({"id": 1, "points": good})),
        ("object.setSkeleton", json!({"id": 2, "points": good})),
        ("object.setSkeleton", json!({"id": 99, "points": good})),
        ("object.setSkeleton", json!({"id": 1})),
    ] {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd}");
    }
}

#[test]
fn skeleton_derived_from_a_shape() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 60, "height": 6}),
    );
    let before = s.design().clone();
    let r = run(&mut s, "object.deriveSkeleton", json!({"id": 1}));
    let o = &s.design().objects[0];
    let region = o.region.as_ref().expect("region");
    assert_eq!(region.kind, RegionType::Satin);
    let sk = region.skeleton.as_ref().expect("skeleton");
    assert_eq!(r["points"].as_array().map(Vec::len), Some(sk.points.len()));
    assert!(sk.length() > 480.0, "{}", sk.length());
    assert!(
        sk.points.iter().all(|p| (p.y - 30.0).abs() < 3.0),
        "{:?}",
        sk
    );
    assert_eq!(o.contours, before.objects[0].contours, "outline kept");
    let p = Compiler::default().compile(s.design());
    assert!(p.count(Command::Stitch) > 100);
    // One undo step.
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design(), &before);
    run(
        &mut s,
        "object.deriveSkeleton",
        json!({"id": 1, "reshape": true}),
    );
    assert_ne!(s.design().objects[0].contours, before.objects[0].contours);
}

#[test]
fn deriving_a_skeleton_fails_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 40, "height": 10}),
    );
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[0, 50], [30, 50], [30, 60]], "closed": false}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "A", "x": 50, "y": 50, "size": 10}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 4, "width": 4, "height": 2}),
    );
    run(
        &mut s,
        "object.setHoleSewing",
        json!({"ids": [1, 4], "enabled": true}),
    );
    let before = s.design().clone();
    for params in [
        json!({}),
        json!({"id": 99}),
        json!({"id": 1}),
        json!({"id": 2}),
        json!({"id": 3}),
        json!({"id": 1, "reshape": "yes"}),
    ] {
        assert!(
            s.execute("object.deriveSkeleton", Some(&params)).is_err(),
            "{params}"
        );
        assert_eq!(s.design(), &before, "{params}");
    }
}
