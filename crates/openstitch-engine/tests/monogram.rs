//! Monograms and decorative frames: `monogram.add`, `monogram.edit`, `decoration.add`.

use openstitch_design::{ObjectKind, Rect};
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn ids(v: &Value) -> Vec<u64> {
    v["ids"]
        .as_array()
        .expect("ids")
        .iter()
        .filter_map(Value::as_u64)
        .collect()
}

fn bounds(s: &Session, ids: &[u64]) -> Rect {
    let d = s.design();
    Rect::enclosing(
        ids.iter()
            .flat_map(|&id| d.objects[d.index_of(id).expect("object")].points()),
    )
    .expect("bounds")
}

#[test]
fn styles_and_frames_are_listed() {
    let mut s = Session::new(std::env::temp_dir());
    let l = run(&mut s, "monogram.list", json!({}));
    assert_eq!(l["styles"].as_array().expect("styles").len(), 4);
    assert_eq!(l["frames"].as_array().expect("frames").len(), 8);
}

#[test]
fn block_monogram_in_a_ring_is_one_group_of_text_and_frame() {
    let mut s = Session::new(std::env::temp_dir());
    let r = run(
        &mut s,
        "monogram.add",
        json!({"letters": "jdm", "size": 30, "x": 50, "y": 50, "frame": "ring",
               "fill": "Red", "frameColor": "Blue"}),
    );
    assert_eq!(r["letters"], "JDM");
    let all = ids(&r);
    assert_eq!(all.len(), 4, "ring + 3 letters");
    let d = s.design();
    let objs: Vec<_> = all
        .iter()
        .map(|&id| &d.objects[d.index_of(id).expect("o")])
        .collect();
    let group = objs[0].group.expect("grouped");
    assert!(objs.iter().all(|o| o.group == Some(group)));
    assert_eq!(objs[0].kind, ObjectKind::Shape, "frame sews first");
    assert!(objs[1..].iter().all(|o| o.kind == ObjectKind::Text));
    assert!(objs[1].monogram.is_some() && objs[2].monogram.is_none());
    assert_eq!(objs[2].text.as_ref().expect("text").text, "D");
    let letters = bounds(&s, &all[1..]);
    let ring = bounds(&s, &all[..1]);
    assert!((letters.center().x - 500.0).abs() < 1.0 && (letters.center().y - 500.0).abs() < 1.0);
    assert!((letters.height() - 300.0).abs() < 3.0, "{letters:?}");
    assert!(ring.min_x < letters.min_x && ring.max_x > letters.max_x);
    assert!(
        (ring.width() - ring.height()).abs() < 1.0,
        "ring keeps its aspect"
    );
    let stats = run(&mut s, "design.generate", json!({}));
    assert_eq!(stats["colors"], json!(["#0a55a3", "#ed171f"]));
    // Letters stay editable text.
    run(&mut s, "object.setText", json!({"id": all[2], "text": "Q"}));
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().objects.is_empty(), "one undo step");
}

#[test]
fn diamond_monogram_letters_are_clipped_shapes() {
    let mut s = Session::new(std::env::temp_dir());
    let r = run(
        &mut s,
        "monogram.add",
        json!({"letters": "AMW", "style": "diamond", "size": 40, "frame": "diamond"}),
    );
    let all = ids(&r);
    assert_eq!(all.len(), 4);
    let letters = bounds(&s, &all[1..]);
    assert!(letters.width() > 0.9 * 46.0 * 10.0, "{letters:?}");
    assert!(
        s.design()
            .objects
            .iter()
            .skip(1)
            .all(|o| o.kind == ObjectKind::Shape)
    );
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64() > Some(1000), "{stats}");
}

