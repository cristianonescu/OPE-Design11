//! Image conversion commands: placement, undo, sandboxing and hostile input.
#![cfg(feature = "digitize")]

use openstitch_digitize::Raster;
use openstitch_engine::Session;
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-digitize-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// 60×40 px: a red block, a thin blue bar and a black outline square on white.
fn write_sample(dir: &std::path::Path) {
    let img = Raster::from_fn(60, 40, |x, y| {
        let inside = |x0, y0, x1, y1| x >= x0 && x < x1 && y >= y0 && y < y1;
        if inside(5, 5, 25, 35) {
            [220, 20, 30, 255]
        } else if inside(35, 10, 55, 14) {
            [20, 40, 200, 255]
        } else if inside(35, 18, 55, 36) && !inside(38, 21, 52, 33) {
            [0, 0, 0, 255]
        } else {
            [255, 255, 255, 255]
        }
    })
    .expect("size");
    std::fs::write(dir.join("sample.png"), img.encode_png().expect("png")).expect("write");
}

#[test]
fn conversions_add_objects_in_one_undo_step() {
    let dir = temp_dir();
    write_sample(&dir);
    let mut s = Session::new(&dir);
    run(&mut s, "design.new", json!({"width": 100, "height": 100}));
    let r = run(
        &mut s,
        "digitize.autoPunch",
        json!({"path": "sample.png", "colors": 4, "width": 60, "x": 20, "y": 30}),
    );
    let ids = r["ids"].as_array().expect("ids");
    assert_eq!(ids.len(), 3, "{r}");
    assert_eq!(r["regions"], json!(3));
    let list = run(&mut s, "object.list", json!({}));
    let b = &list[0]["bounds"];
    assert!(b["x"].as_f64().expect("x") >= 20.0 && b["y"].as_f64().expect("y") >= 30.0);
    run(&mut s, "history.undo", json!({}));
    assert!(
        s.design().objects.is_empty(),
        "one undo removes the whole conversion"
    );

    let r = run(
        &mut s,
        "digitize.crossStitch",
        json!({"path": "sample.png", "cellSize": 2, "width": 60}),
    );
    assert_eq!(r["columns"], json!(30));
    assert_eq!(r["rows"], json!(20));
    assert!(r["ids"].as_array().expect("ids").len() >= 3);

    let r = run(
        &mut s,
        "digitize.photoStitch",
        json!({"path": "sample.png", "mode": "mono", "style": "lines", "width": 30}),
    );
    assert_eq!(r["ids"].as_array().expect("ids").len(), 1);
    let r = run(
        &mut s,
        "digitize.photoStitch",
        json!({"path": "sample.png", "colors": 4}),
    );
    assert!(r["ids"].as_array().expect("ids").len() >= 2);
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64().expect("count") > 1000, "{stats}");
}

#[test]
fn design_center_stages() {
    let dir = temp_dir();
    write_sample(&dir);
    let mut s = Session::new(&dir);
    let r = run(
        &mut s,
        "digitize.lineImage",
        json!({"path": "sample.png", "out": "lines.png"}),
    );
    assert_eq!(r["width"], json!(60));
    assert!(r["linePixels"].as_u64().expect("count") > 100);
    let line_png = std::fs::read(dir.join("lines.png")).expect("written");
    assert_eq!(Raster::decode(&line_png).expect("png").width(), 60);

    let fig = run(
        &mut s,
        "digitize.figure",
        json!({"path": "sample.png", "width": 60, "x": 0, "y": 0}),
    );
    assert!(!fig["lines"].as_array().expect("lines").is_empty());
    assert!(s.design().objects.is_empty(), "figure is read-only");

    let r = run(
        &mut s,
        "digitize.designCenter",
        json!({"path": "sample.png", "width": 60, "lineSew": "triple", "lineColor": "Red"}),
    );
    let ids = r["ids"].as_array().expect("ids");
    assert!(!ids.is_empty());
    let last = s.design().objects.last().expect("lines object");
    assert_eq!(
        last.line.as_ref().map(|l| l.thread.name.as_deref()),
        Some(Some("Red"))
    );
}

