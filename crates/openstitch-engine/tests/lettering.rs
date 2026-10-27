use openstitch_design::{Compiler, ObjectKind, Rect, v2};
use openstitch_engine::{Session, project};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-lettering-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn bounds(s: &Session, id: u64) -> Rect {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")]
        .bounds()
        .expect("bounds")
}

#[test]
fn fonts_are_listed() {
    let mut s = Session::new(temp_dir());
    let fonts = run(&mut s, "fonts.list", json!({}));
    let ids: Vec<&str> = fonts
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|f| f["id"].as_str())
        .collect();
    assert_eq!(ids, ["lato-bold", "pacifico"]);
    assert!(
        fonts
            .as_array()
            .expect("array")
            .iter()
            .all(|f| f["license"] == "OFL-1.1" && f["glyphs"].as_u64() > Some(90))
    );
    assert!(!s.can_undo());
}

#[test]
fn text_is_added_sewn_and_undone() {
    let mut s = Session::new(temp_dir());
    let r = run(
        &mut s,
        "text.add",
        json!({"text": "OpenStitch", "size": 10, "x": 50, "y": 50, "fill": "Red", "name": "title"}),
    );
    assert_eq!(r["glyphs"], 10);
    assert_eq!(r["missing"], json!([]));
    let id = r["id"].as_u64().expect("id");
    let o = &s.design().objects[0];
    assert_eq!(o.kind, ObjectKind::Text);
    assert_eq!(o.glyph_groups().len(), 10);
    // Centred on x = 50 mm, baseline at y = 50 mm; size is 0.7 em, so the ascenders of
    // "OpenStitch" (Lato Bold: 1446 units capitals, 2000 em) reach about 10.6 mm up.
    let b = bounds(&s, id);
    assert!((b.center().x - 500.0).abs() < 5.0, "{b:?}");
    assert!(b.min_y > 390.0 && b.min_y < 400.0, "{b:?}");
    let listed = run(&mut s, "object.list", json!({}));
    assert_eq!(listed[0]["kind"], "text");
    assert_eq!(listed[0]["text"], "OpenStitch");
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64() > Some(500), "{stats}");
    assert_eq!(stats["colors"], json!(["#ed171f"]));
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().objects.is_empty());
}

#[test]
fn set_text_changes_size_font_and_string() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "text.add",
        json!({"text": "Hi", "size": 10, "align": "left", "x": 10, "y": 30}),
    );
    let before = bounds(&s, 1);
    run(&mut s, "object.setText", json!({"id": 1, "size": 20}));
    let after = bounds(&s, 1);
    assert!((after.width() / before.width() - 2.0).abs() < 1e-6);
    // Left alignment keeps the left edge's alignment point at x = 10 mm.
    assert!((after.min_x - 100.0).abs() < (before.min_x - 100.0).abs() * 2.0 + 1e-6);
    run(
        &mut s,
        "object.setText",
        json!({"id": 1, "text": "Hi!", "font": "Pacifico"}),
    );
    let t = s.design().objects[0].text.clone().expect("text");
    assert_eq!(
        (t.text.as_str(), t.font.as_str(), t.size),
        ("Hi!", "pacifico", 20.0)
    );
    // Region and line attributes apply to text as to shapes.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "color": "Blue"}),
    );
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "running", "color": "Black"}),
    );
    let stats = run(&mut s, "design.generate", json!({}));
    assert_eq!(stats["colors"].as_array().map(Vec::len), Some(2));
}

#[test]
fn moved_text_keeps_its_place_through_edits() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "text.add", json!({"text": "AB", "size": 10}));
    run(&mut s, "object.move", json!({"id": 1, "dx": 10, "dy": -5}));
    run(&mut s, "object.rotate", json!({"id": 1, "degrees": 90}));
    let turned = bounds(&s, 1);
    assert!(turned.height() > turned.width(), "rotated text stands up");
    // Changing the string re-lays it out under the same placement.
    run(&mut s, "object.setText", json!({"id": 1, "text": "ABAB"}));
    let longer = bounds(&s, 1);
    assert!(longer.height() > turned.height() * 1.8);
    assert!((longer.width() - turned.width()).abs() < 1.0);
}

