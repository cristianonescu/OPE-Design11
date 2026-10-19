//! Drawing and editing commands: arcs, preset shapes, groups, arranged copies, shape
//! operations, offset lines, attribute pasting and entry/exit points.

use openstitch_design::{Compiler, Rect};
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn session() -> Session {
    Session::new(std::env::temp_dir())
}

fn bounds(s: &Session, id: u64) -> Rect {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")]
        .bounds()
        .expect("bounds")
}

fn id(v: &Value) -> u64 {
    v["id"].as_u64().expect("id")
}

#[test]
fn arcs_and_presets_are_one_undo_step_each() {
    let mut s = session();
    let arc = id(&run(
        &mut s,
        "shape.arc",
        json!({"cx": 50, "cy": 50, "rx": 20, "start": 0, "sweep": 90}),
    ));
    let o = &s.design().objects[0];
    assert!(!o.contours[0].closed && o.region.is_none() && o.line.is_some());
    let b = bounds(&s, arc);
    assert!((b.width() - 200.0).abs() < 1.0 && (b.height() - 200.0).abs() < 1.0);
    let fan = id(&run(
        &mut s,
        "shape.arc",
        json!({"cx": 50, "cy": 50, "rx": 20, "sweep": -180, "mode": "fan", "fill": "Red"}),
    ));
    assert!(s.design().objects[1].region.is_some());
    assert!(bounds(&s, fan).max_y <= 500.0 + 1e-9);
    for kind in [
        "polygon", "star", "heart", "triangle", "diamond", "arrow", "cross", "crescent", "teardrop",
    ] {
        let n = id(&run(
            &mut s,
            "shape.preset",
            json!({"kind": kind, "cx": 50, "cy": 50, "width": 30, "height": 20, "sides": 6}),
        ));
        let b = bounds(&s, n);
        assert!((b.width() - 300.0).abs() < 1.0, "{kind} {b:?}");
        assert!((b.height() - 200.0).abs() < 1.0, "{kind} {b:?}");
        assert!(Compiler::default().compile(s.design()).stitches.len() > 10);
    }
    let count = s.design().objects.len();
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().objects.len(), count - 1);
}

#[test]
fn rounded_rectangle_has_curved_corners() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 20, "cornerRadius": 5}),
    );
    let c = &s.design().objects[0].contours[0];
    assert_eq!(c.points.len(), 8);
    assert!(c.has_curves());
    let b = bounds(&s, 1);
    assert!((b.min_x - 100.0).abs() < 1e-9 && (b.width() - 400.0).abs() < 1e-9);
}

#[test]
fn bad_drawing_parameters_fail_cleanly() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("shape.arc", json!({"cx": 0, "cy": 0, "rx": 10})),
        ("shape.arc", json!({"cx": 0, "cy": 0, "rx": 10, "sweep": 0})),
        (
            "shape.arc",
            json!({"cx": 0, "cy": 0, "rx": 10, "sweep": 400}),
        ),
        (
            "shape.arc",
            json!({"cx": 0, "cy": 0, "rx": 10, "sweep": 90, "mode": "pie"}),
        ),
        ("shape.arc", json!({"cx": 0, "cy": 0, "rx": 0, "sweep": 90})),
        (
            "shape.arc",
            json!({"cx": "0", "cy": 0, "rx": 10, "sweep": 90}),
        ),
        (
            "shape.preset",
            json!({"kind": "blob", "cx": 0, "cy": 0, "width": 10}),
        ),
        ("shape.preset", json!({"cx": 0, "cy": 0, "width": 10})),
        (
            "shape.preset",
            json!({"kind": "star", "cx": 0, "cy": 0, "width": 10, "sides": 2}),
        ),
        (
            "shape.preset",
            json!({"kind": "star", "cx": 0, "cy": 0, "width": 10, "sides": 5.5}),
        ),
        (
            "shape.preset",
            json!({"kind": "star", "cx": 0, "cy": 0, "width": 10, "ratio": 1}),
        ),
        (
            "shape.preset",
            json!({"kind": "heart", "cx": 0, "cy": 0, "width": -1}),
        ),
        (
            "shape.preset",
            json!({"kind": "heart", "cx": 0, "cy": 0, "width": 10, "rotation": 1e300}),
        ),
        (
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": 10, "height": 10, "cornerRadius": -1}),
        ),
    ];
    for (cmd, params) in bad {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd} changed the design");
    }
}

