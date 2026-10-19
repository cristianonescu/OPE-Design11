//! Font Creator: embroidery fonts built from shape objects and TrueType glyphs, edited as
//! shapes, saved as `.osfont` and used by text objects.

use openstitch_core::Command;
use openstitch_design::{Compiler, ObjectKind, Rect};
use openstitch_engine::Session;
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-fontcreator-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn object_bounds(s: &Session, id: u64) -> Rect {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")]
        .bounds()
        .expect("bounds")
}

fn contours_of(s: &Session, id: u64) -> Vec<openstitch_design::Contour> {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")].contours.clone()
}

#[test]
fn converted_truetype_glyphs_match_the_source_font() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "fonts.create", json!({"id": "block", "height": 12}));
    let r = run(
        &mut s,
        "fonts.convertTrueType",
        json!({"font": "block", "from": "Lato", "chars": "OpenStitch \u{4E2D}"}),
    );
    assert_eq!(r["converted"], " OScehinpt", "sorted, distinct");
    assert_eq!(r["missing"], "\u{4E2D}");
    let mine = run(
        &mut s,
        "text.add",
        json!({"text": "Open Stitch", "font": "block", "size": 9, "x": 50, "y": 40}),
    )["id"]
        .as_u64()
        .expect("id");
    let lato = run(
        &mut s,
        "text.add",
        json!({"text": "Open Stitch", "font": "lato-bold", "size": 9, "x": 50, "y": 40}),
    )["id"]
        .as_u64()
        .expect("id");
    let (a, b) = (contours_of(&s, mine), contours_of(&s, lato));
    assert_eq!(a.len(), b.len());
    for (ca, cb) in a.iter().zip(&b) {
        assert_eq!(ca.points.len(), cb.points.len());
        for (p, q) in ca.points.iter().zip(&cb.points) {
            assert!(p.distance(*q) < 1e-6, "{p:?} vs {q:?}");
        }
    }
    // Outline glyphs sew with the text's region sewing.
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64() > Some(1000), "{stats}");
}

#[test]
fn shapes_become_glyphs_and_glyphs_become_shapes() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "fonts.create", json!({"id": "hand", "height": 10}));
    // An "I": a 2 x 10 mm bar standing on the baseline y = 60 with its left set line at x = 20.
    let bar = run(
        &mut s,
        "shape.rectangle",
        json!({"x": 21, "y": 50, "width": 2, "height": 10}),
    )["id"]
        .as_u64()
        .expect("id");
    let r = run(
        &mut s,
        "fonts.setGlyph",
        json!({"font": "hand", "char": "I", "ids": [bar], "x": 20, "y": 60, "advance": 4}),
    );
    assert_eq!(r["advanceMM"], 4.0);
    // An "o" with a hole, default width.
    let ring = run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 24, "cy": 57, "rx": 3, "ry": 3}),
    )["id"]
        .as_u64()
        .expect("id");
    let hole = run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 24, "cy": 57, "rx": 1.5, "ry": 1.5}),
    )["id"]
        .as_u64()
        .expect("id");
    run(
        &mut s,
        "shape.combine",
        json!({"ids": [ring, hole], "op": "difference"}),
    );
    let ring = s.design().objects.last().expect("ring").id;
    let r = run(
        &mut s,
        "fonts.setGlyph",
        json!({"font": "hand", "char": "o", "id": ring, "x": 20, "y": 60}),
    );
    assert!(
        (r["advanceMM"].as_f64().expect("adv") - 7.8).abs() < 0.05,
        "{r}"
    );

    let t = run(
        &mut s,
        "text.add",
        json!({"text": "IoI", "font": "hand", "size": 20, "x": 0, "y": 80, "align": "left"}),
    );
    assert_eq!(t["glyphs"], 3);
    let id = t["id"].as_u64().expect("id");
    let o = &s.design().objects[s.design().index_of(id).expect("o")];
    assert_eq!(o.kind, ObjectKind::Text);
    assert_eq!(o.glyph_groups().len(), 3);
    assert_eq!(o.glyph_groups()[1].len(), 2, "the o keeps its hole");
    let b = object_bounds(&s, id);
    // Size 20 doubles the drawn 10 mm font: I at 2..6 mm, o, I at (8 + 15.6) + 2 mm.
    assert!((b.min_x - 20.0).abs() < 0.5, "{b:?}");
    assert!(
        (b.min_y - 600.0).abs() < 0.5 && (b.max_y - 800.0).abs() < 0.5,
        "{b:?}"
    );
    assert!((b.max_x - (80.0 + 156.0 + 60.0)).abs() < 1.0, "{b:?}");

    // Edit the glyph as a shape, move a point, store it back.
    let e = run(
        &mut s,
        "fonts.editGlyph",
        json!({"font": "hand", "char": "I", "x": 50, "y": 90}),
    );
    assert_eq!(e["capLine"], 80.0);
    let shape = e["id"].as_u64().expect("id");
    assert_eq!(object_bounds(&s, shape).min_x, 510.0);
    run(
        &mut s,
        "object.setPoint",
        json!({"id": shape, "index": 2, "x": 56, "y": 90}),
    );
    run(
        &mut s,
        "fonts.setGlyph",
        json!({"font": "hand", "char": "I", "id": shape, "x": 50, "y": 90, "advance": 7}),
    );
    run(&mut s, "object.setText", json!({"id": id, "text": "I"}));
    let wider = object_bounds(&s, id);
    assert!(
        (wider.max_x - 120.0).abs() < 0.5,
        "edited point moved: {wider:?}"
    );
    // Stitch glyphs cannot be edited as shapes; unknown characters are reported.
    assert!(
        s.execute(
            "fonts.editGlyph",
            Some(&json!({"font": "hand", "char": "Q", "x": 0, "y": 0}))
        )
        .is_err()
    );
}

