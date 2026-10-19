//! Shaping, kerning, vertical text and per-character edits through the engine.

use openstitch_design::Rect;
use openstitch_design::text::{self, TextDirection, TextSpec};
use openstitch_engine::{Session, project};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-shaping-{n}-{:?}",
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

fn spec(s: &Session, id: u64) -> TextSpec {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")]
        .text
        .clone()
        .expect("text")
}

#[test]
fn kerning_and_direction_are_text_options() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "text.add", json!({"text": "AVAVAV", "size": 20}));
    let plain = bounds(&s, 1);
    run(&mut s, "object.setText", json!({"id": 1, "kerning": true}));
    let kerned = bounds(&s, 1);
    assert!(
        kerned.width() < plain.width() - 20.0,
        "{kerned:?} vs {plain:?}"
    );
    let info = run(&mut s, "object.get", json!({"id": 1}));
    assert_eq!(info["text"]["kerning"], true);
    let listed = run(&mut s, "object.list", json!({}));
    assert_eq!(listed[0]["kerning"], true, "{listed}");

    run(
        &mut s,
        "object.setText",
        json!({"id": 1, "kerning": false, "direction": "vertical"}),
    );
    let tall = bounds(&s, 1);
    assert!(tall.height() > 4.0 * tall.width(), "{tall:?}");
    assert_eq!(spec(&s, 1).direction, TextDirection::Vertical);
    run(&mut s, "history.undo", json!({}));
    assert_eq!(spec(&s, 1).direction, TextDirection::Horizontal);
    assert!(spec(&s, 1).kerning);

    for bad in [
        json!({"id": 1, "direction": "diagonal"}),
        json!({"id": 1, "kerning": 1}),
    ] {
        assert!(s.execute("object.setText", Some(&bad)).is_err(), "{bad}");
    }
}

#[test]
fn single_characters_are_edited_saved_and_reset() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(&mut s, "text.add", json!({"text": "HIH", "size": 10}));
    let before = bounds(&s, 1);
    let r = run(
        &mut s,
        "text.setCharacter",
        json!({"id": 1, "index": 1, "y": -5, "rotation": 30, "size": 15}),
    );
    assert_eq!(r["characters"], 1, "{r}");
    let after = bounds(&s, 1);
    assert!(after.min_y < before.min_y - 50.0, "{after:?}");
    assert!(
        after.width() > before.width(),
        "a bigger I pushes the last H"
    );
    let e = spec(&s, 1).char_edit(1).cloned().expect("edit");
    assert_eq!((e.y, e.rotation, e.size), (-5.0, 30.0, Some(15.0)));

    // Unnamed values are kept; null size returns to the text's size.
    run(
        &mut s,
        "text.setCharacter",
        json!({"id": 1, "index": 1, "size": null}),
    );
    let e = spec(&s, 1).char_edit(1).cloned().expect("edit");
    assert_eq!((e.y, e.rotation, e.size), (-5.0, 30.0, None));

    // Saved in the .osd and read back.
    run(&mut s, "project.save", json!({"path": "c.osd"}));
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "c.osd"}));
    assert_eq!(t.design(), s.design());
    let doc: Value =
        serde_json::from_slice(&project::encode(s.design()).expect("encode")).expect("json");
    assert_eq!(
        doc["design"]["objects"][0]["text"]["characters"],
        json!([{"index": 1, "y": -5.0, "rotation": 30.0}])
    );

    // Reset, then shortening the text drops edits past its end.
    run(
        &mut s,
        "text.setCharacter",
        json!({"id": 1, "index": 0, "count": 3, "reset": true}),
    );
    assert!(spec(&s, 1).characters.is_empty());
    let b = bounds(&s, 1);
    assert!(
        (b.width() - before.width()).abs() < 1e-6 && (b.height() - before.height()).abs() < 1e-6
    );
    run(
        &mut s,
        "text.setCharacter",
        json!({"id": 1, "index": 2, "x": 1}),
    );
    run(&mut s, "object.setText", json!({"id": 1, "text": "HI"}));
    assert!(spec(&s, 1).characters.is_empty());

    for bad in [
        json!({"id": 1, "index": 2}),
        json!({"id": 1, "index": 0, "count": 3}),
        json!({"id": 1, "index": 0, "rotation": 400}),
        json!({"id": 1, "index": 0, "size": 0}),
        json!({"id": 9, "index": 0}),
    ] {
        assert!(s.execute("text.setCharacter", Some(&bad)).is_err(), "{bad}");
    }
}

