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

/// The satin's body: without PE-DESIGN's tie-in (the first needle point, 0.3 mm along the
/// first stitch) and end lock (the last two: 0.3 mm back and return), as the satin-zigzag-*
/// specs measure it.
fn body(pts: &[(f64, f64)]) -> &[(f64, f64)] {
    &pts[1..pts.len() - 2]
}

/// The audit's satin assertions for a 5 × 2 mm column from (10, 10) to (15, 12), stitches
/// across its 2 mm height: consecutive needle points alternate between the top and bottom
/// edge, so no stitch lies along an edge but `hops` short ones (PE-DESIGN reaches a last row
/// closer than one row's pitch along the edge), none longer than 0.5 mm; each stitch moves at
/// most `slant` mm along the column (one row: 1/3 mm for a zigzag, 2/3 mm for PE-DESIGN's
/// sawtooth at density 3), and there are `count` ± 1 stitches.
fn assert_zigzag_column(pts: &[(f64, f64)], slant: f64, hops: usize, count: usize, what: &str) {
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
    let mut along_edge = 0;
    for w in pts.windows(2) {
        let along = (w[1].0 - w[0].0).abs();
        if edge(&w[0]) == edge(&w[1]) {
            along_edge += 1;
            assert!(along <= 0.5, "{what}: {w:?} runs {along} mm along an edge");
        } else {
            assert!(along <= slant + 0.06, "{what}: {w:?} slants {along} mm");
        }
    }
    assert_eq!(along_edge, hops, "{what}: stitches along an edge");
    let stitches = pts.len() - 1;
    assert!(
        stitches.abs_diff(count) <= 1,
        "{what}: {stitches} stitches, expected {count} ± 1"
    );
}

/// The audit's column is sewn at density 3 without under sewing (the satin defaults are
/// PE-DESIGN's 4.5 lines/mm with under sewing on), as in the satin-zigzag-* specs.
fn top_satin_at_density_3(s: &mut Session) {
    run(
        s,
        "object.setRegion",
        json!({"id": 1, "density": 3, "underlay": false}),
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
        json!({"id": 1, "type": "satin", "angle": 90, "density": 3, "underlay": false}),
    );
    // End to end (no start point; the Rectangle tool's default splits the column there).
    run(
        &mut s,
        "object.setEntryExit",
        json!({"id": 1, "regionEntry": null, "regionExit": null}),
    );
    let pts = needle_points(&mut s, 1);
    // From the first corner: the opening row across and back, a straight travel inside to the
    // far end's other corner, then the sawtooth back to the start point: the far end's row,
    // the single line next to it (1/3 mm), every other line after that (2/3 mm apart) and the
    // start point's row, as PE-DESIGN sews a column started at one end (satin-length-sweep-*,
    // #122). The travel is left out, as the tie-in and lock are.
    let far = pts
        .iter()
        .position(|&(x, _)| x >= 14.95)
        .expect("reaches the far end");
    assert_zigzag_column(&body(&pts)[far - 1..], 2.0 / 3.0, 0, 17, "region satin");
}

#[test]
fn manual_punch_column_sews_a_zigzag() {
    let mut s = session();
    run(
        &mut s,
        "shape.column",
        json!({"lines": [[[10, 10], [10, 12]], [[15, 10], [15, 12]]]}),
    );
    top_satin_at_density_3(&mut s);
    let pts = needle_points(&mut s, 1);
    // PE-DESIGN's Manual Punch sawtooth: 8 rungs 5/7 mm apart, 15 crossings.
    assert_zigzag_column(body(&pts), 5.0 / 7.0, 0, 15, "shape.column");
}

#[test]
fn skeleton_satin_sews_a_zigzag() {
    let mut s = session();
    run(
        &mut s,
        "shape.skeletonSatin",
        json!({"points": [[10, 11], [15, 11]], "width": 2}),
    );
    top_satin_at_density_3(&mut s);
    let pts = needle_points(&mut s, 1);
    assert_zigzag_column(body(&pts), 1.0 / 3.0, 0, 15, "shape.skeletonSatin");
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
        json!({"id": 1, "type": "satin", "angle": 90, "density": 3, "pullCompensation": 0.2, "underlay": false}),
    );
    // End to end (no start point; the Rectangle tool's default splits the column there).
    run(
        &mut s,
        "object.setEntryExit",
        json!({"id": 1, "regionEntry": null, "regionExit": null}),
    );
    let pts = needle_points(&mut s, 1);
    // Every crossing reaches 0.2 mm beyond both edges (the hop to the last row stays there).
    for w in body(&pts)
        .windows(2)
        .filter(|w| (w[0].1 - w[1].1).abs() > 1.0)
    {
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
