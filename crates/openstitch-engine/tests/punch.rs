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
        "object.setHoleSewing",
        json!({"id": 3, "enabled": true}),
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
