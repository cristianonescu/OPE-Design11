//! Drawing and editing commands behind the ribbon's Cut, Pencil, Manual Punch (Curved Block,
//! Feed), Reverse Direction Line and Change Color buttons.

use openstitch_core::Command;
use openstitch_design::{Compiler, ObjectKind, RegionType, v2};
use openstitch_engine::{Session, project};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn session() -> Session {
    Session::new(std::env::temp_dir())
}

/// Fails, and leaves the design as it was.
fn fails(s: &mut Session, id: &str, params: Value) {
    let before = s.design().clone();
    assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
    assert_eq!(s.design(), &before, "{id} changed the design");
}

fn stitch_points(s: &Session) -> Vec<openstitch_design::Vec2> {
    Compiler::default()
        .compile(s.design())
        .stitches
        .iter()
        .filter(|x| x.command == Command::Stitch)
        .map(|x| v2(x.x, x.y))
        .collect()
}

#[test]
fn freehand_trace_becomes_a_smooth_outline() {
    let mut s = session();
    // A hand-drawn circle of radius 20 mm around (50, 50), slightly wobbly.
    let trace: Vec<Value> = (0..240)
        .map(|k| {
            let t = f64::from(k) / 240.0 * std::f64::consts::TAU;
            let r = 20.0 + 0.05 * f64::from(k % 3);
            json!([50.0 + r * t.cos(), 50.0 + r * t.sin()])
        })
        .collect();
    let r = run(&mut s, "shape.freehand", json!({ "points": trace }));
    assert_eq!(r["id"], json!(1));
    let o = &s.design().objects[0];
    let c = &o.contours[0];
    assert!(c.closed && c.has_curves());
    assert!(c.points.len() < 40, "{} vertices", c.points.len());
    assert_eq!(o.region.as_ref().map(|r| r.kind), Some(RegionType::Fill));
    for p in c.path() {
        assert!((p.distance(v2(500.0, 500.0)) - 200.0).abs() < 8.0, "{p:?}");
    }
    // Open: a running line; straight lines on request.
    run(
        &mut s,
        "shape.freehand",
        json!({ "points": [[0, 0], [10, 0.05], [20, 0], [20, 10]], "closed": false, "smooth": false, "tolerance": 0.2 }),
    );
    let o = &s.design().objects[1];
    assert!(!o.contours[0].closed && o.region.is_none() && o.line.is_some());
    assert_eq!(
        o.contours[0].points,
        vec![v2(0.0, 0.0), v2(200.0, 0.0), v2(200.0, 100.0)]
    );
    fails(
        &mut s,
        "shape.freehand",
        json!({ "points": [[1, 1], [1, 1]] }),
    );
    fails(
        &mut s,
        "shape.freehand",
        json!({ "points": [[0, 0], [5, 5], [9, 0]], "tolerance": 0 }),
    );
    fails(
        &mut s,
        "shape.freehand",
        json!({ "points": [[0, 0]], "closed": false }),
    );
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().objects.len(), 1);
}

fn bend() -> Value {
    json!([
        [[10, 10], [10, 20]],
        [[40, 10], [35, 20]],
        [[50, 35], [40, 35]]
    ])
}

#[test]
fn curved_block_curves_its_sides_and_sewing() {
    let mut s = session();
    run(&mut s, "shape.column", json!({ "lines": bend() }));
    let straight = stitch_points(&s);
    run(&mut s, "history.undo", json!({}));
    run(
        &mut s,
        "shape.column",
        json!({ "lines": bend(), "curved": true }),
    );
    let o = &s.design().objects[0];
    assert_eq!(o.name.as_deref(), Some("Curved block"));
    assert!(o.contours[0].has_curves());
    let r = o.region.as_ref().expect("region");
    assert!(r.curved_lines && r.direction_lines.len() == 3);
    let curved = stitch_points(&s);
    assert_ne!(curved, straight, "the satin follows the curved sides");
    assert!(curved.contains(&v2(500.0, 350.0)), "ends on the last line");
    // Saved and reloaded; clearing the lines makes it a plain region again.
    let back = project::decode(&project::encode(s.design()).expect("encode")).expect("decode");
    assert_eq!(&back, s.design());
    run(
        &mut s,
        "object.setDirectionLines",
        json!({ "id": 1, "lines": null }),
    );
    assert!(
        !s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .curved_lines
    );
    // Lines whose curved sides would cross are refused.
    fails(
        &mut s,
        "shape.column",
        json!({ "lines": [[[0, 0], [0, 10]], [[30, 0], [30, 10]], [[31, 40], [29, 40]], [[0, 41], [0, 51]]], "curved": true }),
    );
    fails(
        &mut s,
        "shape.column",
        json!({ "lines": bend(), "curved": "yes" }),
    );
}