#[test]
fn sandbox_confines_image_reads_and_writes() {
    let root = temp_dir();
    write_sample(&root);
    let outside = temp_dir();
    write_sample(&outside);
    let mut s = Session::sandboxed(&root);
    assert!(
        s.execute("digitize.autoPunch", Some(&json!({"path": "sample.png"})))
            .is_ok()
    );
    let escape = outside.join("sample.png").display().to_string();
    assert!(
        s.execute("digitize.autoPunch", Some(&json!({"path": escape})))
            .is_err()
    );
    assert!(
        s.execute(
            "digitize.autoPunch",
            Some(&json!({"path": "../sample.png"}))
        )
        .is_err()
    );
    let out = outside.join("x.png").display().to_string();
    assert!(
        s.execute(
            "digitize.lineImage",
            Some(&json!({"path": "sample.png", "out": out}))
        )
        .is_err()
    );
}

#[test]
fn bad_digitize_parameters_fail_cleanly() {
    let dir = temp_dir();
    write_sample(&dir);
    std::fs::write(dir.join("corrupt.png"), b"\x89PNG\r\n\x1a\n garbage").expect("write");
    std::fs::write(dir.join("empty.png"), b"").expect("write");
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("digitize.autoPunch", json!({})),
        ("digitize.autoPunch", json!({"path": "missing.png"})),
        ("digitize.autoPunch", json!({"path": "corrupt.png"})),
        ("digitize.autoPunch", json!({"path": "empty.png"})),
        (
            "digitize.autoPunch",
            // 0 is the automatic count; 65 is out of range.
            json!({"path": "sample.png", "colors": 65}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "colors": 65}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "colors": 2.5}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "resolution": 100000}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "width": 0}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "width": 1e300}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "simplify": -1}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "curves": "yes"}),
        ),
        (
            "digitize.autoPunch",
            json!({"path": "sample.png", "x": "left"}),
        ),
        (
            "digitize.crossStitch",
            json!({"path": "sample.png", "cellSize": 0.1}),
        ),
        (
            "digitize.crossStitch",
            json!({"path": "sample.png", "type": "triple"}),
        ),
        (
            "digitize.crossStitch",
            json!({"path": "sample.png", "cellSize": 1, "width": 2000}),
        ),
        (
            "digitize.photoStitch",
            json!({"path": "sample.png", "style": "oil"}),
        ),
        (
            "digitize.photoStitch",
            json!({"path": "sample.png", "mode": "mono", "thread": "#zzzzzz"}),
        ),
        (
            "digitize.photoStitch",
            json!({"path": "sample.png", "rowSpacing": 0}),
        ),
        (
            "digitize.photoStitch",
            json!({"path": "sample.png", "rowSpacing": 0.2, "stitchLength": 0.3, "width": 2000}),
        ),
        (
            "digitize.designCenter",
            json!({"path": "sample.png", "lineSew": "chain"}),
        ),
        (
            "digitize.designCenter",
            json!({"path": "sample.png", "threshold": 300}),
        ),
        (
            "digitize.designCenter",
            json!({"path": "sample.png", "threshold": 0}),
        ),
        (
            "digitize.designCenter",
            json!({"path": "sample.png", "lineWidth": 50}),
        ),
        (
            "digitize.designCenter",
            json!({"path": "sample.png", "lineSew": "none", "regionSew": "none"}),
        ),
        ("digitize.figure", json!({"path": 7})),
        (
            "digitize.lineImage",
            json!({"path": "sample.png", "out": 3}),
        ),
    ];
    for (id, params) in bad {
        let r = s.execute(id, Some(&params));
        assert!(r.is_err(), "{id} {params} should fail, got {r:?}");
        assert_eq!(s.design(), &before, "{id} {params} changed the design");
    }
    assert!(s.can_undo());
}