#[test]
fn metrics_and_files() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(&mut s, "fonts.create", json!({"id": "f", "height": 10}));
    run(
        &mut s,
        "fonts.convertTrueType",
        json!({"font": "f", "from": "pacifico", "chars": "Hello"}),
    );
    let one = run(&mut s, "text.add", json!({"text": "H\nH", "font": "f"}))["id"]
        .as_u64()
        .expect("id");
    let before = object_bounds(&s, one);
    run(
        &mut s,
        "fonts.setMetrics",
        json!({"font": "f", "lineGap": 5, "family": "Mine"}),
    );
    run(&mut s, "object.setText", json!({"id": one, "text": "H\nH"}));
    let after = object_bounds(&s, one);
    assert!((after.height() - before.height() - 50.0).abs() < 1e-6);
    run(&mut s, "object.setText", json!({"id": one, "text": "H"}));
    let single = object_bounds(&s, one).height();
    let r = run(
        &mut s,
        "fonts.setMetrics",
        json!({"font": "f", "height": 20}),
    );
    assert_eq!(r["heightMM"], 20.0);
    run(&mut s, "object.setText", json!({"id": one, "text": "H"}));
    // Drawn at 10 mm, standard height now 20 mm: size 10 halves it.
    assert!((object_bounds(&s, one).height() * 2.0 - single).abs() < 1e-6);

    run(
        &mut s,
        "fonts.save",
        json!({"font": "f", "path": "f.osfont"}),
    );
    let mut t = Session::new(&dir);
    run(&mut t, "fonts.open", json!({"path": "f.osfont"}));
    assert_eq!(t.design().fonts, s.design().fonts);
    assert_eq!(t.design().fonts[0].family, "Mine");
    let p = Compiler::default().compile(s.design());
    assert!(
        p.count(Command::Stitch) > 20,
        "{}",
        p.count(Command::Stitch)
    );
}

#[test]
fn font_creator_bad_input_fails_cleanly() {
    let dir = temp_dir();
    std::fs::write(
        dir.join("u.ttf"),
        include_bytes!("../../openstitch-design/fonts/Lato-Bold.ttf"),
    )
    .expect("font");
    let mut s = Session::new(&dir);
    run(&mut s, "fonts.create", json!({"id": "f", "height": 10}));
    run(
        &mut s,
        "fonts.loadTrueType",
        json!({"path": "u.ttf", "id": "tt"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 5, "height": 5}),
    );
    run(
        &mut s,
        "fonts.mapCharacter",
        json!({"font": "f", "char": "s", "object": 1}),
    );
    let stitches = run(&mut s, "design.generate", json!({}));
    assert!(stitches["stitches"].as_u64() > Some(0));
    let before = s.design().clone();
    let many: String = (0x4E00u32..0x4E00 + 1100)
        .filter_map(char::from_u32)
        .collect();
    let bad: Vec<(&str, Value)> = vec![
        (
            "fonts.setGlyph",
            json!({"font": "f", "char": "A", "ids": [99], "x": 0, "y": 0}),
        ),
        (
            "fonts.setGlyph",
            json!({"font": "f", "char": "AB", "ids": [1], "x": 0, "y": 0}),
        ),
        (
            "fonts.setGlyph",
            json!({"font": "f", "char": "A", "ids": [1]}),
        ),
        (
            "fonts.setGlyph",
            json!({"font": "f", "char": "A", "ids": [1], "x": 1e9, "y": 0}),
        ),
        (
            "fonts.setGlyph",
            json!({"font": "tt", "char": "A", "ids": [1], "x": 0, "y": 0}),
        ),
        (
            "fonts.setGlyph",
            json!({"font": "f", "char": "A", "ids": [1], "x": 0, "y": 0, "advance": -1}),
        ),
        (
            "fonts.editGlyph",
            json!({"font": "f", "char": "s", "x": 0, "y": 0}),
        ),
        (
            "fonts.editGlyph",
            json!({"font": "nope", "char": "A", "x": 0, "y": 0}),
        ),
        (
            "fonts.convertTrueType",
            json!({"font": "f", "from": "f", "chars": "A"}),
        ),
        (
            "fonts.convertTrueType",
            json!({"font": "f", "from": "nope", "chars": "A"}),
        ),
        (
            "fonts.convertTrueType",
            json!({"font": "f", "from": "tt", "chars": ""}),
        ),
        (
            "fonts.convertTrueType",
            json!({"font": "f", "from": "tt", "chars": "\u{4E2D}"}),
        ),
        (
            "fonts.convertTrueType",
            json!({"font": "f", "from": "tt", "chars": many}),
        ),
        (
            "fonts.convertTrueType",
            json!({"font": "tt", "from": "tt", "chars": "A"}),
        ),
        ("fonts.setMetrics", json!({"font": "f", "height": 0})),
        ("fonts.setMetrics", json!({"font": "f", "lineGap": -1})),
        ("fonts.setMetrics", json!({"font": "f", "family": ""})),
        ("fonts.setMetrics", json!({"font": "tt", "height": 5})),
    ];
    for (id, params) in bad {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
}
