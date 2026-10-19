//! Floral pattern and decorative frames.

use openstitch_design::Compiler;
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn floral_pattern_inside_a_shape() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.preset",
        json!({"kind": "heart", "cx": 50, "cy": 50, "width": 60, "fill": "Red"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 5, "height": 5}),
    );
    let r = run(
        &mut s,
        "shape.floral",
        json!({"id": 1, "density": 4, "pattern": 20, "color": "Black"}),
    );
    let id = r["id"].as_u64().expect("id");
    let d = s.design();
    assert_eq!(d.objects[1].id, id, "sewn right after its shape");
    let f = &d.objects[1];
    assert_eq!(f.contours.len(), 4);
    assert!(f.region.is_none() && f.line.is_some());
    let heart = d.objects[0].bounds().expect("b");
    let b = f.bounds().expect("b");
    assert!(b.min_x >= heart.min_x - 0.5 && b.max_x <= heart.max_x + 0.5);
    assert!(Compiler::default().compile(d).stitches.len() > 300);
    // Text works too; undo removes the pattern in one step.
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().objects.len(), 2);
}

#[test]
fn decorative_frames_around_objects_or_centred() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 30, "y": 40, "width": 40, "height": 20}),
    );
    for kind in ["scallop", "wave", "zigzag", "scallopOval", "corners"] {
        let r = run(
            &mut s,
            "shape.decorative",
            json!({"kind": kind, "id": 1, "keepAspect": false, "margin": 2}),
        );
        let id = r["id"].as_u64().expect("id");
        let d = s.design();
        let b = d.objects[d.index_of(id).expect("o")].bounds().expect("b");
        assert!(
            b.min_x <= 280.0 + 1.0 && b.max_x >= 720.0 - 1.0,
            "{kind} {b:?}"
        );
        assert!(b.height() < 400.0, "{kind} follows the 2:1 box: {b:?}");
    }
    let r = run(
        &mut s,
        "shape.decorative",
        json!({"kind": "scallopOval", "id": 1}),
    );
    let d = s.design();
    let b = d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")]
        .bounds()
        .expect("b");
    assert!(
        (b.width() - b.height()).abs() < 50.0,
        "square with keepAspect: {b:?}"
    );
    let r = run(&mut s, "shape.decorative", json!({"kind": "wave"}));
    let d = s.design();
    let b = d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")]
        .bounds()
        .expect("b");
    assert!(b.center().distance(d.page.center()) < 1.0);
}

#[test]
fn bad_ornament_parameters_fail_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[0, 0], [5, 5]], "closed": false}),
    );
    let before = s.design().clone();
    for (cmd, params) in [
        ("shape.floral", json!({})),
        ("shape.floral", json!({"id": 2})),
        ("shape.floral", json!({"id": 99})),
        ("shape.floral", json!({"id": 1, "density": 0})),
        ("shape.floral", json!({"id": 1, "density": 11})),
        ("shape.floral", json!({"id": 1, "pattern": 252})),
        ("shape.floral", json!({"id": 1, "pattern": 1.5})),
        ("shape.floral", json!({"id": 1, "color": "#nothex"})),
        ("shape.decorative", json!({})),
        ("shape.decorative", json!({"kind": "laurel"})),
        ("shape.decorative", json!({"kind": "wave", "ids": []})),
        ("shape.decorative", json!({"kind": "wave", "id": 99})),
        ("shape.decorative", json!({"kind": "wave", "margin": -1})),
        ("shape.decorative", json!({"kind": "wave", "size": 0})),
        (
            "shape.decorative",
            json!({"kind": "wave", "keepAspect": "yes"}),
        ),
    ] {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd}");
    }
}
