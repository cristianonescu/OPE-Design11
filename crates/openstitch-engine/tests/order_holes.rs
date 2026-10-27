//! Compatibility audit items 16 (Optimize Sewing Order moves entry and exit points; the
//! "Optimize Entry/Exit Points" option) and 17 (hole sewing is a validated pair). Item 16 follows
//! PE-DESIGN 11.50's output (spec `optimize-entry-exit`); the rest are internal regressions built
//! from the PE-DESIGN 11 online guide.

use openstitch_core::{Command, Stitch};
use openstitch_design::Compiler;
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn order(s: &Session) -> Vec<u64> {
    s.design().objects.iter().map(|o| o.id).collect()
}

/// Needle travel between objects: from the last stitch before each run of jumps / trims to
/// the first stitch after it, mm.
fn jumps(s: &Session) -> Vec<f64> {
    design_jumps(s.design())
}

/// [`jumps`] of a design.
fn design_jumps(d: &openstitch_design::Design) -> Vec<f64> {
    let stitches = Compiler::default().compile(d).stitches;
    let mut out = Vec::new();
    let (mut last, mut jumped): (Option<&Stitch>, bool) = (None, false);
    for st in &stitches {
        match st.command {
            Command::Stitch => {
                if let (Some(l), true) = (last, jumped) {
                    out.push(((st.x - l.x).hypot(st.y - l.y)) / 10.0);
                }
                (last, jumped) = (Some(st), false);
            }
            Command::Jump | Command::Trim => jumped = true,
            _ => {}
        }
    }
    out
}

/// Three same-colour 10 mm squares drawn at x = 0, 50 and 25 mm (ids 1, 2, 3), without under
/// sewing, so each fill's first needle point is its entry point (as in compat
/// optimize-entry-exit).
fn three_squares(s: &mut Session) {
    for x in [0, 50, 25] {
        run(
            s,
            "shape.rectangle",
            json!({"x": x, "y": 0, "width": 10, "height": 10, "fill": "Red"}),
        );
    }
    run(
        s,
        "object.setRegion",
        json!({"ids": [1, 2, 3], "underlay": false}),
    );
}

#[test]
fn optimize_moves_entry_and_exit_to_the_closest_points() {
    let mut s = Session::new(std::env::temp_dir());
    three_squares(&mut s);
    let plain = jumps(&s);
    run(&mut s, "order.optimize", json!({}));
    // PE-DESIGN 11 keeps the drawing order of same-colour objects (x = 0, 50, 25).
    assert_eq!(order(&s), [1, 2, 3]);
    let j = jumps(&s);
    assert_eq!(j.len(), 2, "one jump between neighbours: {j:?}");
    // Each jump crosses the gap between the outlines: 40 mm, then 15 mm.
    for (d, gap) in j.iter().zip([40.0, 15.0]) {
        assert!(
            (d - gap).abs() <= 0.5,
            "jump {d} mm, expected the {gap} mm gap ({j:?})"
        );
    }
    assert!(j.iter().sum::<f64>() < plain.iter().sum::<f64>());
    // The points are on the facing sides.
    let d = s.design();
    let r = |i: usize| d.objects[i].region.as_ref().expect("region");
    assert_eq!(r(0).exit.map(|p| p.x), Some(100.0));
    assert_eq!(r(1).entry.map(|p| p.x), Some(500.0));
    assert_eq!(r(1).exit.map(|p| p.x), Some(500.0));
    assert_eq!(r(2).entry.map(|p| p.x), Some(350.0));
    // The ends that link to nothing keep the Rectangle tool's start and end point, the
    // middle of the bottom edge (PE-DESIGN 11; spec shape-start-points).
    assert_eq!(r(0).entry, Some(openstitch_design::v2(50.0, 100.0)));
    assert_eq!(r(2).exit, Some(openstitch_design::v2(300.0, 100.0)));
}