fn three_squares(s: &mut Session) {
    for (x, fill) in [(0, "Red"), (20, "Blue"), (40, "Red")] {
        run(
            s,
            "shape.rectangle",
            json!({"x": x, "y": 0, "width": 10, "height": 10, "fill": fill}),
        );
    }
}

#[test]
fn group_ungroup_and_dissolve() {
    let mut s = session();
    three_squares(&mut s);
    let g = run(&mut s, "object.group", json!({"ids": [1, 2]}))["group"]
        .as_u64()
        .expect("group");
    assert!(g > 3);
    assert_eq!(s.design().objects[0].group, Some(g));
    assert_eq!(s.design().with_groups(&[1]), vec![1, 2]);
    let list = run(&mut s, "object.list", json!({}));
    assert_eq!(list[1]["group"], json!(g));
    // Grouping a member with another object merges the whole group in.
    let g2 = run(&mut s, "object.group", json!({"ids": [2, 3]}))["group"]
        .as_u64()
        .expect("group");
    assert!(s.design().objects.iter().all(|o| o.group == Some(g2)));
    // Regrouping an existing group changes nothing.
    let before = s.design().clone();
    run(&mut s, "object.group", json!({"ids": [1]}));
    assert_eq!(s.design(), &before);
    // A duplicate of a group is a new group.
    let dup = run(&mut s, "object.duplicate", json!({"ids": [1, 2, 3]}));
    let copy = dup["ids"][0].as_u64().expect("id");
    let cg = s.design().objects[s.design().index_of(copy).expect("copy")].group;
    assert!(cg.is_some() && cg != Some(g2));
    // Deleting all but one member dissolves the group.
    run(&mut s, "object.delete", json!({"ids": [2, 3]}));
    assert_eq!(s.design().objects[0].group, None);
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "object.ungroup", json!({"id": 3}));
    assert!(s.design().objects[..3].iter().all(|o| o.group.is_none()));
    assert!(
        s.execute("object.ungroup", Some(&json!({"id": 1})))
            .is_err()
    );
    assert!(s.execute("object.group", Some(&json!({"id": 1}))).is_err());
}

#[test]
fn mirror_copies_touch_the_original() {
    let mut s = session();
    run(
        &mut s,
        "shape.preset",
        json!({"kind": "arrow", "cx": 10, "cy": 10, "width": 10, "height": 6}),
    );
    let r = run(
        &mut s,
        "object.arrangeCopies",
        json!({"id": 1, "mode": "mirrorHorizontal", "spacing": 2}),
    );
    let c = id(&json!({"id": r["ids"][0]}));
    let (a, b) = (bounds(&s, 1), bounds(&s, c));
    assert!((b.min_x - a.max_x - 20.0).abs() < 1e-6);
    // The arrow pointed right; its mirror points left.
    let tip = |id| {
        let d = s.design();
        let o = &d.objects[d.index_of(id).expect("o")];
        o.contours[0].points[3].x
    };
    assert!((tip(c) - b.min_x).abs() < 1e-6 && (tip(1) - a.max_x).abs() < 1e-6);
    let r = run(
        &mut s,
        "object.arrangeCopies",
        json!({"id": 1, "mode": "mirrorFourWay"}),
    );
    assert_eq!(r["ids"].as_array().map(Vec::len), Some(3));
    run(&mut s, "history.undo", json!({}));
    let r = run(
        &mut s,
        "object.arrangeCopies",
        json!({"id": 1, "mode": "mirrorVertical"}),
    );
    let c = r["ids"][0].as_u64().expect("id");
    assert!((bounds(&s, c).min_y - bounds(&s, 1).max_y).abs() < 1e-6);
}

