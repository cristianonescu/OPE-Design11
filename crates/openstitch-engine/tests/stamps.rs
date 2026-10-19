//! Embossing/engraving and stamps.

use openstitch_core::Command;
use openstitch_design::Compiler;
use openstitch_engine::{Session, project};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn stitches(s: &Session) -> usize {
    Compiler::default()
        .compile(s.design())
        .count(Command::Stitch)
}

/// Longest stitch between two consecutive needle points (0.1 mm).
fn longest(s: &Session) -> f64 {
    let p = Compiler::default().compile(s.design());
    p.stitches
        .windows(2)
        .filter(|w| w[0].command == Command::Stitch && w[1].command == Command::Stitch)
        .map(|w| (w[0].x - w[1].x).hypot(w[0].y - w[1].y))
        .fold(0.0, f64::max)
}

fn base(s: &mut Session) {
    run(
        s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 60, "height": 40, "fill": "Red"}),
    );
    run(s, "shape.ellipse", json!({"cx": 40, "cy": 30, "rx": 12}));
}

#[test]
fn emboss_and_engrave_from_a_shape() {
    let plain = {
        let mut s = Session::new(std::env::temp_dir());
        base(&mut s);
        run(&mut s, "object.delete", json!({"id": 2}));
        assert!(longest(&s) <= 50.0 + 1e-6, "{}", longest(&s));
        stitches(&s)
    };
    for (effect, more) in [
        ("engravedLine", true),
        ("engravedRegion", true),
        ("embossedRegion", false),
    ] {
        let mut s = Session::new(std::env::temp_dir());
        base(&mut s);
        let r = run(
            &mut s,
            "shape.emboss",
            json!({"id": 1, "tool": 2, "effect": effect}),
        );
        assert_eq!(r["index"], json!(0));
        assert_eq!(s.design().objects.len(), 1, "the tool is used up");
        let n = stitches(&s);
        if more {
            assert!(n > plain, "{effect}: {n} vs {plain}");
        } else {
            // Embossed: long stitches over the raised area.
            assert!(longest(&s) > 60.0, "{effect}: {}", longest(&s));
        }
        run(&mut s, "history.undo", json!({}));
        assert_eq!(s.design().objects.len(), 2);
    }
}

#[test]
fn stamps_add_edit_delete_and_follow_the_shape() {
    let mut s = Session::new(std::env::temp_dir());
    base(&mut s);
    run(&mut s, "object.delete", json!({"id": 2}));
    let plain = stitches(&s);
    run(
        &mut s,
        "object.addStamp",
        json!({"id": 1, "pattern": "star", "effect": "engravedRegion", "x": 30, "y": 30, "size": 15}),
    );
    let r = run(
        &mut s,
        "object.addStamp",
        json!({"id": 1, "pattern": "heart", "effect": "embossedRegion", "x": 55, "y": 30, "rotation": 20}),
    );
    assert_eq!(r["index"], json!(1));
    assert_ne!(stitches(&s), plain);
    let bounds = |s: &Session, k: usize| {
        s.design().objects[0].region.as_ref().expect("r").stamps[k]
            .bounds()
            .expect("b")
    };
    run(
        &mut s,
        "object.editStamp",
        json!({"id": 1, "index": 0, "x": 25, "y": 25, "size": 10, "effect": "engravedLine"}),
    );
    let b = bounds(&s, 0);
    assert!(b.center().distance(openstitch_design::v2(250.0, 250.0)) < 1e-6);
    assert!((b.width().max(b.height()) - 100.0).abs() < 1e-6);
    run(&mut s, "object.move", json!({"id": 1, "dx": 10}));
    assert!((bounds(&s, 0).center().x - 350.0).abs() < 1e-6);
    let back = project::decode(&project::encode(s.design()).expect("encode")).expect("decode");
    assert_eq!(&back, s.design());
    run(&mut s, "object.deleteStamp", json!({"id": 1, "index": 0}));
    assert_eq!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .stamps
            .len(),
        1
    );
    run(&mut s, "object.deleteStamp", json!({"id": 1, "all": true}));
    assert!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .stamps
            .is_empty()
    );
}

#[test]
fn bad_stamp_parameters_fail_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    base(&mut s);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 80, "y": 80, "width": 5, "height": 5}),
    );
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[0, 0], [5, 5]], "closed": false}),
    );
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 30, "cy": 70, "rx": 5}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 5, "type": "radial"}),
    );
    let before = s.design().clone();
    for (cmd, params) in [
        ("shape.emboss", json!({"id": 1, "tool": 2})),
        (
            "shape.emboss",
            json!({"id": 1, "tool": 2, "effect": "melt"}),
        ),
        (
            "shape.emboss",
            json!({"id": 1, "tool": 1, "effect": "engravedLine"}),
        ),
        (
            "shape.emboss",
            json!({"id": 1, "tool": 3, "effect": "engravedLine"}),
        ),
        (
            "shape.emboss",
            json!({"id": 1, "tool": 4, "effect": "engravedLine"}),
        ),
        (
            "shape.emboss",
            json!({"id": 4, "tool": 2, "effect": "engravedLine"}),
        ),
        (
            "shape.emboss",
            json!({"id": 99, "tool": 2, "effect": "engravedLine"}),
        ),
        (
            "shape.emboss",
            json!({"id": 1, "tool": 99, "effect": "engravedLine"}),
        ),
        (
            "object.addStamp",
            json!({"id": 1, "pattern": "dragon", "effect": "engravedLine", "x": 1, "y": 1}),
        ),
        (
            "object.addStamp",
            json!({"id": 1, "pattern": "star", "effect": "engravedLine", "x": 1}),
        ),
        (
            "object.addStamp",
            json!({"id": 1, "pattern": "star", "effect": "engravedLine", "x": 1, "y": 1, "size": 0}),
        ),
        (
            "object.addStamp",
            json!({"id": 5, "pattern": "star", "effect": "engravedLine", "x": 1, "y": 1}),
        ),
        (
            "object.addStamp",
            json!({"id": 4, "pattern": "star", "effect": "engravedLine", "x": 1, "y": 1}),
        ),
        ("object.editStamp", json!({"id": 1, "index": 0})),
        ("object.deleteStamp", json!({"id": 1, "index": 0})),
        ("object.deleteStamp", json!({"id": 1, "all": "yes"})),
    ] {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd}");
    }
    run(
        &mut s,
        "object.addStamp",
        json!({"id": 1, "pattern": "star", "effect": "engravedLine", "x": 30, "y": 30}),
    );
    let before = s.design().clone();
    for params in [
        json!({"id": 1, "index": 1}),
        json!({"id": 1, "index": -1}),
        json!({"id": 1, "index": 0, "x": 5}),
        json!({"id": 1, "index": 0, "size": 9999}),
        json!({"id": 1, "index": 0, "effect": "melt"}),
    ] {
        assert!(
            s.execute("object.editStamp", Some(&params)).is_err(),
            "{params}"
        );
        assert_eq!(s.design(), &before);
    }
}