#[test]
fn optimize_keeps_colour_groups_and_overlap_order() {
    let mut s = Session::new(std::env::temp_dir());
    for (x, c) in [(0, "Red"), (40, "Blue"), (20, "Red")] {
        run(
            &mut s,
            "shape.rectangle",
            json!({"x": x, "y": 0, "width": 10, "height": 10, "fill": c}),
        );
    }
    // A blue square over the first red one must stay after it.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 5, "y": 5, "width": 10, "height": 10, "fill": "Blue"}),
    );
    run(&mut s, "order.optimize", json!({}));
    let o = order(&s);
    assert_eq!(o[..2], [1, 3], "the reds sew together: {o:?}");
    let at = |id| o.iter().position(|&x| x == id).expect("kept");
    assert!(at(4) > at(1), "the overlapping square stays on top: {o:?}");
    // Entry and exit points link only same-colour neighbours, as in PE-DESIGN 11.
    let d = s.design();
    let r = |id: u64| {
        d.objects[d.index_of(id).expect("kept")]
            .region
            .as_ref()
            .expect("region")
    };
    assert!(
        r(1).exit.is_some() && r(3).entry.is_some(),
        "red to red is linked"
    );
    assert_eq!(
        r(3).exit,
        Some(openstitch_design::v2(250.0, 100.0)),
        "red to blue is not linked; the default bottom-centre point stays: {o:?}"
    );
}