#[test]
fn circle_and_matrix_copies() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 45, "y": 10, "width": 10, "height": 4}),
    );
    let r = run(
        &mut s,
        "object.arrangeCopies",
        json!({"id": 1, "mode": "circle", "count": 4, "radius": 30}),
    );
    assert_eq!(r["ids"].as_array().map(Vec::len), Some(3));
    // Centre = 30 mm below the original's centre (50, 12) → (50, 42); copy 2 is opposite.
    let opposite = bounds(&s, r["ids"][1].as_u64().expect("id"));
    assert!(
        opposite
            .center()
            .distance(openstitch_design::v2(500.0, 720.0))
            < 1e-6
    );
    // Quarter turn: the 10 × 4 bar stands upright when rotated, stays flat when not.
    let quarter = bounds(&s, r["ids"][0].as_u64().expect("id"));
    assert!((quarter.height() - 100.0).abs() < 1e-6);
    run(&mut s, "history.undo", json!({}));
    let r = run(
        &mut s,
        "object.arrangeCopies",
        json!({"id": 1, "mode": "circle", "count": 4, "radius": 30, "rotate": false}),
    );
    assert!((bounds(&s, r["ids"][0].as_u64().expect("id")).width() - 100.0).abs() < 1e-6);
    run(&mut s, "history.undo", json!({}));
    let r = run(
        &mut s,
        "object.arrangeCopies",
        json!({"id": 1, "mode": "matrix", "rows": 2, "columns": 3, "spacingX": 1, "spacingY": -1}),
    );
    let ids: Vec<u64> = r["ids"]
        .as_array()
        .expect("ids")
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    assert_eq!(ids.len(), 5);
    let last = bounds(&s, ids[4]);
    assert!((last.min_x - (450.0 + 2.0 * 110.0)).abs() < 1e-6);
    assert!((last.min_y - (100.0 + 30.0)).abs() < 1e-6);
}

#[test]
fn arrange_optimize_order_groups_colours() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10, "fill": "Red"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 20, "width": 10, "height": 10, "fill": "Blue"}),
    );
    run(
        &mut s,
        "object.arrangeCopies",
        json!({"ids": [1, 2], "mode": "matrix", "columns": 3, "optimizeOrder": true}),
    );
    let p = Compiler::default().compile(s.design());
    assert_eq!(p.threads.len(), 2, "red copies sew together, then blue");
    let colours: Vec<String> = s
        .design()
        .objects
        .iter()
        .map(|o| o.region.as_ref().expect("region").thread.hex())
        .collect();
    assert_eq!(
        colours[..3],
        [colours[0].clone(), colours[0].clone(), colours[0].clone()]
    );
}

#[test]
fn paste_attributes_between_objects() {
    let mut s = session();
    three_squares(&mut s);
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "angle": 30, "density": 4}),
    );
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "triple", "color": "Yellow"}),
    );
    let red = s.design().objects[0].region.clone().expect("region").thread;
    run(
        &mut s,
        "object.pasteAttributes",
        json!({"from": 1, "ids": [2]}),
    );
    let (a, b) = (&s.design().objects[0], &s.design().objects[1]);
    assert_eq!(a.region, b.region);
    assert_eq!(a.line, b.line);
    // Only the line colour.
    run(
        &mut s,
        "object.pasteAttributes",
        json!({"from": 1, "id": 3, "attributes": ["regionColor"]}),
    );
    let c = &s.design().objects[2];
    assert_eq!(
        c.region.as_ref().map(|r| r.thread.clone()),
        Some(red.clone())
    );
    assert!(c.line.is_none());
    assert_eq!(
        c.region.as_ref().map(|r| r.kind),
        Some(openstitch_design::RegionType::Fill)
    );
    // Sewing without colour keeps the target's thread.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 3, "color": "Blue"}),
    );
    run(
        &mut s,
        "object.pasteAttributes",
        json!({"from": 1, "id": 3, "attributes": ["regionSewing"]}),
    );
    let c = s.design().objects[2].region.clone().expect("region");
    assert_eq!(c.kind, openstitch_design::RegionType::Satin);
    assert_ne!(c.thread, red);
}

