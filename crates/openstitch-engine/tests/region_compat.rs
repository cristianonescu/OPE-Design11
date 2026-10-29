//! Compatibility tests for region sewing against Brother's PE-DESIGN 11 guide (internal
//! regression tier: the guide's documented behaviour, not reference output):
//! Running Stitch Path (audit item 3), the programmable fill default angle (item 14), and the
//! Fill Frequency and run-time ranges (item 15).

use openstitch_core::Thread;
use openstitch_design::{RegionSewing, RegionType, RunningStitchPath};
use openstitch_engine::Session;
use serde_json::{Value, json};
use std::env::temp_dir;

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The audit's U polygon, mm.
const U: [[f64; 2]; 8] = [
    [10.0, 10.0],
    [40.0, 10.0],
    [40.0, 40.0],
    [30.0, 40.0],
    [30.0, 20.0],
    [20.0, 20.0],
    [20.0, 40.0],
    [10.0, 40.0],
];

fn inside(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
        {
            c = !c;
        }
    }
    c
}

fn distance_to_outline(p: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    (0..poly.len())
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let l2 = d[0] * d[0] + d[1] * d[1];
            let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0);
            ((p[0] - a[0] - t * d[0]).powi(2) + (p[1] - a[1] - t * d[1]).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

/// Sews the U with `region` and returns the stitch entries (x, y mm, command).
fn sew_u(region: Value) -> Vec<(f64, f64, String)> {
    let mut s = Session::new(temp_dir());
    run(&mut s, "shape.polygon", json!({"points": U, "fill": "Red"}));
    let mut params = region;
    params["id"] = json!(1);
    run(&mut s, "object.setRegion", params);
    run(&mut s, "object.toStitches", json!({"ids": [1]}));
    let points = run(&mut s, "stitches.points", json!({"id": 1}));
    points["points"]
        .as_array()
        .expect("points")
        .iter()
        .map(|p| {
            (
                p["x"].as_f64().expect("x"),
                p["y"].as_f64().expect("y"),
                p["command"].as_str().expect("command").to_string(),
            )
        })
        .collect()
}

/// No trim (before the closing one) and no jump after the first entry, and every stitch's midpoint inside the U or
/// within 0.05 mm of its outline (plus the 0.01 mm rounding of `stitches.points`).
fn assert_travels_inside(entries: &[(f64, f64, String)], what: &str) {
    let mut commands: Vec<&str> = entries.iter().map(|e| e.2.as_str()).collect();
    if commands.last() == Some(&"trim") {
        commands.pop();
    }
    assert!(
        !commands.contains(&"trim"),
        "{what}: trims inside the object"
    );
    assert!(
        !commands.iter().skip(1).any(|&c| c == "jump"),
        "{what}: jumps inside the object"
    );
    let stitched = entries.windows(2).filter(|w| w[1].2 == "stitch");
    let mut n = 0;
    for w in stitched {
        let m = [(w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0];
        assert!(
            inside(m, &U) || distance_to_outline(m, &U) <= 0.05 + 0.01,
            "{what}: stitch {:?} -> {:?} leaves the region",
            (w[0].0, w[0].1),
            (w[1].0, w[1].1)
        );
        n += 1;
    }
    assert!(n > 100, "{what}: {n} stitches");
}

#[test]
fn fill_and_satin_travel_between_parts_without_trims() {
    let paths = [
        None,
        Some("inside"),
        Some("alongOutline"),
        Some("onOutline"),
    ];
    for kind in ["fill", "satin"] {
        for path in paths {
            let mut region = json!({"type": kind, "angle": 0});
            if let Some(p) = path {
                region["runningStitchPath"] = json!(p);
            }
            let what = format!("{kind} {}", path.unwrap_or("default"));
            assert_travels_inside(&sew_u(region), &what);
        }
    }
    // Gradation rules out Inside: Along outline is sewn instead, still without trims.
    assert_travels_inside(
        &sew_u(json!({"type": "fill", "angle": 0, "gradation": 50})),
        "fill with gradation",
    );
}

#[test]
fn the_outline_paths_differ_from_inside_travel() {
    let pts = |path: &str| -> Vec<(i64, i64)> {
        // 45°: at 0° the rows end on the notch's edge, where the shortest inside path is the
        // outline itself. PE-DESIGN's 0° U files share that leg-to-leg travel too
        // (compat/reference/stitches/region-travel-u-fill.pes and -on-outline.pes).
        sew_u(json!({"type": "fill", "angle": 45, "runningStitchPath": path}))
            .iter()
            .map(|e| ((e.0 * 100.0) as i64, (e.1 * 100.0) as i64))
            .collect()
    };
    let (inside, along, on) = (pts("inside"), pts("alongOutline"), pts("onOutline"));
    assert_ne!(inside, along);
    assert_ne!(inside, on);
    assert_ne!(along, on);
}

#[test]
fn old_fill_without_travel_still_jumps() {
    // Designs saved before Running Stitch Path, with travel off, sew as they did.
    let entries = sew_u(json!({"type": "fill", "angle": 0, "travel": false}));
    assert!(entries.iter().skip(1).any(|e| e.2 == "jump"));
}

#[test]
fn running_stitch_path_follows_the_guide_exclusions() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "shape.polygon", json!({"points": U, "fill": "Red"}));
    let refused = [
        json!({"runningStitchPath": "outside"}),
        json!({"runningStitchPath": "alongOutline", "autoDirection": true}),
        json!({"runningStitchPath": "alongOutline", "featherWidth": 1}),
        json!({"runningStitchPath": "inside", "gradation": 50}),
        json!({"runningStitchPath": "inside", "type": "spiral"}),
    ];
    for mut params in refused {
        params["id"] = json!(1);
        assert!(
            s.execute("object.setRegion", Some(&params)).is_err(),
            "{params} should be refused"
        );
    }
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "runningStitchPath": "onOutline", "gradation": 50}),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.running_stitch_path, Some(RunningStitchPath::OnOutline));
    run(
        &mut s,
        "text.add",
        json!({"text": "U", "size": 20, "x": 60, "y": 60}),
    );
    let text = s.design().objects[1].id;
    assert!(
        s.execute(
            "object.setRegion",
            Some(&json!({"id": text, "runningStitchPath": "onOutline"}))
        )
        .is_err(),
        "text has no Running Stitch Path"
    );
}