#[test]
fn text_follows_its_path() {
    let mut s = Session::new(temp_dir());
    // An arc: from (10, 60) bulging up to (90, 60).
    run(
        &mut s,
        "shape.path",
        json!({"points": [[10, 60], [90, 60]], "closed": false,
               "curves": [[[25, 20], [75, 20]]], "outline": "Black"}),
    );
    let r = run(
        &mut s,
        "text.add",
        json!({"text": "ARCH", "size": 8, "path": 1}),
    );
    assert_eq!(r["glyphs"], 4);
    let path = s.design().objects[0].contours[0].clone();
    let line = path.path();
    let near_path = |s: &Session| {
        s.design().objects[1]
            .glyph_groups()
            .iter()
            .map(|g| {
                let c = Rect::enclosing(g.iter().flat_map(|c| c.outline_points()))
                    .expect("bounds")
                    .center();
                line.iter()
                    .map(|p| p.distance(c))
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(0.0, f64::max)
    };
    assert!(near_path(&s) < 60.0, "{}", near_path(&s));
    // Centred along the path: the middle glyphs straddle the apex at x = 50 mm.
    let b = bounds(&s, 2);
    assert!((b.center().x - 500.0).abs() < 10.0, "{b:?}");

    // Editing the path moves the text with it.
    run(&mut s, "object.move", json!({"id": 1, "dx": 0, "dy": 20}));
    let moved = bounds(&s, 2);
    assert!((moved.min_y - b.min_y - 200.0).abs() < 1e-6, "{moved:?}");

    // Detaching keeps it in place as straight text.
    run(&mut s, "text.setPath", json!({"id": 2, "path": null}));
    let t = s.design().objects[1].text.clone().expect("text");
    assert_eq!(t.path, None);
    let straight = bounds(&s, 2);
    assert!((straight.center().x - moved.center().x).abs() < 20.0);

    // Re-attach, then delete the path: the text detaches itself; undo re-attaches.
    run(&mut s, "text.setPath", json!({"id": 2, "path": 1}));
    run(&mut s, "object.delete", json!({"id": 1}));
    assert_eq!(
        s.design().objects[0].text.as_ref().and_then(|t| t.path),
        None
    );
    run(&mut s, "history.undo", json!({}));
    assert_eq!(
        s.design().objects[1].text.as_ref().and_then(|t| t.path),
        Some(1)
    );
    let _ = v2(0.0, 0.0);
}

#[test]
fn fit_text_to_outline_options_and_one_text_per_outline() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 30}),
    );
    run(&mut s, "text.add", json!({"text": "ABCDE", "size": 8}));
    run(&mut s, "text.add", json!({"text": "XY", "size": 8}));
    run(
        &mut s,
        "text.setPath",
        json!({"id": 2, "path": 1, "align": "center", "valign": "middle", "otherSide": true}),
    );
    let t = s.design().objects[1].text.clone().expect("text");
    assert_eq!(t.path, Some(1));
    assert_eq!(t.fit.valign, openstitch_design::VAlign::Middle);
    assert!(t.fit.other_side && !t.fit.upright);
    // A second text on the same outline is refused and changes nothing.
    let before = s.design().clone();
    let e = s
        .execute("text.setPath", Some(&json!({"id": 3, "path": 1})))
        .expect_err("second text");
    assert!(e.to_string().contains("only one text"), "{e}");
    assert!(
        s.execute("text.add", Some(&json!({"text": "Z", "path": 1})))
            .is_err()
    );
    assert_eq!(s.design(), &before);
    // Released, the outline takes another text; fitting again keeps the text's own options.
    run(&mut s, "text.setPath", json!({"id": 2, "path": null}));
    run(
        &mut s,
        "text.setPath",
        json!({"id": 3, "path": 1, "upright": true}),
    );
    let t = s.design().objects[2].text.clone().expect("text");
    // First fitted: centred, as PE-DESIGN's dialog defaults.
    assert!(t.fit.upright && t.align == openstitch_design::TextAlign::Center);
    run(
        &mut s,
        "text.setPath",
        json!({"id": 3, "path": 1, "align": "spaced"}),
    );
    let t = s.design().objects[2].text.clone().expect("text");
    assert_eq!(t.fit.spread, openstitch_design::Spread::Even);
    assert!(t.fit.upright, "other options kept");
    run(
        &mut s,
        "text.setPath",
        json!({"id": 3, "path": 1, "align": "right"}),
    );
    let t = s.design().objects[2].text.clone().expect("text");
    assert_eq!(t.fit.spread, openstitch_design::Spread::None);
    assert_eq!(t.align, openstitch_design::TextAlign::Right);
    for bad in [
        json!({"id": 3, "path": 1, "valign": "above"}),
        json!({"id": 3, "path": 1, "otherSide": 2}),
    ] {
        assert!(s.execute("text.setPath", Some(&bad)).is_err(), "{bad}");
    }
}

#[test]
fn convert_to_shapes_keeps_the_stitches() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "text.add",
        json!({"text": "Bob 8", "size": 12, "fill": "Red", "name": "label"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 1, "y": 1, "width": 5, "height": 5}),
    );
    let before = Compiler::default().compile(s.design());
    let r = run(&mut s, "text.toShapes", json!({"id": 1}));
    let ids: Vec<u64> = r["ids"]
        .as_array()
        .expect("ids")
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    assert_eq!(ids.len(), 4, "B, o, b, 8");
    let d = s.design();
    assert_eq!(d.objects.len(), 5);
    assert!(
        d.objects[..4]
            .iter()
            .all(|o| o.kind == ObjectKind::Shape && o.text.is_none())
    );
    assert_eq!(d.objects[0].name.as_deref(), Some("label 'B'"));
    assert_eq!(d.objects[1].contours.len(), 2, "o keeps its counter");
    assert_eq!(d.objects[4].id, 2, "rectangle stays after the glyphs");
    assert_eq!(Compiler::default().compile(d), before);
    // Converted glyphs are ordinary shapes: their points can be edited.
    run(
        &mut s,
        "object.deletePoint",
        json!({"id": ids[0], "index": 0}),
    );
}