#[test]
fn paste_attributes_text_and_shape_share_only_colours() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10, "fill": "Red"}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin"}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "A", "x": 0, "y": 30, "size": 10}),
    );
    run(
        &mut s,
        "object.pasteAttributes",
        json!({"from": 1, "id": 2}),
    );
    let t = s.design().objects[1].region.clone().expect("region");
    assert_eq!(t.kind, openstitch_design::RegionType::Fill);
    assert_eq!(
        Some(t.thread),
        s.design().objects[0].region.clone().map(|r| r.thread)
    );
}

#[test]
fn bad_group_arrange_and_paste_parameters_fail_cleanly() {
    let mut s = session();
    three_squares(&mut s);
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("object.group", json!({"ids": [1]})),
        ("object.group", json!({"ids": [1, 99]})),
        ("object.group", json!({"ids": []})),
        ("object.ungroup", json!({"ids": [1, 2]})),
        ("object.arrangeCopies", json!({"id": 1})),
        ("object.arrangeCopies", json!({"id": 1, "mode": "spiral"})),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "circle", "radius": 10}),
        ),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "circle", "count": 1, "radius": 10}),
        ),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "circle", "count": 4}),
        ),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "circle", "count": 4, "radius": 10, "cx": 5}),
        ),
        ("object.arrangeCopies", json!({"id": 1, "mode": "matrix"})),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "matrix", "rows": 101}),
        ),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "matrix", "columns": 3, "spacingX": -10}),
        ),
        (
            "object.arrangeCopies",
            json!({"ids": [1, 2, 3], "mode": "matrix", "rows": 100, "columns": 100}),
        ),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "mirrorHorizontal", "spacing": "1"}),
        ),
        ("object.pasteAttributes", json!({"id": 2})),
        ("object.pasteAttributes", json!({"from": 99, "id": 2})),
        ("object.pasteAttributes", json!({"from": 1, "id": 99})),
        (
            "object.pasteAttributes",
            json!({"from": 1, "id": 2, "attributes": []}),
        ),
        (
            "object.pasteAttributes",
            json!({"from": 1, "id": 2, "attributes": ["sparkle"]}),
        ),
        (
            "object.pasteAttributes",
            json!({"from": 1, "id": 2, "attributes": "regionColor"}),
        ),
    ];
    for (cmd, params) in bad {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd} changed the design");
    }
}

fn area(s: &Session, id: u64) -> f64 {
    let d = s.design();
    let o = &d.objects[d.index_of(id).expect("object")];
    openstitch_design::boolean::simplify(&o.contours)
        .iter()
        .map(|r| openstitch_design::boolean::region_area(r))
        .sum::<f64>()
        / 100.0
}

fn two_overlapping(s: &mut Session) {
    run(
        s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20, "fill": "Red"}),
    );
    run(
        s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20, "fill": "Blue"}),
    );
}

#[test]
fn merge_takes_the_last_sewn_attributes() {
    let mut s = session();
    two_overlapping(&mut s);
    run(&mut s, "object.setRegion", json!({"id": 2, "angle": 45}));
    let r = run(&mut s, "shape.merge", json!({"ids": [2, 1]}));
    assert_eq!(id(&r), 2);
    assert_eq!(s.design().objects.len(), 1);
    assert!((area(&s, 2) - 700.0).abs() < 0.1, "{}", area(&s, 2));
    let region = s.design().objects[0].region.clone().expect("region");
    assert_eq!(region.angle, 0.0, "direction resets");
    assert_eq!(region.thread.hex().to_lowercase(), blue_hex(&mut session()));
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().objects.len(), 2);
    // Apart: refused.
    run(&mut s, "object.move", json!({"id": 2, "dx": 50}));
    assert!(
        s.execute("shape.merge", Some(&json!({"ids": [1, 2]})))
            .is_err()
    );
}

fn blue_hex(s: &mut Session) -> String {
    run(
        s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 1, "height": 1, "fill": "Blue"}),
    );
    s.design().objects[0]
        .region
        .as_ref()
        .expect("region")
        .thread
        .hex()
        .to_lowercase()
}