#[test]
fn optimize_entry_exit_option_persists_and_applies_to_new_shapes() {
    let dir = std::env::temp_dir().join(format!("os-entry-exit-{}", std::process::id()));
    let file = dir.join("user.json");
    let _ = std::fs::remove_file(&file);
    let mut s = Session::new(&dir);
    s.with_user_settings(&file).expect("settings");
    assert_eq!(
        run(&mut s, "options.get", json!({})),
        json!({"optimizeEntryExit": false})
    );
    // Off: new shapes keep the Rectangle tool's default entry and exit, the middle of the bottom
    // edge (PE-DESIGN 11; spec shape-start-points).
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 25, "y": 0, "width": 10, "height": 10}),
    );
    assert_eq!(
        s.design().objects[1].region.as_ref().expect("r").entry,
        Some(openstitch_design::v2(300.0, 100.0))
    );
    run(&mut s, "options.set", json!({"optimizeEntryExit": true}));
    assert!(
        s.execute("options.set", Some(&json!({"optimizeEntryExit": 1})))
            .is_err()
    );

    // The next app start sees the option.
    let mut t = Session::new(&dir);
    t.with_user_settings(&file).expect("load");
    assert!(t.user_settings().optimize_entry_exit);
    run(
        &mut t,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    run(
        &mut t,
        "shape.rectangle",
        json!({"x": 25, "y": 0, "width": 10, "height": 10}),
    );
    let d = t.design();
    assert_eq!(
        d.objects[0]
            .region
            .as_ref()
            .and_then(|r| r.exit)
            .map(|p| p.x),
        Some(100.0)
    );
    assert_eq!(
        d.objects[1]
            .region
            .as_ref()
            .and_then(|r| r.entry)
            .map(|p| p.x),
        Some(250.0)
    );
    // Measured without under sewing, so the fills start on their entry points (a copy, so the
    // undo history below is unchanged).
    let mut plain = t.design().clone();
    for o in &mut plain.objects {
        if let Some(r) = &mut o.region {
            r.underlay = false;
        }
    }
    let j = design_jumps(&plain);
    assert!(j.len() == 1 && (j[0] - 15.0).abs() <= 0.5, "{j:?}");
    // One undo step removes the shape and the moved exit point together: the first square's
    // exit is back at its own default, the middle of its bottom edge.
    run(&mut t, "history.undo", json!({}));
    assert_eq!(
        t.design().objects[0].region.as_ref().expect("r").exit,
        Some(openstitch_design::v2(50.0, 100.0))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn star(cx: f64, cy: f64, r: f64, inner: f64) -> Vec<[f64; 2]> {
    (0..10)
        .map(|i| {
            let a = std::f64::consts::PI * (i as f64 / 5.0) - std::f64::consts::FRAC_PI_2;
            let rr = if i % 2 == 0 { r } else { inner };
            [cx + rr * a.cos(), cy + rr * a.sin()]
        })
        .collect()
}

fn pentagon(cx: f64, cy: f64, r: f64) -> Vec<[f64; 2]> {
    (0..5)
        .map(|i| {
            let a = 2.0 * std::f64::consts::PI * (i as f64 / 5.0) - std::f64::consts::FRAC_PI_2;
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect()
}

fn shoelace(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1])
        .sum::<f64>()
        .abs()
        / 2.0
}

/// Area sewn by the first colour block, mm²: the fill rows (horizontal stitches) times the row
/// spacing.
fn first_block_area(s: &Session) -> f64 {
    let pattern = Compiler::default().compile(s.design());
    let blocks = pattern.color_blocks();
    let rows: Vec<(f64, f64)> = blocks[0]
        .1
        .windows(2)
        .filter(|w| w[0].command == Command::Stitch && w[1].command == Command::Stitch)
        .map(|w| ((w[1].x - w[0].x).abs(), (w[1].y - w[0].y).abs(), w[0].y))
        .filter(|&(dx, dy, _)| dx > 0.0 && dy < 0.05 * dx)
        .map(|(dx, _, y)| (dx, y))
        .collect();
    let mut ys: Vec<f64> = rows.iter().map(|r| r.1).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    let mut gaps: Vec<f64> = ys.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_by(f64::total_cmp);
    let spacing = gaps[gaps.len() / 2];
    rows.iter().map(|r| r.0).sum::<f64>() * spacing / 100.0
}

/// A 40 mm pentagon (id 1) with a 20 mm star inside it (id 2).
fn pentagon_and_star(s: &mut Session) -> (f64, f64) {
    let (p, st) = (pentagon(50.0, 50.0, 20.0), star(50.0, 52.0, 10.0, 4.0));
    run(s, "shape.polygon", json!({"points": p, "fill": "Blue"}));
    run(s, "shape.polygon", json!({"points": st, "fill": "Yellow"}));
    (shoelace(&p), shoelace(&st))
}

#[test]
fn hole_sewing_cuts_out_only_the_paired_shape() {
    let mut s = Session::new(std::env::temp_dir());
    let (pentagon, star) = pentagon_and_star(&mut s);
    let full = first_block_area(&s);
    assert!(
        (full - pentagon).abs() <= 0.02 * pentagon,
        "{full} vs {pentagon}"
    );
    // The pair is given inner first; the outer one is found by enclosure.
    let r = run(
        &mut s,
        "object.setHoleSewing",
        json!({"ids": [2, 1], "enabled": true}),
    );
    assert_eq!(r, json!({"outer": 1, "inner": 2}));
    let o = &s.design().objects[0];
    assert!(o.hole_sewing);
    assert_eq!(o.hole_inner, Some(2));
    let holed = first_block_area(&s);
    let expected = pentagon - star;
    assert!(
        (holed - expected).abs() <= 0.02 * expected,
        "stitched {holed} mm², pentagon − star {expected} mm²"
    );

    // A later region that is not the pair is not cut out (no Remove Overlap behaviour).
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 45, "y": 62, "width": 10, "height": 4, "fill": "Red"}),
    );
    let again = first_block_area(&s);
    assert!((again - holed).abs() < 1e-9, "{again} vs {holed}");

    // Saved and reopened, the pair is kept; turning it off clears it.
    let dir = std::env::temp_dir().join(format!("os-holes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let mut t = Session::new(&dir);
    run(
        &mut t,
        "object.paste",
        json!({"objects": serde_json::to_value(&s.design().objects).expect("json")}),
    );
    // Pasted together, the copies stay a pair under their new ids.
    assert_eq!(
        t.design().objects[0].hole_inner,
        Some(t.design().objects[1].id)
    );
    run(&mut t, "project.save", json!({"path": "h.osd"}));
    let mut u = Session::new(&dir);
    run(&mut u, "project.open", json!({"path": "h.osd"}));
    assert_eq!(u.design(), t.design());
    run(
        &mut u,
        "object.setHoleSewing",
        json!({"id": 2, "enabled": false}),
    );
    let o = &u.design().objects[0];
    assert!(!o.hole_sewing && o.hole_inner.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn designs_saved_before_pairs_cut_out_the_enclosed_region_only() {
    let mut s = Session::new(std::env::temp_dir());
    let (pentagon, star) = pentagon_and_star(&mut s);
    // A later region outside the pentagon's hole.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 45, "y": 62, "width": 10, "height": 4, "fill": "Red"}),
    );
    // Old .osd: `"holeSewing": true` and no `holeInner`.
    let mut v = serde_json::to_value(&s.design().objects[0]).expect("json");
    v["holeSewing"] = json!(true);
    assert!(v.get("holeInner").is_none());
    let old: openstitch_design::DesignObject = serde_json::from_value(v).expect("old object");
    assert_eq!(old.hole_inner, None);
    let mut d = s.design().clone();
    d.objects[0] = old;
    let mut t = Session::new(std::env::temp_dir());
    run(
        &mut t,
        "object.paste",
        json!({"objects": serde_json::to_value(&d.objects).expect("json")}),
    );
    assert!(t.design().objects[0].hole_sewing);
    let holed = first_block_area(&t);
    let expected = pentagon - star;
    assert!(
        (holed - expected).abs() <= 0.02 * expected,
        "{holed} vs {expected}"
    );
}

#[test]
fn hole_sewing_is_refused_outside_the_guide_limits() {
    let mut s = Session::new(std::env::temp_dir());
    // 1, 2: partial overlap.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20}),
    );
    // 3: a bow tie that crosses itself, 4: a square inside its right half.
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[50, 0], [90, 40], [90, 0], [50, 40]]}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 83, "y": 17, "width": 4, "height": 6}),
    );
    // 5: a fill sewn along two direction lines, 6: a square inside it.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 50, "width": 40, "height": 20}),
    );
    run(
        &mut s,
        "object.setDirectionLines",
        json!({"id": 5, "lines": [[[0, 50], [0, 70]], [[40, 50], [40, 70]]]}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 15, "y": 55, "width": 5, "height": 5}),
    );
    // 7: feathered edges, 8: a square inside it.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 50, "y": 50, "width": 40, "height": 20}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 7, "featherWidth": 2}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 65, "y": 55, "width": 5, "height": 5}),
    );
    // 9: a square with nothing inside it.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 100, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    for (params, reason) in [
        (json!({"ids": [1, 2]}), "fully inside"),
        (json!({"ids": [3, 4]}), "crosses itself"),
        (json!({"id": 3}), "crosses itself"),
        (json!({"ids": [5, 6]}), "direction lines"),
        (json!({"ids": [7, 8]}), "feathered"),
        (json!({"id": 9}), "encloses no other region"),
        (json!({"ids": [1, 2, 9]}), "exactly two"),
    ] {
        let mut p = params.clone();
        p["enabled"] = json!(true);
        let e = s
            .execute("object.setHoleSewing", Some(&p))
            .expect_err(&p.to_string())
            .to_string();
        assert!(e.contains(reason), "{p}: {e}");
        assert_eq!(s.design(), &before, "{p}");
    }
    // A feathered edge is refused on either shape of a pair.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 7, "featherWidth": 0}),
    );
    run(
        &mut s,
        "object.setHoleSewing",
        json!({"ids": [7, 8], "enabled": true}),
    );
    let before = s.design().clone();
    for id in [7, 8] {
        let e = s
            .execute(
                "object.setRegion",
                Some(&json!({"id": id, "featherWidth": 2})),
            )
            .expect_err("feather refused");
        assert!(e.to_string().contains("feathered"), "{e}");
        assert_eq!(s.design(), &before);
    }
    // Other attributes still change.
    run(&mut s, "object.setRegion", json!({"id": 7, "angle": 45}));
}
