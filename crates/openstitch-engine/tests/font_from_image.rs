//! Font Creator: characters from images in one step (`fonts.traceImage`, `fonts.fromImage`),
//! on small images drawn by the tests.
#![cfg(feature = "digitize")]

use openstitch_design::Rect;
use openstitch_digitize::Raster;
use openstitch_engine::Session;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-fontimage-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

type Shape = fn(u32, u32) -> bool;

/// Writes a `w`×`h` PNG: ink where any shape holds, on `ground`.
fn write(dir: &Path, name: &str, w: u32, h: u32, ink: [u8; 4], ground: [u8; 4], shapes: &[Shape]) {
    let img = Raster::from_fn(w, h, |x, y| {
        if shapes.iter().any(|f| f(x, y)) {
            ink
        } else {
            ground
        }
    })
    .expect("raster");
    std::fs::write(dir.join(name), img.encode_png().expect("png")).expect("write");
}

fn inside(x: u32, y: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> bool {
    x >= x0 && x < x1 && y >= y0 && y < y1
}

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

/// Two rows: "I" (a bar), "O" (a ring), "i" (stem and dot), a speck of noise; then two
/// squares and a bar hanging below their baseline.
fn alphabet(dir: &Path) {
    write(
        dir,
        "alphabet.png",
        240,
        120,
        BLACK,
        WHITE,
        &[
            |x, y| inside(x, y, 10, 10, 20, 50),
            |x, y| inside(x, y, 40, 10, 80, 50) && !inside(x, y, 50, 20, 70, 40),
            |x, y| inside(x, y, 100, 22, 108, 50) || inside(x, y, 100, 12, 108, 18),
            |x, y| x == 200 && y == 5,
            |x, y| inside(x, y, 10, 70, 30, 100),
            |x, y| inside(x, y, 40, 70, 60, 100),
            |x, y| inside(x, y, 70, 75, 80, 115),
        ],
    );
}

fn glyph_count(s: &mut Session, font: &str) -> u64 {
    let list = run(s, "fonts.list", json!({}));
    list.as_array()
        .expect("rows")
        .iter()
        .find(|f| f["id"] == font)
        .map_or(0, |f| f["glyphs"].as_u64().unwrap_or(0))
}

/// The character's outline put on the page with its set line and baseline at the origin.
fn glyph_bounds(s: &mut Session, font: &str, ch: &str) -> Rect {
    let id = run(
        s,
        "fonts.editGlyph",
        json!({"font": font, "char": ch, "x": 0, "y": 0}),
    )["id"]
        .as_u64()
        .expect("id");
    let d = s.design();
    let b = d.objects[d.index_of(id).expect("object")]
        .bounds()
        .expect("bounds");
    run(s, "history.undo", json!({}));
    b
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn an_alphabet_image_splits_into_characters_in_reading_order() {
    let dir = temp_dir();
    alphabet(&dir);
    let mut s = Session::new(dir);
    run(&mut s, "fonts.create", json!({"id": "drawn", "height": 10}));
    let t = run(
        &mut s,
        "fonts.traceImage",
        json!({"path": "alphabet.png", "font": "drawn", "chars": "IOi", "outlines": true}),
    );
    assert_eq!(t["rows"], 2, "{t}");
    assert_eq!(t["inverted"], false);
    let found = t["characters"].as_array().expect("characters");
    assert_eq!(found.len(), 6, "the speck is ignored: {t}");
    let rows: Vec<u64> = found
        .iter()
        .map(|c| c["row"].as_u64().expect("row"))
        .collect();
    assert_eq!(rows, [0, 0, 0, 1, 1, 1]);
    assert_eq!(found[1]["contours"], 2, "the ring keeps its hole");
    assert_eq!(found[2]["components"], 2, "the dot stays with its stem");
    assert_eq!(found[0]["char"], "I");
    assert!(found[3]["char"].is_null());
    assert!(near(
        found[0]["heightMM"].as_f64().expect("height"),
        10.0,
        0.4
    ));
    assert!(found[1]["outline"].as_array().is_some_and(|o| o.len() == 2));
    assert_eq!(glyph_count(&mut s, "drawn"), 0, "a preview changes nothing");

    let r = run(
        &mut s,
        "fonts.fromImage",
        json!({"path": "alphabet.png", "font": "drawn", "chars": ["I", "O", "i", "", "b", "g"]}),
    );
    assert_eq!(r["created"], "IOibg", "{r}");
    assert_eq!(r["found"], 6);
    assert_eq!(r["skipped"], 1);
    assert_eq!(r["sewing"], "region");
    assert_eq!(glyph_count(&mut s, "drawn"), 5);

    // The tallest character spans the standard height (10 mm = 100 units, 2.5 per pixel),
    // standing on the baseline right of the left set line.
    let i = glyph_bounds(&mut s, "drawn", "I");
    assert!(
        near(i.min_y, -100.0, 3.0) && near(i.max_y, 0.0, 3.0),
        "{i:?}"
    );
    assert!(
        near(i.min_x, 4.0, 3.0) && near(i.width(), 25.0, 3.0),
        "{i:?}"
    );
    // The dotted character stands on the same baseline; the hanging bar goes below it.
    let dotted = glyph_bounds(&mut s, "drawn", "i");
    assert!(near(dotted.min_y, -95.0, 3.0) && near(dotted.max_y, 0.0, 3.0));
    let g = glyph_bounds(&mut s, "drawn", "g");
    assert!(
        near(g.max_y, 37.5, 3.0) && near(g.min_y, -62.5, 3.0),
        "{g:?}"
    );
    let b = glyph_bounds(&mut s, "drawn", "b");
    assert!(near(b.height(), 75.0, 3.0) && near(b.max_y, 0.0, 3.0));

    // The new characters letter text at once.
    run(
        &mut s,
        "text.add",
        json!({"text": "IOi bg", "font": "drawn", "size": 10, "x": 50, "y": 50}),
    );
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64() > Some(100), "{stats}");

    // One undo step removes the text, a second all the traced characters.
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));
    assert_eq!(glyph_count(&mut s, "drawn"), 0);
    run(&mut s, "history.redo", json!({}));
    assert_eq!(glyph_count(&mut s, "drawn"), 5);
}