#[test]
fn remove_overlap_cuts_the_lower_shape() {
    let mut s = session();
    two_overlapping(&mut s);
    let r = run(&mut s, "shape.removeOverlap", json!({"ids": [1, 2]}));
    assert_eq!(r["changed"], json!([1]));
    assert!((area(&s, 1) - 300.0).abs() < 0.1);
    assert!((area(&s, 2) - 400.0).abs() < 0.1);
    // A shape completely covered disappears.
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 5, "y": 5, "width": 5, "height": 5}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    let r = run(&mut s, "shape.removeOverlap", json!({"ids": [1, 2]}));
    assert_eq!(r["removed"], json!([1]));
    assert_eq!(s.design().objects.len(), 1);
    // Enclosed the other way round: the lower shape gets a hole.
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 5, "y": 5, "width": 5, "height": 5}),
    );
    run(&mut s, "shape.removeOverlap", json!({"ids": [1, 2]}));
    assert_eq!(s.design().objects[0].contours.len(), 2);
    assert!((area(&s, 1) - 375.0).abs() < 0.1);
    let p = Compiler::default().compile(s.design());
    assert!(p.stitches.len() > 50);
}

#[test]
fn combine_operations() {
    for (op, expected, left) in [
        ("union", 700.0, 1),
        ("difference", 300.0, 1),
        ("intersection", 100.0, 1),
        ("xor", 600.0, 1),
    ] {
        let mut s = session();
        two_overlapping(&mut s);
        let r = run(&mut s, "shape.combine", json!({"ids": [1, 2], "op": op}));
        assert_eq!(id(&r), 1);
        assert!((area(&s, 1) - expected).abs() < 0.1, "{op} {}", area(&s, 1));
        assert_eq!(s.design().objects.len(), left);
    }
    let mut s = session();
    two_overlapping(&mut s);
    run(
        &mut s,
        "shape.combine",
        json!({"ids": [2, 1], "op": "difference", "keepOperands": true}),
    );
    assert_eq!(s.design().objects.len(), 2);
    assert!((area(&s, 2) - 300.0).abs() < 0.1);
    // Curved shapes work too.
    let mut s = session();
    run(
        &mut s,
        "shape.arc",
        json!({"cx": 20, "cy": 20, "rx": 10, "sweep": 360}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 20, "y": 0, "width": 40, "height": 40}),
    );
    run(
        &mut s,
        "shape.combine",
        json!({"ids": [1, 2], "op": "difference"}),
    );
    let half = std::f64::consts::PI * 100.0 / 2.0;
    assert!((area(&s, 1) - half).abs() < half * 0.01);
}

#[test]
fn split_along_a_cut_outline() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20, "fill": "Red"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 50, "y": 0, "width": 5, "height": 5}),
    );
    let r = run(
        &mut s,
        "shape.split",
        json!({"id": 1, "points": [[-5, -5], [10, -5], [10, 25], [-5, 25]]}),
    );
    let piece = r["piece"].as_u64().expect("piece");
    assert_eq!(s.design().objects[1].id, piece, "sewn right after");
    assert!((area(&s, 1) - 200.0).abs() < 0.1);
    assert!((area(&s, piece) - 200.0).abs() < 0.1);
    assert_eq!(
        s.design().objects[0]
            .region
            .as_ref()
            .map(|r| r.thread.clone()),
        s.design().objects[1]
            .region
            .as_ref()
            .map(|r| r.thread.clone())
    );
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().objects.len(), 2);
}

#[test]
fn split_detaches_text_on_the_path() {
    let mut s = session();
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 30}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "AB", "x": 0, "y": 0, "size": 8, "path": 1}),
    );
    run(
        &mut s,
        "shape.split",
        json!({"id": 1, "points": [[0, 0], [50, 0], [50, 100], [0, 100]]}),
    );
    assert!(
        s.design()
            .objects
            .iter()
            .all(|o| o.text.as_ref().is_none_or(|t| t.path.is_none()))
    );
}