#[test]
fn osd_round_trip_and_backward_compatibility() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "shape.path",
        json!({"points": [[10, 80], [90, 80]], "closed": false}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "Two\nlines", "size": 9, "letterSpacing": 0.5, "lineSpacing": 2, "align": "right", "x": 90, "y": 20}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "path", "path": 1, "font": "pacifico"}),
    );
    run(&mut s, "object.rotate", json!({"id": 2, "degrees": 15}));
    run(&mut s, "project.save", json!({"path": "t.osd"}));
    let saved = s.design().clone();
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "t.osd"}));
    assert_eq!(t.design(), &saved);

    // A shape serializes exactly as before lettering existed (no text key), and a
    // text object without the optional fields still loads.
    let json = String::from_utf8(project::encode(s.design()).expect("encode")).expect("utf8");
    let doc: Value = serde_json::from_str(&json).expect("json");
    assert!(doc["design"]["objects"][0].get("text").is_none());
    let mut minimal = doc;
    let text = &mut minimal["design"]["objects"][1]["text"];
    let obj = text.as_object_mut().expect("text object");
    for key in [
        "letterSpacing",
        "lineSpacing",
        "align",
        "transform",
        "glyphs",
    ] {
        obj.remove(key);
    }
    let d = project::decode(&serde_json::to_vec(&minimal).expect("bytes")).expect("decode");
    let spec = d.objects[1].text.clone().expect("text");
    assert_eq!(spec.letter_spacing, 0.0);
    assert_eq!(spec.glyphs, Vec::<u32>::new());
    // Without glyph counts the object is filled as one region; it still compiles.
    assert!(Compiler::default().compile(&d).stitches.len() > 100);

    // Out-of-range text attributes are rejected.
    assert!(json.contains("\"size\": 9.0"));
    let bad = json.replacen("\"size\": 9.0", "\"size\": 1e400", 1);
    assert!(project::decode(bad.as_bytes()).is_err());
}

#[test]
fn lettering_bad_input_fails_cleanly() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    run(&mut s, "text.add", json!({"text": "ok"}));
    let before = s.design().clone();
    let long = "W".repeat(10_000);
    let wide = "W".repeat(2_000);
    let bad: Vec<(&str, Value)> = vec![
        ("text.add", json!({})),
        ("text.add", json!({"text": ""})),
        ("text.add", json!({"text": 42})),
        ("text.add", json!({"text": long})),
        ("text.add", json!({"text": wide, "size": 300})),
        ("text.add", json!({"text": "A", "size": 1e9})),
        ("text.add", json!({"text": "A", "size": 0})),
        ("text.add", json!({"text": "A", "size": -5})),
        ("text.add", json!({"text": "A", "size": "big"})),
        ("text.add", json!({"text": "A", "font": "Comic Sans"})),
        ("text.add", json!({"text": "A", "font": 3})),
        ("text.add", json!({"text": "A", "align": "justify"})),
        ("text.add", json!({"text": "A", "letterSpacing": 1e6})),
        ("text.add", json!({"text": "A", "x": 1e12})),
        ("text.add", json!({"text": "   \n\t "})),
        ("text.add", json!({"text": "\u{4E2D}\u{6587}"})),
        ("text.add", json!({"text": "A", "path": 999})),
        ("text.add", json!({"text": "A", "path": 2})),
        ("text.add", json!({"text": "A", "path": -1})),
        ("text.add", json!({"text": "A", "fill": "#nothex"})),
        ("object.setText", json!({"id": 1, "text": "x"})),
        ("object.setText", json!({"id": 2, "text": ""})),
        ("object.setText", json!({"id": 2, "font": "nope"})),
        ("object.setText", json!({"id": 99, "size": 5})),
        ("text.setPath", json!({"id": 2, "path": 2})),
        ("text.setPath", json!({"id": 1, "path": 2})),
        ("text.setPath", json!({"id": 2, "path": "1"})),
        ("text.toShapes", json!({"id": 1})),
        ("text.toShapes", json!({"ids": []})),
        (
            "object.setPoint",
            json!({"id": 2, "index": 0, "x": 1, "y": 1}),
        ),
        ("object.setSegment", json!({"id": 2, "segment": 0})),
    ];
    for (id, params) in bad {
        let r = s.execute(id, Some(&params));
        assert!(r.is_err(), "{id} {params} should fail");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
}

#[test]
fn longest_text_lays_out_and_sews() {
    let mut s = Session::new(temp_dir());
    let line = "The quick brown fox jumps over the lazy dog 0123456\n";
    let text: String = line.repeat(40).chars().take(2000).collect();
    assert_eq!(text.chars().count(), 2000);
    let r = run(
        &mut s,
        "text.add",
        json!({"text": text, "size": 4, "align": "left", "x": 5, "y": 10}),
    );
    assert!(r["glyphs"].as_u64() > Some(1500));
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64() > Some(10_000));
    let too_long: String = "a".repeat(2001);
    assert!(
        s.execute("object.setText", Some(&json!({"id": 1, "text": too_long})))
            .is_err()
    );
}