#[test]
fn name_drop_resets_character_edits() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "text.add", json!({"text": "Name"}));
    run(
        &mut s,
        "text.setCharacter",
        json!({"id": 1, "index": 0, "size": 14}),
    );
    run(&mut s, "text.setNameDrop", json!({"ids": [1]}));
    assert!(spec(&s, 1).characters.is_empty());
}

/// Hebrew and Arabic through a system font, when the machine has one with both (fonts with
/// these scripts are not bundled).
#[test]
fn right_to_left_scripts_with_a_system_font() {
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ];
    let Some(path) = candidates.iter().map(Path::new).find(|p| p.exists()) else {
        eprintln!("no system font with Hebrew and Arabic; skipped");
        return;
    };
    let mut s = Session::new(temp_dir());
    if s.execute(
        "fonts.loadTrueType",
        Some(&json!({ "path": path.display().to_string(), "id": "rtl" })),
    )
    .is_err()
    {
        eprintln!("{} is not loadable here; skipped", path.display());
        return;
    }
    let fonts = s.design().fonts.clone();
    let lay =
        |t: &str| text::layout_in(&TextSpec::new(t, "rtl", 10.0), None, &fonts).expect("layout");
    let centre = |l: &text::Layout, index: usize| {
        let g = l.glyphs.iter().find(|g| g.index == index).expect("glyph");
        Rect::enclosing(g.contours.iter().flat_map(|c| c.outline_points()))
            .expect("bounds")
            .center()
            .x
    };

    // Hebrew runs right to left; a Latin word before it stays on the left.
    let l = lay("ab \u{05E9}\u{05DC}\u{05D5}\u{05DD}");
    assert!(
        l.missing.is_empty() && l.shaping.is_empty(),
        "{:?} {:?}",
        l.missing,
        l.shaping
    );
    assert!(centre(&l, 0) < centre(&l, 1), "Latin left to right");
    assert!(
        centre(&l, 3) > centre(&l, 4) && centre(&l, 4) > centre(&l, 6),
        "Hebrew right to left"
    );
    assert!(
        centre(&l, 1) < centre(&l, 6),
        "Latin before (left of) the Hebrew"
    );

    // Arabic letters join: a medial beh is not the isolated beh, and the word is right to left.
    let isolated = lay("\u{0628}");
    let word = lay("\u{0628}\u{0628}\u{0628}");
    assert!(word.missing.is_empty(), "{:?}", word.missing);
    assert_eq!(word.outlined().count(), 3);
    let middle = word.glyphs.iter().find(|g| g.index == 1).expect("medial");
    let shift = |c: &openstitch_design::Contour| {
        let b = Rect::enclosing(c.outline_points()).expect("b");
        (b.width().round(), b.height().round())
    };
    assert_ne!(
        middle.contours.iter().map(shift).collect::<Vec<_>>(),
        isolated.glyphs[0]
            .contours
            .iter()
            .map(shift)
            .collect::<Vec<_>>(),
        "contextual form"
    );
    assert!(centre(&word, 0) > centre(&word, 2));

    // Through the engine: added, sewn, no warnings.
    let r = run(
        &mut s,
        "text.add",
        json!({"text": "\u{0633}\u{0644}\u{0627}\u{0645} \u{05E9}\u{05DC}\u{05D5}\u{05DD}", "font": "rtl", "size": 15}),
    );
    assert_eq!(r["missing"], json!([]), "{r}");
    assert_eq!(r["shaping"], json!([]), "{r}");
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64() > Some(200), "{stats}");
}