#[test]
fn offset_lines_outward_and_inward() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20, "outline": "Black"}),
    );
    let r = run(
        &mut s,
        "shape.offsetLines",
        json!({"id": 1, "spacing": 2, "count": 3}),
    );
    let lines = id(&r);
    assert_eq!(s.design().objects[1].id, lines);
    let o = &s.design().objects[1];
    assert_eq!(o.contours.len(), 3);
    assert!(o.region.is_none() && o.line.is_some());
    let b = bounds(&s, lines);
    assert!((b.width() - 320.0).abs() < 0.5, "{b:?}");
    // Inward stops when the shape runs out: 20 mm square, 4 mm steps → 2 lines (4, 8 mm).
    let r = run(
        &mut s,
        "shape.offsetLines",
        json!({"id": 1, "direction": "inward", "spacing": 4, "count": 5, "roundedCorners": true}),
    );
    let d = s.design();
    let inner = &d.objects[d.index_of(id(&r)).expect("o")];
    assert_eq!(inner.contours.len(), 2);
    assert!(Compiler::default().compile(s.design()).stitches.len() > 100);
    // Text works as a source too.
    run(
        &mut s,
        "text.add",
        json!({"text": "O", "x": 50, "y": 50, "size": 20}),
    );
    let t = s.design().objects.last().expect("text").id;
    run(&mut s, "shape.offsetLines", json!({"id": t, "spacing": 1}));
}

#[test]
fn arrange_can_merge_overlapping_copies() {
    let mut s = session();
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 20, "rx": 8}),
    );
    let r = run(
        &mut s,
        "object.arrangeCopies",
        json!({"id": 1, "mode": "circle", "count": 8, "radius": 15, "mergeOverlapping": true}),
    );
    assert_eq!(r["objects"].as_array().map(Vec::len), Some(1));
    assert_eq!(s.design().objects.len(), 1);
    // A ring: the merged outline has a hole.
    assert_eq!(s.design().objects[0].contours.len(), 2);
}

#[test]
fn bad_shape_operation_parameters_fail_cleanly() {
    let mut s = session();
    two_overlapping(&mut s);
    run(
        &mut s,
        "text.add",
        json!({"text": "A", "x": 0, "y": 50, "size": 10}),
    );
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[0, 0], [5, 5], [9, 0]], "closed": false}),
    );
    run(
        &mut s,
        "object.setHoleSewing",
        json!({"id": 2, "enabled": true}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("shape.merge", json!({"ids": [1]})),
        ("shape.merge", json!({"ids": [1, 1]})),
        ("shape.merge", json!({"ids": [1, 3]})),
        ("shape.merge", json!({"ids": [1, 4]})),
        ("shape.merge", json!({"ids": [1, 99]})),
        ("shape.removeOverlap", json!({"ids": [1]})),
        ("shape.removeOverlap", json!({"ids": [1, 3]})),
        ("shape.combine", json!({"ids": [1, 2]})),
        ("shape.combine", json!({"ids": [1, 2], "op": "add"})),
        ("shape.combine", json!({"ids": [1, 1], "op": "union"})),
        ("shape.combine", json!({"ids": [1], "op": "union"})),
        (
            "shape.combine",
            json!({"ids": [1, 2], "op": "union", "keepOperands": 1}),
        ),
        ("shape.split", json!({"id": 1})),
        ("shape.split", json!({"id": 1, "points": [[0, 0], [1, 1]]})),
        (
            "shape.split",
            json!({"id": 1, "points": [[-5, -5], [10, 25], [10, -5], [-5, 25]]}),
        ),
        (
            "shape.split",
            json!({"id": 1, "points": [[100, 100], [110, 100], [110, 110]]}),
        ),
        (
            "shape.split",
            json!({"id": 1, "points": [[-5, -5], [50, -5], [50, 50], [-5, 50]]}),
        ),
        (
            "shape.split",
            json!({"id": 2, "points": [[-5, -5], [15, -5], [15, 50], [-5, 50]]}),
        ),
        (
            "shape.split",
            json!({"id": 3, "points": [[-5, -5], [15, -5], [15, 80], [-5, 80]]}),
        ),
        ("shape.offsetLines", json!({"id": 4})),
        (
            "shape.offsetLines",
            json!({"id": 1, "direction": "sideways"}),
        ),
        ("shape.offsetLines", json!({"id": 1, "spacing": 0})),
        ("shape.offsetLines", json!({"id": 1, "count": 21})),
        (
            "shape.offsetLines",
            json!({"id": 1, "direction": "inward", "spacing": 50}),
        ),
        ("shape.offsetLines", json!({"id": 99})),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "matrix", "rows": 20, "columns": 20, "mergeOverlapping": true}),
        ),
    ];
    for (cmd, params) in bad {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd} changed the design");
    }
}

