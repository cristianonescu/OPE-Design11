//! Compatibility tests of docs/COMPATIBILITY_AUDIT.md items 1 (satin is a zigzag across the
//! column, not a square wave) and 7 (Auto direction follows the shape). Internal regression
//! tier: the expectations come from the audit, not from PE-DESIGN reference output.

use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Needle points (mm) of object `id` after converting it to stitches: stitch entries only.
fn needle_points(s: &mut Session, id: u64) -> Vec<(f64, f64)> {
    run(s, "object.toStitches", json!({"ids": [id]}));
    run(s, "stitches.points", json!({"id": id}))["points"]
        .as_array()
        .expect("points")
        .iter()
        .filter(|p| p["command"] == "stitch")
        .map(|p| (p["x"].as_f64().expect("x"), p["y"].as_f64().expect("y")))
        .collect()
}

fn session() -> Session {
    let mut s = Session::new(std::env::temp_dir());
    run(&mut s, "design.new", json!({"width": 100, "height": 100}));
    s
}

/// The audit's satin assertions for a 5 × 2 mm column from (10, 10) to (15, 12), stitches
/// across its 2 mm height: consecutive needle points alternate between the top and bottom
/// edge, so no stitch (let alone one shorter than 0.5 mm) lies along an edge, and the stitch
/// count is 15 ± 1 at density 3.
fn assert_zigzag_column(pts: &[(f64, f64)], what: &str) {
    let edge = |&(x, y): &(f64, f64)| {
        assert!((9.9..=15.1).contains(&x), "{what}: {x} outside the column");
        if (y - 10.0).abs() < 0.05 {
            0
        } else if (y - 12.0).abs() < 0.05 {
            1
        } else {
            panic!("{what}: needle point {x}, {y} is not on an edge")
        }
    };
    for w in pts.windows(2) {
        assert_ne!(edge(&w[0]), edge(&w[1]), "{what}: {w:?} runs along an edge");
        let along = (w[1].0 - w[0].0).abs();
        assert!(along < 0.5, "{what}: {w:?} slants {along} mm");
    }
    let stitches = pts.len() - 1;
    assert!(
        (14..=16).contains(&stitches),
        "{what}: {stitches} stitches, expected 15 ± 1"
    );
}

#[test]
fn region_satin_sews_a_zigzag_across_the_column() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 5, "height": 2}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "angle": 90, "density": 3}),
    );
    let pts = needle_points(&mut s, 1);
    assert_zigzag_column(&pts, "region satin");
}

#[test]
fn manual_punch_column_sews_a_zigzag() {
    let mut s = session();
    run(
        &mut s,
        "shape.column",
        json!({"lines": [[[10, 10], [10, 12]], [[15, 10], [15, 12]]]}),
    );
    let pts = needle_points(&mut s, 1);
    assert_zigzag_column(&pts, "shape.column");
}

#[test]
fn skeleton_satin_sews_a_zigzag() {
    let mut s = session();
    run(
        &mut s,
        "shape.skeletonSatin",
        json!({"points": [[10, 11], [15, 11]], "width": 2}),
    );
    let pts = needle_points(&mut s, 1);
    assert_zigzag_column(&pts, "shape.skeletonSatin");
}

#[test]
fn pull_compensation_widens_the_zigzag() {
    let mut s = session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 5, "height": 2}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "angle": 90, "density": 3, "pullCompensation": 0.2}),
    );
    let pts = needle_points(&mut s, 1);
    for w in pts.windows(2) {
        let (a, b) = (w[0].1.min(w[1].1), w[0].1.max(w[1].1));
        assert!((a - 9.8).abs() < 0.02 && (b - 12.2).abs() < 0.02, "{w:?}");
    }
}

/// A 90° bent band 5 mm wide: along y = 0…5 from x = 0 to 30, then up x = 25…30 to y = 30.
/// With Auto direction every stitch near a station on the band's middle line is within 10°
/// of perpendicular to the band there.
#[test]
fn auto_direction_satin_follows_a_bent_band() {
    let mut s = session();
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[0, 0], [30, 0], [30, 30], [25, 30], [25, 5], [0, 5]]}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "autoDirection": true}),
    );
    let pts = needle_points(&mut s, 1);
    // Station on the middle line and the band's axis direction there.
    let stations = [
        ((5.0, 2.5), (1.0, 0.0)),
        ((12.0, 2.5), (1.0, 0.0)),
        ((19.0, 2.5), (1.0, 0.0)),
        ((27.5, 13.0), (0.0, 1.0)),
        ((27.5, 22.0), (0.0, 1.0)),
    ];
    for ((sx, sy), (ax, ay)) in stations {
        let mut seen = 0;
        for w in pts.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            let (dx, dy) = (x1 - x0, y1 - y0);
            let len2 = dx * dx + dy * dy;
            if len2 < 1.0 {
                continue;
            }
            // Stitches passing within 0.3 mm of the station.
            let t = (((sx - x0) * dx + (sy - y0) * dy) / len2).clamp(0.0, 1.0);
            let (px, py) = (x0 + dx * t, y0 + dy * t);
            if ((px - sx).powi(2) + (py - sy).powi(2)).sqrt() > 0.3 {
                continue;
            }
            seen += 1;
            let cos = ((dx * ax + dy * ay) / len2.sqrt()).abs();
            let off = 90.0 - cos.acos().to_degrees();
            assert!(
                off <= 10.0,
                "at ({sx}, {sy}) a stitch is {off:.1}° off perpendicular"
            );
        }
        assert!(seen > 0, "no stitch crosses the band at ({sx}, {sy})");
    }
    // Rows stay across the band: no stitch longer than the band is wide (plus the slant).
    for w in pts.windows(2) {
        let d = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        assert!(d < 7.5, "stitch of {d} mm in a 5 mm band");
    }
}