#[test]
fn running_stitch_path_defaults_and_old_files() {
    for kind in [RegionType::Fill, RegionType::Satin] {
        let r = RegionSewing::new(kind, Thread::rgb(0));
        assert_eq!(r.running_stitch_path, Some(RunningStitchPath::Inside));
    }
    let mut stored = serde_json::to_value(RegionSewing::new(RegionType::Fill, Thread::rgb(0)))
        .expect("serialize");
    assert_eq!(stored["runningStitchPath"], json!("inside"));
    let load = |v: &Value| serde_json::from_value::<RegionSewing>(v.clone()).expect("load");
    // Old files: `travel` true travels inside; false or missing jumps.
    let map = stored.as_object_mut().expect("object");
    map.remove("runningStitchPath");
    assert_eq!(load(&stored).running_stitch_path, None);
    stored["travel"] = json!(true);
    assert_eq!(
        load(&stored).running_stitch_path,
        Some(RunningStitchPath::Inside)
    );
    stored["travel"] = json!(false);
    assert_eq!(load(&stored).running_stitch_path, None);
}

#[test]
fn new_fills_take_pe_frequency_and_old_files_keep_the_third_stagger() {
    let fill = RegionSewing::new(RegionType::Fill, Thread::rgb(0));
    assert_eq!(fill.frequency, Some(30.0));
    assert_eq!(
        RegionSewing::new(RegionType::Satin, Thread::rgb(0)).frequency,
        None
    );
    let mut stored = serde_json::to_value(&fill).expect("serialize");
    assert_eq!(stored["frequency"], json!(30.0));
    // A file saved before the default: no `frequency`, so the ⅓ stagger stays.
    stored.as_object_mut().expect("object").remove("frequency");
    let old: RegionSewing = serde_json::from_value(stored).expect("load");
    assert_eq!(old.frequency, None);
    assert_eq!(old.fill_shift(), None);
}

/// Direction (degrees, 0-180) that carries the most stitch length, in 1° bins.
fn dominant_direction(entries: &[(f64, f64, String)]) -> f64 {
    let mut bins = [0.0; 180];
    for w in entries.windows(2).filter(|w| w[1].2 == "stitch") {
        let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 {
            let a = dy.atan2(dx).to_degrees().rem_euclid(180.0);
            bins[(a.round() as usize) % 180] += len;
        }
    }
    let best = (0..180)
        .max_by(|&a, &b| bins[a].total_cmp(&bins[b]))
        .expect("bins");
    best as f64
}

#[test]
fn programmable_fill_defaults_to_45_degrees() {
    assert_eq!(
        RegionSewing::new(RegionType::ProgrammableFill, Thread::rgb(0)).angle,
        45.0
    );
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 40, "outline": "Black"}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "programmableFill"}),
    );
    run(&mut s, "object.setLine", json!({"id": 1, "type": "none"}));
    assert_eq!(
        s.design().objects[0].region.as_ref().expect("region").angle,
        45.0
    );
    run(&mut s, "object.toStitches", json!({"ids": [1]}));
    let points = run(&mut s, "stitches.points", json!({"id": 1}));
    let entries: Vec<(f64, f64, String)> = points["points"]
        .as_array()
        .expect("points")
        .iter()
        .map(|p| {
            (
                p["x"].as_f64().expect("x"),
                p["y"].as_f64().expect("y"),
                p["command"].as_str().expect("command").to_string(),
            )
        })
        .collect();
    let a = dominant_direction(&entries);
    // y points down, so 45° rows appear at 45° or 135° on screen depending on convention.
    assert!(
        (a - 45.0).abs() <= 1.0 || (a - 135.0).abs() <= 1.0,
        "dominant direction {a}°"
    );
}

