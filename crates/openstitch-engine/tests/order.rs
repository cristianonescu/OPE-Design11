//! Sewing-order editing and per-object jump-stitch trimming.

use openstitch_core::Command;
use openstitch_design::Compiler;
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn order(s: &Session) -> Vec<u64> {
    s.design().objects.iter().map(|o| o.id).collect()
}

/// Five squares in a row: red, blue, red, blue, red (ids 1-5), 15 mm apart.
fn squares(s: &mut Session) {
    for (k, c) in ["Red", "Blue", "Red", "Blue", "Red"].iter().enumerate() {
        run(
            s,
            "shape.rectangle",
            json!({"x": k * 15, "y": 0, "width": 10, "height": 10, "fill": c}),
        );
    }
}

#[test]
fn move_first_last_earlier_later() {
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s);
    run(&mut s, "order.move", json!({"ids": [2, 4], "to": "first"}));
    assert_eq!(order(&s), [2, 4, 1, 3, 5]);
    run(&mut s, "order.move", json!({"ids": [2, 4], "to": "last"}));
    assert_eq!(order(&s), [1, 3, 5, 2, 4]);
    run(
        &mut s,
        "order.move",
        json!({"ids": [5, 4], "to": "earlier"}),
    );
    assert_eq!(order(&s), [1, 5, 3, 4, 2]);
    // Already at the start: the block stops there.
    run(
        &mut s,
        "order.move",
        json!({"ids": [1, 5], "to": "earlier"}),
    );
    assert_eq!(order(&s), [1, 5, 3, 4, 2]);
    run(&mut s, "order.move", json!({"id": 1, "to": "later"}));
    assert_eq!(order(&s), [5, 1, 3, 4, 2]);
    run(&mut s, "history.undo", json!({}));
    assert_eq!(order(&s), [1, 5, 3, 4, 2]);
    run(
        &mut s,
        "order.move",
        json!({"ids": [2, 1], "to": "before", "target": 3}),
    );
    assert_eq!(order(&s), [5, 1, 2, 3, 4]);
    run(
        &mut s,
        "order.move",
        json!({"ids": [5], "to": "after", "target": 4}),
    );
    assert_eq!(order(&s), [1, 2, 3, 4, 5]);
}

#[test]
fn by_colour_cuts_colour_changes() {
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s);
    let before = Compiler::default()
        .compile(s.design())
        .count(Command::ColorChange);
    assert_eq!(before, 4);
    run(&mut s, "order.byColor", json!({}));
    assert_eq!(order(&s), [1, 3, 5, 2, 4]);
    assert_eq!(
        Compiler::default()
            .compile(s.design())
            .count(Command::ColorChange),
        1
    );
    // Only some objects, among their own places.
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s);
    run(&mut s, "order.byColor", json!({"ids": [2, 3, 4]}));
    assert_eq!(order(&s), [1, 2, 4, 3, 5]);
}

#[test]
fn optimize_keeps_overlapping_objects_in_order() {
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s);
    // A blue square on top of red square 5: it must stay after 5.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 62, "y": 2, "width": 6, "height": 6, "fill": "Blue"}),
    );
    run(&mut s, "order.optimize", json!({}));
    let o = order(&s);
    let at = |id| o.iter().position(|&x| x == id).expect("id");
    assert!(at(6) > at(5), "{o:?}");
    let p = Compiler::default().compile(s.design());
    assert_eq!(p.count(Command::ColorChange), 1, "{o:?}");
    // A red square on top of blue 2 forces a third colour block.
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 17, "y": 2, "width": 6, "height": 6, "fill": "Red"}),
    );
    run(&mut s, "order.optimize", json!({}));
    let o = order(&s);
    let at = |id| o.iter().position(|&x| x == id).expect("id");
    assert!(at(6) > at(2), "{o:?}");
    assert_eq!(
        Compiler::default()
            .compile(s.design())
            .count(Command::ColorChange),
        2,
        "{o:?}"
    );
}

#[test]
fn jump_trimming_per_object_on_multi_needle() {
    let mut s = Session::new(std::env::temp_dir());
    for x in [0, 30, 60] {
        run(
            &mut s,
            "shape.rectangle",
            json!({"x": x, "y": 0, "width": 10, "height": 10, "fill": "Red"}),
        );
    }
    let trims = |s: &Session| {
        Compiler::for_design(s.design())
            .compile(s.design())
            .count(Command::Trim)
    };
    let all = trims(&s);
    run(
        &mut s,
        "object.setJumpTrimming",
        json!({"ids": [2, 3], "enabled": false}),
    );
    assert_eq!(trims(&s), all, "single-needle machines ignore the setting");
    run(&mut s, "machine.set", json!({"machine": "multi"}));
    let multi = trims(&s);
    run(
        &mut s,
        "object.setJumpTrimming",
        json!({"ids": [2, 3], "enabled": true}),
    );
    assert!(trims(&s) > multi, "{} vs {multi}", trims(&s));
    assert!(s.design().objects.iter().all(|o| o.trim_jumps.is_none()));
    // Colour changes still cut the thread.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 2, "color": "Blue"}),
    );
    run(
        &mut s,
        "object.setJumpTrimming",
        json!({"ids": [1, 2, 3], "enabled": false}),
    );
    let p = Compiler::for_design(s.design()).compile(s.design());
    let cut_before_change = p
        .stitches
        .windows(2)
        .any(|w| w[0].command == Command::Trim && w[1].command == Command::ColorChange);
    assert!(cut_before_change);
}

#[test]
fn bad_order_parameters_fail_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s);
    let before = s.design().clone();
    for (cmd, params) in [
        ("order.move", json!({"ids": [1]})),
        ("order.move", json!({"ids": [1], "to": "middle"})),
        ("order.move", json!({"ids": [99], "to": "first"})),
        ("order.move", json!({"ids": [], "to": "first"})),
        ("order.move", json!({"to": "first"})),
        ("order.move", json!({"ids": [1], "to": "before"})),
        (
            "order.move",
            json!({"ids": [1], "to": "before", "target": 1}),
        ),
        (
            "order.move",
            json!({"ids": [1], "to": "after", "target": 99}),
        ),
        (
            "order.move",
            json!({"ids": [1], "to": "after", "target": -2}),
        ),
        ("order.byColor", json!({"ids": [99]})),
        ("order.byColor", json!({"ids": "1"})),
        ("object.setJumpTrimming", json!({"id": 1})),
        ("object.setJumpTrimming", json!({"id": 1, "enabled": "no"})),
        (
            "object.setJumpTrimming",
            json!({"id": 99, "enabled": false}),
        ),
    ] {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd}");
    }
}