#[test]
fn the_whole_background_image_becomes_one_character_where_it_is_placed() {
    let dir = temp_dir();
    write(
        &dir,
        "square.png",
        100,
        100,
        BLACK,
        WHITE,
        &[
            |x, y| inside(x, y, 20, 20, 80, 80) && !inside(x, y, 40, 40, 60, 60),
            |x, y| inside(x, y, 45, 85, 55, 95),
        ],
    );
    let mut s = Session::new(dir);
    run(&mut s, "fonts.create", json!({"id": "frame", "height": 12}));
    // 0.5 mm per pixel: the square's ink spans 20-50 mm across and 30-60 mm down.
    run(
        &mut s,
        "background.set",
        json!({"path": "square.png", "width": 50, "x": 10, "y": 20}),
    );
    // Aligned on the page: left set line at 20 mm, baseline at 60 mm; the image is not scaled,
    // so the dot below the square hangs below the baseline.
    let r = run(
        &mut s,
        "fonts.fromImage",
        json!({"font": "frame", "chars": "H", "mode": "whole", "x": 20, "y": 60}),
    );
    assert_eq!(r["created"], "H", "{r}");
    assert_eq!(r["found"], 1);
    let h = glyph_bounds(&mut s, "frame", "H");
    assert!(
        near(h.min_x, 0.0, 3.0) && near(h.max_x, 300.0, 3.0),
        "{h:?}"
    );
    assert!(
        near(h.min_y, -300.0, 3.0) && near(h.max_y, 75.0, 3.0),
        "{h:?}"
    );

    // Fitted instead: the ink's bottom on the baseline, its height the standard height.
    run(
        &mut s,
        "fonts.fromImage",
        json!({"font": "frame", "chars": "K", "mode": "whole"}),
    );
    let k = glyph_bounds(&mut s, "frame", "K");
    assert!(
        near(k.height(), 120.0, 3.0) && near(k.max_y, 0.0, 3.0),
        "{k:?}"
    );
    let t = run(
        &mut s,
        "fonts.traceImage",
        json!({"mode": "whole", "font": "frame"}),
    );
    assert_eq!(t["characters"][0]["components"], 2);
    assert_eq!(t["characters"][0]["contours"], 3);
}