/// Needle positions (mm) of the first and last stitch of the design.
fn first_and_last_stitch(s: &Session) -> ((f64, f64), (f64, f64)) {
    let p = Compiler::default().compile(s.design());
    let st: Vec<_> = p
        .stitches
        .iter()
        .filter(|x| x.command == openstitch_core::Command::Stitch)
        .collect();
    let (a, b) = (st[0], st[st.len() - 1]);
    ((a.x / 10.0, a.y / 10.0), (b.x / 10.0, b.y / 10.0))
}

#[test]
fn region_entry_and_exit_points() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 30, "height": 20, "fill": "Red"}),
    );
    let r = run(
        &mut s,
        "object.setEntryExit",
        json!({"id": 1, "regionEntry": [5, 22], "regionExit": [41, 12]}),
    );
    assert_eq!(
        r["regionEntry"],
        json!([10.0, 22.0]),
        "snapped to the left edge"
    );
    assert_eq!(r["regionExit"], json!([40.0, 12.0]));
    let (first, last) = first_and_last_stitch(&s);
    assert_eq!(first, (10.0, 22.0));
    assert_eq!(last, (40.0, 12.0));
    // Moving the shape moves its points; the .osd keeps them.
    run(&mut s, "object.move", json!({"id": 1, "dx": 5}));
    let (first, _) = first_and_last_stitch(&s);
    assert_eq!(first, (15.0, 22.0));
    let saved = openstitch_engine::project::encode(s.design()).expect("encode");
    let back = openstitch_engine::project::decode(&saved).expect("decode");
    assert_eq!(&back, s.design());
    // Underlay starts at the entry too.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "underlay": true}),
    );
    assert_eq!(first_and_last_stitch(&s).0, (15.0, 22.0));
    // Clearing.
    run(
        &mut s,
        "object.setEntryExit",
        json!({"id": 1, "regionEntry": null, "regionExit": null}),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert!(r.entry.is_none() && r.exit.is_none());
}

#[test]
fn line_entry_point_rotates_the_outline_start() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20, "outline": "Black"}),
    );
    run(
        &mut s,
        "object.setEntryExit",
        json!({"id": 1, "lineEntry": [20, 7]}),
    );
    let (first, last) = first_and_last_stitch(&s);
    assert_eq!(first, (20.0, 7.0));
    assert_eq!(last, (20.0, 7.0));
    // An open line starts from the nearer end.
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[50, 0], [60, 0], [60, 10]], "closed": false}),
    );
    run(&mut s, "object.delete", json!({"id": 1}));
    run(
        &mut s,
        "object.setEntryExit",
        json!({"id": 2, "lineEntry": [61, 11]}),
    );
    assert_eq!(first_and_last_stitch(&s), ((60.0, 10.0), (50.0, 0.0)));
}

#[test]
fn bad_entry_exit_parameters_fail_cleanly() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "A", "x": 0, "y": 50, "size": 10}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("object.setEntryExit", json!({"id": 1})),
        ("object.setEntryExit", json!({"id": 1, "regionEntry": [1]})),
        (
            "object.setEntryExit",
            json!({"id": 1, "regionEntry": "1,2"}),
        ),
        (
            "object.setEntryExit",
            json!({"id": 1, "regionEntry": [1e9, 0]}),
        ),
        ("object.setEntryExit", json!({"id": 1, "lineEntry": [1, 1]})),
        (
            "object.setEntryExit",
            json!({"id": 2, "regionEntry": [1, 1]}),
        ),
        (
            "object.setEntryExit",
            json!({"id": 99, "regionEntry": [1, 1]}),
        ),
        ("object.setEntryExit", json!({"regionEntry": [1, 1]})),
    ];
    for (cmd, params) in bad {
        assert!(s.execute(cmd, Some(&params)).is_err(), "{cmd} {params}");
        assert_eq!(s.design(), &before, "{cmd} changed the design");
    }
}