#[test]
fn frequency_and_run_times_follow_the_guide_ranges() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 40}),
    );
    for params in [
        json!({"id": 1, "type": "fill", "frequency": 60}),
        json!({"id": 1, "type": "fill", "frequency": -1}),
    ] {
        assert!(s.execute("object.setRegion", Some(&params)).is_err());
    }
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "frequency": 50}),
    );
    // Density: PE-DESIGN 11.50's field runs 0.1–14.0 for fill and satin (references
    // sweep-fill-density-min and sweep-satin-density-min sew at 0.1; issue #139's 0.2 floor is
    // contradicted by them), 0.5–10 for the other region types.
    for (kind, density) in [
        ("fill", 0.1),
        ("fill", 14.0),
        ("satin", 0.1),
        ("satin", 0.2),
        ("programmableFill", 0.5),
    ] {
        run(
            &mut s,
            "object.setRegion",
            json!({"id": 1, "type": kind, "density": density}),
        );
    }
    for (kind, density) in [
        ("fill", 0.09),
        ("fill", 14.1),
        ("satin", 0.09),
        ("programmableFill", 0.4),
    ] {
        let params = json!({"id": 1, "type": kind, "density": density});
        assert!(
            s.execute("object.setRegion", Some(&params)).is_err(),
            "{kind} {density}"
        );
    }
    for kind in ["eStitch", "vStitch"] {
        let params = json!({"id": 1, "type": kind, "runTimes": 6});
        assert!(
            s.execute("object.setLine", Some(&params)).is_err(),
            "{kind} 6"
        );
        run(
            &mut s,
            "object.setLine",
            json!({"id": 1, "type": kind, "runTimes": 5}),
        );
    }
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "chain", "runTimes": 6}),
    );
}

/// Fill of a 40×40 mm square at `frequency` (None: default): the needle-grid shift (mm) from
/// each row to the next, modulo the 4 mm stitch length.
fn row_shifts(frequency: Option<f64>) -> Vec<f64> {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 40}),
    );
    // The top rows only: under sewing runs across them.
    let mut params =
        json!({"id": 1, "type": "fill", "angle": 0, "stitchLength": 4, "underlay": false});
    if let Some(f) = frequency {
        params["frequency"] = json!(f);
    }
    run(&mut s, "object.setRegion", params);
    run(&mut s, "object.setLine", json!({"id": 1, "type": "none"}));
    // Sewn from the top row down (no start point), so the rows here are in sewing order.
    run(
        &mut s,
        "object.setEntryExit",
        json!({"id": 1, "regionEntry": null, "regionExit": null}),
    );
    run(&mut s, "object.toStitches", json!({"ids": [1]}));
    let points = run(&mut s, "stitches.points", json!({"id": 1}));
    let mut rows: Vec<(f64, Vec<f64>)> = Vec::new();
    for p in points["points"].as_array().expect("points") {
        let (x, y) = (p["x"].as_f64().expect("x"), p["y"].as_f64().expect("y"));
        // Row ends (on the square's sides) and the tie-in steps are not on the needle grid.
        let interior = x > 11.0 && x < 49.0;
        match rows.last_mut() {
            Some((ry, xs)) if (*ry - y).abs() < 1e-6 => {
                if interior {
                    xs.push(x);
                }
            }
            _ => rows.push((y, if interior { vec![x] } else { Vec::new() })),
        }
    }
    let l = 4.0;
    let phase = |xs: &[f64]| xs.first().map(|x| x.rem_euclid(l));
    // Fill rows only: the straight entry and exit runs give lines with one interior point.
    rows.retain(|(_, xs)| xs.len() >= 2);
    rows.windows(2)
        .filter_map(|w| Some((phase(&w[1].1)? - phase(&w[0].1)?).rem_euclid(l)))
        .collect()
}

#[test]
fn fill_frequency_sets_the_row_to_row_needle_shift() {
    let l = 4.0;
    for (f, expected) in [(Some(0.0), 0.0), (Some(25.0), 0.25), (Some(50.0), 0.5)] {
        let shifts = row_shifts(f);
        assert!(shifts.len() > 50, "{f:?}: {} rows", shifts.len());
        for d in shifts {
            // Circular distance to the expected shift, within 2 % of the stitch length.
            let e = (d - expected * l).rem_euclid(l);
            let e = e.min(l - e);
            assert!(
                e <= 0.02 * l,
                "{f:?}: shift {d} mm, expected {}",
                expected * l
            );
        }
    }
    // A new fill takes PE-DESIGN's default Frequency, 30 % (sweep-fill-density-4p5.pes);
    // older files without a Frequency keep the ⅓ stagger (see the test below).
    for d in row_shifts(None) {
        let e = (d - 0.3 * l).rem_euclid(l);
        assert!(e.min(l - e) <= 0.02 * l, "default: shift {d}");
    }
}