#[test]
fn light_lettering_on_a_dark_ground_is_found() {
    let dir = temp_dir();
    write(
        &dir,
        "dark.png",
        80,
        40,
        WHITE,
        BLACK,
        &[
            |x, y| inside(x, y, 10, 10, 20, 30),
            |x, y| inside(x, y, 40, 10, 60, 30),
        ],
    );
    let mut s = Session::new(dir);
    run(&mut s, "fonts.create", json!({"id": "neg", "height": 10}));
    let t = run(&mut s, "fonts.traceImage", json!({"path": "dark.png"}));
    assert_eq!(t["inverted"], true, "{t}");
    assert_eq!(t["characters"].as_array().map(Vec::len), Some(2));
    let forced = run(
        &mut s,
        "fonts.traceImage",
        json!({"path": "dark.png", "invert": false}),
    );
    assert_eq!(forced["characters"].as_array().map(Vec::len), Some(1));
    let r = run(
        &mut s,
        "fonts.fromImage",
        json!({"path": "dark.png", "font": "neg", "chars": "lm"}),
    );
    assert_eq!(r["created"], "lm");
}

#[test]
fn bad_image_requests_fail_cleanly_and_change_nothing() {
    let dir = temp_dir();
    alphabet(&dir);
    write(&dir, "blank.png", 50, 50, BLACK, WHITE, &[]);
    std::fs::write(dir.join("junk.png"), b"not an image").expect("write");
    let mut s = Session::new(dir);
    run(&mut s, "fonts.create", json!({"id": "f", "height": 10}));
    let bad = [
        json!({"font": "f", "chars": "A"}),
        json!({"path": "blank.png", "font": "f", "chars": "A"}),
        json!({"path": "junk.png", "font": "f", "chars": "A"}),
        json!({"path": "../outside.png", "font": "f", "chars": "A"}),
        json!({"path": "alphabet.png", "font": "nope", "chars": "A"}),
        json!({"path": "alphabet.png", "font": "f", "chars": "ABCDEFG"}),
        json!({"path": "alphabet.png", "font": "f", "chars": "AA"}),
        json!({"path": "alphabet.png", "font": "f", "chars": "   "}),
        json!({"path": "alphabet.png", "font": "f", "chars": ["AB"]}),
        json!({"path": "alphabet.png", "font": "f", "chars": [1]}),
        json!({"path": "alphabet.png", "font": "f", "chars": 5}),
        json!({"path": "alphabet.png", "font": "f", "chars": "A", "mode": "rows"}),
        json!({"path": "alphabet.png", "font": "f", "chars": "A", "threshold": 0}),
        json!({"path": "alphabet.png", "font": "f", "chars": "A", "resolution": 10}),
        json!({"path": "alphabet.png", "font": "f", "chars": "A", "x": 0, "y": 0}),
        json!({"path": "alphabet.png", "font": "f", "chars": "A", "mode": "whole", "x": 0, "y": 0}),
        json!({"path": "alphabet.png", "font": "f", "chars": "A", "capPixels": 0}),
        json!({"path": "alphabet.png", "font": "f", "chars": "A", "minArea": 100000}),
    ];
    for params in bad {
        assert!(
            s.execute("fonts.fromImage", Some(&params)).is_err(),
            "accepted {params}"
        );
    }
    assert!(
        s.execute("fonts.traceImage", Some(&json!({"path": "blank.png"})))
            .is_err()
    );
    assert_eq!(glyph_count(&mut s, "f"), 0);
}
