//! Centre point, reference point and curved sewing direction commands.

use openstitch_design::{Compiler, RegionType, v2};
use openstitch_engine::{Session, project};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn square(s: &mut Session, kind: &str) {
    run(
        s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 40, "fill": "Red"}),
    );
    run(s, "object.setRegion", json!({"id": 1, "type": kind}));
}

fn stitches(s: &Session) -> usize {
    Compiler::default().compile(s.design()).stitches.len()
}

#[test]
fn centre_point_moves_with_the_shape_and_saves() {
    let mut s = Session::new(std::env::temp_dir());
    square(&mut s, "radial");
    let plain = Compiler::default().compile(s.design());
    run(
        &mut s,
        "object.setCenterPoint",
        json!({"id": 1, "point": [15, 15]}),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.center, Some(v2(150.0, 150.0)));
    assert_ne!(Compiler::default().compile(s.design()), plain);
    run(&mut s, "object.move", json!({"id": 1, "dx": 5}));
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.center, Some(v2(200.0, 150.0)));
    let back = project::decode(&project::encode(s.design()).expect("encode")).expect("decode");
    assert_eq!(&back, s.design());
    run(
        &mut s,
        "object.setCenterPoint",
        json!({"id": 1, "point": null}),
    );
    assert!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .center
            .is_none()
    );
    run(&mut s, "history.undo", json!({}));
    assert!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .center
            .is_some()
    );
}

#[test]
fn reference_point_and_direction_curve() {
    let mut s = Session::new(std::env::temp_dir());
    square(&mut s, "programmableFill");
    run(
        &mut s,
        "object.setReferencePoint",
        json!({"id": 1, "point": [13, 17]}),
    );
    assert!(stitches(&s) > 100);
    let before = Compiler::default().compile(s.design());
    run(
        &mut s,
        "object.setDirectionCurve",
        json!({"id": 1, "points": [[0, 30], [60, 30]], "curves": [[[20, 0], [40, 0]]]}),
    );
    assert_ne!(Compiler::default().compile(s.design()), before);
    let curve = s.design().objects[0]
        .region
        .as_ref()
        .and_then(|r| r.direction_curve.clone())
        .expect("curve");
    assert!(curve.has_curves() && !curve.closed);
    run(&mut s, "object.rotate", json!({"id": 1, "degrees": 90}));
    let rotated = s.design().objects[0]
        .region
        .as_ref()
        .and_then(|r| r.direction_curve.clone())
        .expect("curve");
    assert_ne!(rotated, curve);
    // Switching to a type without the setting keeps it stored but unused.
    run(&mut s, "object.setRegion", json!({"id": 1, "type": "fill"}));
    assert!(stitches(&s) > 100);
    run(
        &mut s,
        "object.setDirectionCurve",
        json!({"id": 1, "points": null}),
    );
    assert!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .direction_curve
            .is_none()
    );
    assert_eq!(
        s.design().objects[0].region.as_ref().expect("r").kind,
        RegionType::Fill
    );
}

#[test]
fn bad_direction_parameters_fail_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    square(&mut s, "fill");
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[0, 0], [5, 5]], "closed": false}),
    );
    let before = s.design().clone();
    for (cmd, params) in [
        ("object.setCenterPoint", json!({"id": 1, "point": [1, 1]})),
        ("object.setCenterPoint", json!({"id": 1})),
        ("object.setCenterPoint", json!({"id": 2, "point": null})),
        ("object.setCenterPoint", json!({"id": 99, "point": [1, 1]})),
        (
            "object.setReferencePoint",
            json!({"id": 1, "point": [1, 1]}),
        ),
        (
            "object.setReferencePoint",
            json!({"id": 1, "point": [1e9, 1]}),
        ),
        (
            "object.setDirectionCurve",
            json!({"id": 1, "points": [[0, 0]]}),
        ),
        (
            "object.setDirectionCurve",
            json!({"id": 1, "points": [[0, 0], [0.001, 0]]}),
        ),
        (
            "object.setDirectionCurve",
            json!({"id": 1, "points": [[0, 0], [50, 0], [20, 5]]}),
        ),
        (
            "object.setDirectionCurve",
            json!({"id": 1, "points": [[0, 0], [50, 0]], "curves": []}),
        ),
        (
            "object.setDirectionCurve",
            json!({"id": 1, "points": [[0, 0], [50, 0]], "curves": [[1, 2]]}),
        ),
        (
            "object.setDirectionCurve",
            json!({"id": 1, "points": "0,0"}),
        ),
        (
            "object.setDirectionCurve",
            json!({"id": 2, "points": [[0, 0], [50, 0]]}),
        ),
    ] {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd}");
    }
    // Satin has no curved direction.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin"}),
    );
    assert!(
        s.execute(
            "object.setDirectionCurve",
            Some(&json!({"id": 1, "points": [[0, 0], [50, 0]]}))
        )
        .is_err()
    );
}