#[test]
fn editing_rebuilds_the_monogram_in_place() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 5, "height": 5}),
    );
    let r = run(
        &mut s,
        "monogram.add",
        json!({"letters": "AB", "x": 40, "y": 60, "frame": "shield", "fill": "Red", "frameColor": "Black"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 90, "y": 90, "width": 5, "height": 5}),
    );
    let before = bounds(&s, &ids(&r));
    run(
        &mut s,
        "object.move",
        json!({"ids": ids(&r), "dx": 10, "dy": 0}),
    );
    let e = run(
        &mut s,
        "monogram.edit",
        json!({"id": ids(&r)[2], "letters": "xyz", "style": "circle", "frame": "laurel"}),
    );
    assert_eq!(e["letters"], "XYZ");
    let new_ids = ids(&e);
    let d = s.design();
    // Still between the two rectangles in sewing order.
    assert_eq!(d.objects.first().map(|o| o.kind), Some(ObjectKind::Shape));
    let last = d.objects.last().expect("last");
    assert!(
        !new_ids.contains(&last.id),
        "the later rectangle stays after the monogram"
    );
    // The letters stay centred where the (moved) monogram's letters were.
    let letters: Vec<u64> = new_ids
        .iter()
        .copied()
        .filter(|&i| {
            !d.objects[d.index_of(i).expect("o")]
                .name
                .as_deref()
                .is_some_and(|n| n.starts_with("Frame"))
        })
        .collect();
    let after = bounds(&s, &letters);
    assert!((after.center().x - 500.0).abs() < 2.0, "{after:?}");
    assert!((after.center().y - 600.0).abs() < 2.0, "{after:?}");
    assert!(before.width() > 0.0);
    let letter = &d.objects[d
        .index_of(
            new_ids
                .iter()
                .copied()
                .find(|&i| d.objects[d.index_of(i).expect("o")].monogram.is_some())
                .expect("holder"),
        )
        .expect("o")];
    assert_eq!(
        letter.region.as_ref().expect("region").thread.hex(),
        "#ed171f",
        "colour kept"
    );
    // Remove the frame.
    let e2 = run(
        &mut s,
        "monogram.edit",
        json!({"id": new_ids[0], "frame": null}),
    );
    assert_eq!(ids(&e2).len(), 3);
    // Round trip through .osd keeps the spec.
    let dir = std::env::temp_dir().join(format!("openstitch-monogram-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let mut t = Session::new(&dir);
    std::fs::write(
        dir.join("m.osd"),
        openstitch_engine::project::encode(s.design()).expect("enc"),
    )
    .expect("write");
    run(&mut t, "project.open", json!({"path": "m.osd"}));
    assert_eq!(t.design(), s.design());
    run(
        &mut t,
        "monogram.edit",
        json!({"id": ids(&e2)[0], "size": 20}),
    );
}

#[test]
fn decorations_frame_any_selection() {
    let mut s = Session::new(std::env::temp_dir());
    let t = run(
        &mut s,
        "text.add",
        json!({"text": "Hello", "size": 10, "x": 50, "y": 50}),
    )["id"]
        .as_u64()
        .expect("id");
    let inner = bounds(&s, &[t]);
    let r = run(
        &mut s,
        "decoration.add",
        json!({"frame": "ring", "ids": [t], "maintainAspect": false, "color": "#00a000"}),
    );
    let f = bounds(&s, &ids(&r));
    assert!(f.width() / f.height() > 2.0, "stretched to the text: {f:?}");
    assert!(f.min_x < inner.min_x && f.max_y > inner.max_y);
    assert_eq!(s.design().objects[0].id, ids(&r)[0], "sewn before the text");
    let r = run(
        &mut s,
        "decoration.add",
        json!({"frame": "stars", "x": 0, "y": 0, "width": 20, "height": 20}),
    );
    assert_eq!(ids(&r).len(), 12);
}

#[test]
fn monogram_bad_input_fails_cleanly() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("monogram.add", json!({})),
        ("monogram.add", json!({"letters": ""})),
        ("monogram.add", json!({"letters": "ABCD"})),
        ("monogram.add", json!({"letters": "A!"})),
        ("monogram.add", json!({"letters": "AB", "style": "gothic"})),
        ("monogram.add", json!({"letters": "AB", "font": "nope"})),
        ("monogram.add", json!({"letters": "AB", "size": 0})),
        ("monogram.add", json!({"letters": "AB", "size": 1e9})),
        ("monogram.add", json!({"letters": "AB", "frame": "spiral"})),
        ("monogram.add", json!({"letters": "AB", "margin": -1})),
        (
            "monogram.add",
            json!({"letters": "AB", "maintainAspect": "yes"}),
        ),
        ("monogram.add", json!({"letters": "AB", "fill": "#xyz"})),
        (
            "monogram.add",
            json!({"letters": "\u{4E2D}", "style": "diamond"}),
        ),
        ("monogram.edit", json!({"id": 1, "letters": "A"})),
        ("monogram.edit", json!({"id": 99})),
        ("decoration.add", json!({"frame": "ring"})),
        ("decoration.add", json!({"frame": "nope", "ids": [1]})),
        ("decoration.add", json!({"frame": null, "ids": [1]})),
        ("decoration.add", json!({"frame": "ring", "ids": [99]})),
        (
            "decoration.add",
            json!({"frame": "ring", "x": 0, "y": 0, "width": 0, "height": 5}),
        ),
    ];
    for (id, params) in bad {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
}