#[test]
fn feed_moves_the_frame_without_sewing() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({ "x": 0, "y": 0, "width": 10, "height": 10 }),
    );
    let before = Compiler::default().compile(s.design());
    let r = run(
        &mut s,
        "shape.feed",
        json!({ "points": [[20, 0], [40, 0], [40, 30]] }),
    );
    let id = r["id"].as_u64().expect("id");
    let o = &s.design().objects[1];
    assert_eq!((o.id, o.kind), (id, ObjectKind::Stitches));
    assert_eq!(o.name.as_deref(), Some("Feed"));
    let after = Compiler::default().compile(s.design());
    assert_eq!(
        after.count(Command::Stitch),
        before.count(Command::Stitch),
        "a feed sews nothing"
    );
    let jumps: Vec<_> = after
        .stitches
        .iter()
        .filter(|x| x.command == Command::Jump)
        .map(|x| v2(x.x, x.y))
        .collect();
    for p in [v2(200.0, 0.0), v2(400.0, 0.0), v2(400.0, 300.0)] {
        assert!(jumps.contains(&p), "jump to {p:?}");
    }
    fails(&mut s, "shape.feed", json!({ "points": [[1, 1]] }));
    fails(&mut s, "shape.feed", json!({ "points": [[1, 1], [1, 1]] }));
}

#[test]
fn reverse_direction_line_sews_the_other_way() {
    let mut s = session();
    run(&mut s, "shape.column", json!({ "lines": bend() }));
    let forward = stitch_points(&s);
    run(&mut s, "object.reverseDirection", json!({ "id": 1 }));
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.direction_lines[0], [v2(100.0, 200.0), v2(100.0, 100.0)]);
    let reversed = stitch_points(&s);
    assert_ne!(reversed, forward);
    assert_eq!(
        reversed.len(),
        forward.len(),
        "same stitches, other side first"
    );
    // A plain region turns its angle; twice is back where it was.
    run(
        &mut s,
        "shape.rectangle",
        json!({ "x": 60, "y": 0, "width": 20, "height": 10 }),
    );
    run(&mut s, "object.setRegion", json!({ "id": 2, "angle": 30 }));
    run(&mut s, "object.reverseDirection", json!({ "ids": [2] }));
    assert_eq!(
        s.design().objects[1].region.as_ref().expect("r").angle,
        210.0
    );
    run(&mut s, "object.reverseDirection", json!({ "ids": [1, 2] }));
    assert_eq!(
        s.design().objects[1].region.as_ref().expect("r").angle,
        30.0
    );
    assert_eq!(stitch_points(&s)[..forward.len()], forward[..]);
    run(
        &mut s,
        "shape.polygon",
        json!({ "points": [[0, 40], [10, 40]], "closed": false }),
    );
    fails(&mut s, "object.reverseDirection", json!({ "ids": [1, 3] }));
    fails(&mut s, "object.reverseDirection", json!({ "ids": [] }));
}

#[test]
fn change_color_of_a_stitch_pattern() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({ "x": 0, "y": 0, "width": 10, "height": 10, "fill": "Red", "outline": "Black" }),
    );
    run(&mut s, "object.toStitches", json!({ "ids": [1] }));
    let id = s.design().objects[0].id;
    let block = s.design().objects[0].stitch_block.clone().expect("block");
    assert_eq!(block.threads.len(), 2);
    run(
        &mut s,
        "stitches.setThread",
        json!({ "id": id, "index": 1, "color": "#00ff00" }),
    );
    let b = s.design().objects[0].stitch_block.clone().expect("block");
    assert_eq!(b.threads[1].hex(), "#00ff00");
    assert_eq!(b.threads[0], block.threads[0]);
    assert_eq!(b.stitches, block.stitches);
    fails(
        &mut s,
        "stitches.setThread",
        json!({ "id": id, "index": 2, "color": "Red" }),
    );
    fails(
        &mut s,
        "stitches.setThread",
        json!({ "id": id, "index": 0, "color": "no such thread" }),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({ "x": 0, "y": 0, "width": 10, "height": 10 }),
    );
    let shape = s.design().objects[1].id;
    fails(
        &mut s,
        "stitches.setThread",
        json!({ "id": shape, "index": 0, "color": "Red" }),
    );
}

#[test]
fn cut_objects_paste_back_with_new_ids() {
    let mut s = session();
    for x in [0, 20] {
        run(
            &mut s,
            "shape.rectangle",
            json!({ "x": x, "y": 0, "width": 10, "height": 10 }),
        );
    }
    run(&mut s, "object.group", json!({ "ids": [1, 2] }));
    let cut: Vec<Value> = [1, 2]
        .into_iter()
        .map(|id| run(&mut s, "object.get", json!({ "id": id })))
        .collect();
    run(&mut s, "object.delete", json!({ "ids": [1, 2] }));
    assert!(s.design().objects.is_empty());
    let r = run(&mut s, "object.paste", json!({ "objects": cut, "dx": 5 }));
    let ids: Vec<u64> = r["ids"]
        .as_array()
        .expect("ids")
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(!ids.contains(&1) && !ids.contains(&2), "new ids");
    let d = s.design();
    assert_eq!(d.objects[0].group, d.objects[1].group, "still grouped");
    assert!(d.objects[0].group.is_some());
    assert_eq!(d.objects[0].contours[0].points[0], v2(50.0, 0.0));
    // Pasted again: another group.
    run(&mut s, "object.paste", json!({ "objects": cut }));
    assert_ne!(s.design().objects[2].group, s.design().objects[0].group);
    fails(&mut s, "object.paste", json!({ "objects": [] }));
    fails(
        &mut s,
        "object.paste",
        json!({ "objects": [{ "kind": "shape" }] }),
    );
    fails(
        &mut s,
        "object.paste",
        json!({ "objects": [{ "id": 9, "kind": "stitches" }] }),
    );
}
