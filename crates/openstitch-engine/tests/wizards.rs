//! Pattern wizards: background fill, appliqué, embroidered patch, cutwork. Each test checks the
//! stitch order, the colour blocks (stops), the geometry offsets and one-step undo.

use openstitch_core::{Command, StitchPattern};
use openstitch_design::{Compiler, Contour, Design, Vec2, v2};
use openstitch_engine::Session;
use serde_json::{Value, json};

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir()
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn compile(d: &Design) -> StitchPattern {
    Compiler::for_design(d).compile(d)
}

/// A 100 × 100 mm page with a red circle (r = 10 mm) at its centre: object 1.
fn circle_session() -> Session {
    let mut s = Session::new(temp_dir());
    run(&mut s, "design.new", json!({"width": 100, "height": 100}));
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 10, "fill": "Red"}),
    );
    s
}

/// Every flattened point of the object's contours, mm.
fn points_mm(d: &Design, id: u64) -> Vec<Vec2> {
    let o = &d.objects[d.index_of(id).expect("object")];
    o.contours
        .iter()
        .flat_map(Contour::path)
        .map(|p| v2(p.x / 10.0, p.y / 10.0))
        .collect()
}

/// Undo removes exactly what the command added.
fn assert_one_undo(s: &mut Session, before: &Design) {
    run(s, "history.undo", json!({}));
    assert_eq!(s.design(), before, "one undo step");
}

/// Angle of `a`→`b` in degrees, folded into 0..180.
fn direction(a: Vec2, b: Vec2) -> f64 {
    (b.y - a.y).atan2(b.x - a.x).to_degrees().rem_euclid(180.0)
}

// MARK: background fill

#[test]
fn echo_clipped_rings_sit_at_multiples_of_the_spacing() {
    let mut s = circle_session();
    let before = s.design().clone();
    let r = run(
        &mut s,
        "wizard.backgroundFill",
        json!({"ids": [1], "fillType": "echo", "spacing": 5, "sew": "triple"}),
    );
    let id = r["id"].as_u64().expect("id");
    let d = s.design().clone();
    // Sewn first: the background goes before the pattern it surrounds.
    assert_eq!(d.objects[0].id, id);
    assert_eq!(d.objects[1].id, 1);
    let o = &d.objects[0];
    assert_eq!(o.name.as_deref(), Some("Background fill"));
    assert!(o.region.is_none() && o.line.is_some());
    let pts = points_mm(&d, id);
    assert!(!pts.is_empty());
    let mut rings = std::collections::BTreeSet::new();
    for p in &pts {
        assert!(
            (-0.01..=100.01).contains(&p.x) && (-0.01..=100.01).contains(&p.y),
            "{p:?} outside the hoop"
        );
        let gap = p.distance(v2(50.0, 50.0)) - 10.0;
        let k = (gap / 5.0).round();
        assert!(k >= 1.0, "{p:?} is closer than the spacing");
        assert!(
            (gap - 5.0 * k).abs() < 0.35,
            "{p:?}: {gap} mm is not a ring"
        );
        rings.insert(k as i64);
    }
    // Rings reach the page corners (≈ 60.7 mm from the circle's edge).
    assert!(rings.len() >= 11, "{rings:?}");
    // Colour blocks: the background thread, then the circle.
    let pattern = compile(&d);
    assert_eq!(pattern.threads.len(), 2);
    assert_eq!(pattern.threads[0].hex(), "#000000");
    assert_eq!(pattern.count(Command::ColorChange), 1);
    // Deterministic.
    let mut t = circle_session();
    run(
        &mut t,
        "wizard.backgroundFill",
        json!({"ids": [1], "fillType": "echo", "spacing": 5, "sew": "triple"}),
    );
    assert_eq!(t.design(), &d);
    assert_one_undo(&mut s, &before);
}

#[test]
fn linked_echo_ends_sew_with_fewer_jumps() {
    let mut s = circle_session();
    let loose = run(&mut s, "wizard.backgroundFill", json!({"ids": [1]}));
    let loose_lines = loose["contours"].as_u64().expect("count");
    let jumps_loose = compile(s.design()).count(Command::Jump);
    run(&mut s, "history.undo", json!({}));
    let linked = run(
        &mut s,
        "wizard.backgroundFill",
        json!({"ids": [1], "linkEchoEnds": true}),
    );
    assert!(linked["contours"].as_u64().expect("count") < loose_lines);
    assert!(compile(s.design()).count(Command::Jump) < jumps_loose);
    // Joins stay outside the circle.
    for p in points_mm(s.design(), linked["id"].as_u64().expect("id")) {
        assert!(p.distance(v2(50.0, 50.0)) > 12.0, "{p:?}");
    }
}

#[test]
fn echo_rings_stay_between_both_outlines() {
    let mut s = circle_session();
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 80, "height": 80, "outline": "Black"}),
    );
    let r = run(
        &mut s,
        "wizard.backgroundFill",
        json!({"ids": [1], "area": 2, "echoType": "echo", "spacing": 4}),
    );
    let d = s.design();
    let o = &d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")];
    assert!(o.contours.iter().all(|c| c.closed));
    for p in points_mm(d, o.id) {
        assert!(p.distance(v2(50.0, 50.0)) > 10.0 + 4.0 - 0.3, "{p:?}");
        assert!(
            (14.0 - 0.3..=86.0 + 0.3).contains(&p.x) && (14.0 - 0.3..=86.0 + 0.3).contains(&p.y),
            "{p:?}"
        );
    }
    // The area shape itself is left as it was.
    assert_eq!(d.objects[d.index_of(2).expect("area")].contours.len(), 1);
}

#[test]
fn stippling_skips_the_pattern_and_its_gap() {
    let mut s = circle_session();
    let r = run(
        &mut s,
        "wizard.backgroundFill",
        json!({"ids": [1], "fillType": "stippling", "spacing": 3, "color": "#00a050"}),
    );
    let d = s.design().clone();
    let o = &d.objects[0];
    assert_eq!(o.id, r["id"].as_u64().expect("id"));
    let region = o.region.as_ref().expect("region");
    assert_eq!(region.kind.id(), "stippling");
    // Background first, then the circle: two colour blocks.
    let pattern = compile(&d);
    assert_eq!(pattern.threads.len(), 2);
    let first_block: Vec<Vec2> = pattern
        .stitches
        .iter()
        .take_while(|st| st.command != Command::ColorChange)
        .filter(|st| st.command == Command::Stitch)
        .map(|st| v2(st.x / 10.0, st.y / 10.0))
        .collect();
    assert!(first_block.len() > 100);
    for p in &first_block {
        assert!(
            p.distance(v2(50.0, 50.0)) > 13.0 - 0.3,
            "{p:?} inside the gap"
        );
    }
    // skipOverlap false fills under the pattern too.
    run(&mut s, "history.undo", json!({}));
    run(
        &mut s,
        "wizard.backgroundFill",
        json!({"ids": [1], "fillType": "stippling", "skipOverlap": false}),
    );
    let o = &s.design().objects[0];
    assert!(openstitch_design::geometry::contains(
        v2(500.0, 500.0),
        &o.contours
    ));
}

#[test]
fn crosshatch_and_stripes_follow_the_angle_and_line_spacing() {
    for (kind, families) in [("stripes", 1), ("crosshatch", 2)] {
        let mut s = circle_session();
        let r = run(
            &mut s,
            "wizard.backgroundFill",
            json!({"ids": [1], "fillType": kind, "angle": 60, "lineSpacing": 8, "spacing": 2}),
        );
        let d = s.design();
        let o = &d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")];
        let mut angles = std::collections::BTreeSet::new();
        let mut offsets: Vec<f64> = Vec::new();
        for c in &o.contours {
            let (a, b) = (c.points[0], *c.points.last().expect("end"));
            let ang = direction(a, b);
            let fam = if (ang - 60.0).abs() < 0.5 {
                60
            } else if (ang - 150.0).abs() < 0.5 {
                150
            } else {
                panic!("{kind}: line at {ang}°");
            };
            angles.insert(fam);
            if fam == 60 {
                // Distance along the normal, mm.
                let n = v2(-(60f64.to_radians().sin()), 60f64.to_radians().cos());
                offsets.push((a.x * n.x + a.y * n.y) / 10.0);
            }
            for p in c.path() {
                assert!(p.distance(v2(500.0, 500.0)) > 120.0 - 3.0, "{kind}: {p:?}");
            }
        }
        assert_eq!(angles.len(), families, "{kind}");
        // Linked lines: every join is short and the hatch lines keep their angle.
        let unlinked = o.contours.len();
        run(&mut s, "history.undo", json!({}));
        let r = run(
            &mut s,
            "wizard.backgroundFill",
            json!({"ids": [1], "fillType": kind, "angle": 60, "lineSpacing": 8, "spacing": 2, "linkEchoEnds": true}),
        );
        let d = s.design();
        let linked = &d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")];
        assert!(
            linked.contours.len() < unlinked,
            "{kind}: lines were joined"
        );
        for c in &linked.contours {
            for w in c.points.windows(2) {
                let ang = direction(w[0], w[1]);
                let on_hatch = [60.0, 150.0].iter().any(|f| (ang - f).abs() < 0.5);
                assert!(
                    on_hatch || w[0].distance(w[1]) <= 160.0,
                    "{kind}: join {w:?}"
                );
            }
        }
        offsets.sort_by(f64::total_cmp);
        offsets.dedup_by(|x, y| (*x - *y).abs() < 0.01);
        for w in offsets.windows(2) {
            let k = ((w[1] - w[0]) / 8.0).round();
            assert!(k >= 1.0 && (w[1] - w[0] - 8.0 * k).abs() < 0.01, "{w:?}");
        }
    }
}

#[test]
fn decorative_fill_and_hoop_area() {
    let mut s = circle_session();
    run(&mut s, "machine.set", json!({"hoop": "100x100"}));
    let r = run(
        &mut s,
        "wizard.backgroundFill",
        json!({"ids": [1], "fillType": "decorative", "patternWidth": 8, "patternHeight": 6, "connectEdges": false}),
    );
    let d = s.design();
    let o = &d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")];
    let region = o.region.as_ref().expect("region");
    assert_eq!(region.kind.id(), "decorativeFill");
    assert_eq!(
        (region.pattern_width, region.pattern_height),
        (Some(8.0), Some(6.0))
    );
    assert!(!region.travel);
    let b = o.bounds().expect("bounds");
    assert!((b.width() - 1000.0).abs() < 1.0 && (b.height() - 1000.0).abs() < 1.0);
}

#[test]
fn background_fill_works_around_text_and_open_lines() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "text.add",
        json!({"text": "O", "x": 50, "y": 50, "size": 20}),
    );
    run(
        &mut s,
        "shape.path",
        json!({"points": [[10, 80], [90, 80]], "closed": false, "outline": "Blue"}),
    );
    let r = run(
        &mut s,
        "wizard.backgroundFill",
        json!({"ids": [1, 2], "fillType": "stippling"}),
    );
    let d = s.design();
    let o = &d.objects[0];
    assert_eq!(o.id, r["id"].as_u64().expect("id"));
    // The counter of the O is part of the letter's footprint, so it is not filled.
    let text = &d.objects[1];
    let b = text.bounds().expect("text");
    assert!(!openstitch_design::geometry::contains(
        b.center(),
        &o.contours
    ));
    // The open line counts as its bounding box (plus 0.5 mm) and the 2 mm gap.
    assert!(!openstitch_design::geometry::contains(
        v2(500.0, 815.0),
        &o.contours
    ));
    assert!(openstitch_design::geometry::contains(
        v2(500.0, 860.0),
        &o.contours
    ));
}

#[test]
fn bad_background_fill_parameters_fail_cleanly() {
    let mut s = circle_session();
    run(
        &mut s,
        "shape.path",
        json!({"points": [[0, 0], [10, 10]], "closed": false}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 55, "y": 55, "width": 5, "height": 5}),
    );
    let before = s.design().clone();
    let bad: Vec<Value> = vec![
        json!({}),
        json!({"ids": []}),
        json!({"ids": [99]}),
        json!({"ids": ["1"]}),
        json!({"ids": [1], "fillType": "plaid"}),
        json!({"ids": [1], "spacing": 0}),
        json!({"ids": [1], "spacing": 1e9}),
        json!({"ids": [1], "spacing": "5"}),
        json!({"ids": [1], "spacing": f64::MAX}),
        json!({"ids": [1], "fillType": "stippling", "spacing": 8}),
        json!({"ids": [1], "fillType": "stippling", "stippleSpacing": 0}),
        json!({"ids": [1], "echoType": "ripple"}),
        json!({"ids": [1], "sew": "satin"}),
        json!({"ids": [1], "pitch": 0}),
        json!({"ids": [1], "runTimes": 6}),
        json!({"ids": [1], "runTimes": 1.5}),
        json!({"ids": [1], "chainShape": "loop"}),
        json!({"ids": [1], "sew": "chain", "chainSize": 50}),
        json!({"ids": [1], "color": "#12345"}),
        json!({"ids": [1], "linkEchoEnds": "yes"}),
        json!({"ids": [1], "area": 1}),
        json!({"ids": [1], "area": 2}),
        json!({"ids": [1], "area": 99}),
        json!({"ids": [1], "area": -1}),
        // The area lies entirely inside the pattern's gap: nothing to fill.
        json!({"ids": [1], "area": 3, "fillType": "stippling", "spacing": 7}),
        json!({"ids": [1], "fillType": "decorative", "pattern": "nope"}),
        json!({"ids": [1], "fillType": "decorative", "patternWidth": 0}),
        json!({"ids": [1], "fillType": "crosshatch", "lineSpacing": 1}),
        json!({"ids": [1], "fillType": "stripes", "angle": 10}),
        json!({"ids": [1], "fillType": "stripes", "angle": "45"}),
        json!({"ids": [1], "skipOverlap": 1}),
    ];
    for params in bad {
        let r = s.execute("wizard.backgroundFill", Some(&params));
        assert!(r.is_err(), "{params} should fail");
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
}

// MARK: appliqué

/// Thread names of the compiled colour blocks, in sewing order.
fn block_names(p: &StitchPattern) -> Vec<String> {
    p.threads
        .iter()
        .map(|t| t.name.as_deref().unwrap_or("").to_string())
        .collect()
}

/// Ids from a wizard result's `ids`.
fn ids(r: &Value) -> Vec<u64> {
    r["ids"]
        .as_array()
        .expect("ids")
        .iter()
        .filter_map(Value::as_u64)
        .collect()
}

#[test]
fn applique_stages_sew_in_order_with_stops_between() {
    let mut s = circle_session();
    let before = s.design().clone();
    let r = run(
        &mut s,
        "wizard.applique",
        json!({"ids": [1], "output": "replace", "cover": "zigzag", "coverWidth": 3, "coverDensity": 5}),
    );
    let new = ids(&r);
    assert_eq!(new.len(), 4);
    assert_eq!(r["removed"], json!([1]));
    let d = s.design().clone();
    let names: Vec<&str> = d.objects.iter().filter_map(|o| o.name.as_deref()).collect();
    assert_eq!(
        names,
        [
            "Appliqué material",
            "Appliqué position",
            "Appliqué tack-down",
            "Appliqué cover"
        ]
    );
    // One group, in sewing order, replacing the circle.
    let group = d.objects[0].group.expect("grouped");
    assert!(d.objects.iter().all(|o| o.group == Some(group)));
    assert_eq!(d.objects.iter().map(|o| o.id).collect::<Vec<_>>(), new);
    let pattern = compile(&d);
    assert_eq!(
        block_names(&pattern),
        [
            "Appliqué material",
            "Appliqué position",
            "Appliqué tack-down",
            "Red"
        ]
    );
    // Three stops: the colour changes between the four stages; every colour is the pattern's.
    assert_eq!(pattern.count(Command::ColorChange), 3);
    assert!(pattern.threads.iter().all(|t| t.hex() == "#ed171f"));
    // All stages follow the circle's outline (r = 10 mm).
    for o in &d.objects {
        for p in points_mm(&d, o.id) {
            assert!((p.distance(v2(50.0, 50.0)) - 10.0).abs() < 0.05, "{p:?}");
        }
    }
    // Cover zigzag reaches 1.5 mm to each side; tack-down 1 mm.
    let blocks = needle_blocks(&pattern);
    let reach = |b: &[Vec2]| {
        b.iter()
            .map(|p| (p.distance(v2(50.0, 50.0)) - 10.0).abs())
            .fold(0.0, f64::max)
    };
    assert!(
        (reach(&blocks[3]) - 1.5).abs() < 0.1,
        "{}",
        reach(&blocks[3])
    );
    assert!(
        (reach(&blocks[2]) - 1.0).abs() < 0.1,
        "{}",
        reach(&blocks[2])
    );
    assert!(reach(&blocks[1]) < 0.05 && reach(&blocks[0]) < 0.05);
    assert_one_undo(&mut s, &before);
}

/// Stitch positions (mm) per colour block.
fn needle_blocks(p: &StitchPattern) -> Vec<Vec<Vec2>> {
    let mut out = vec![Vec::new()];
    for st in &p.stitches {
        match st.command {
            Command::ColorChange => out.push(Vec::new()),
            Command::Stitch => out
                .last_mut()
                .expect("block")
                .push(v2(st.x / 10.0, st.y / 10.0)),
            _ => {}
        }
    }
    out
}

#[test]
fn applique_add_keeps_the_pattern_and_offsets_the_outline() {
    let mut s = circle_session();
    let r = run(
        &mut s,
        "wizard.applique",
        json!({"ids": [1], "distance": 4, "material": "none", "tackDown": "vStitch", "cover": "eStitch"}),
    );
    let new = ids(&r);
    assert_eq!(new.len(), 3);
    let d = s.design().clone();
    // The appliqué goes under the pattern, which stays and is sewn last.
    assert_eq!(d.objects.last().map(|o| o.id), Some(1));
    for &id in &new {
        for p in points_mm(&d, id) {
            assert!((p.distance(v2(50.0, 50.0)) - 14.0).abs() < 0.05, "{p:?}");
        }
    }
    let pattern = compile(&d);
    assert_eq!(
        block_names(&pattern),
        ["Appliqué position", "Appliqué tack-down", "Red"]
    );
    // Position → tack-down → cover are stops; the cover and the pattern share a colour block.
    assert_eq!(pattern.count(Command::ColorChange), 2);
    let kinds: Vec<_> = d.objects[..3]
        .iter()
        .map(|o| o.line.as_ref().expect("line").kind.id())
        .collect();
    assert_eq!(kinds, ["running", "vStitch", "eStitch"]);
}

#[test]
fn applique_material_can_be_cut_by_cutwork_needles() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 20, "y": 20, "width": 40, "height": 30, "fill": "Blue"}),
    );
    run(
        &mut s,
        "wizard.applique",
        json!({"ids": [1], "material": "cutting", "output": "replace", "cover": "chain", "coverWidth": 2, "coverTimes": 2, "color": "Black"}),
    );
    let d = s.design();
    let material = d.objects[0].line.as_ref().expect("line");
    assert_eq!(material.kind.id(), "cutting");
    assert!(material.leave_uncut, "bridges are left for hand cutting");
    let names = block_names(&compile(d));
    // Horizontal and vertical cut lines use the 0° and 90° needles.
    assert_eq!(
        names,
        [
            "Cutwork needle 1 (0°)",
            "Cutwork needle 3 (90°)",
            "Appliqué position",
            "Appliqué tack-down",
            "Black"
        ]
    );
}

#[test]
fn applique_replace_keeps_or_fills_holes() {
    for (holes, contours) in [(true, 2), (false, 1)] {
        let mut s = Session::new(temp_dir());
        run(
            &mut s,
            "text.add",
            json!({"text": "O", "x": 50, "y": 50, "size": 30}),
        );
        run(
            &mut s,
            "wizard.applique",
            json!({"ids": [1], "output": "replace", "holes": holes, "tackDown": "none"}),
        );
        let d = s.design();
        assert_eq!(d.objects.len(), 3);
        assert!(
            d.objects.iter().all(|o| o.contours.len() == contours),
            "holes {holes}"
        );
        assert_eq!(compile(d).count(Command::ColorChange), 2);
    }
}

#[test]
fn bad_applique_parameters_fail_cleanly() {
    let mut s = circle_session();
    run(
        &mut s,
        "shape.path",
        json!({"points": [[0, 0], [10, 10]], "closed": false}),
    );
    let before = s.design().clone();
    let bad: Vec<Value> = vec![
        json!({}),
        json!({"ids": []}),
        json!({"ids": [42]}),
        json!({"ids": [1], "material": "laser"}),
        json!({"ids": [1], "tackDown": "satin"}),
        json!({"ids": [1], "tackDownWidth": 0}),
        json!({"ids": [1], "tackDownDensity": 100}),
        json!({"ids": [1], "tackDownInterval": -1}),
        json!({"ids": [1], "cover": "running"}),
        json!({"ids": [1], "coverWidth": 1e9}),
        json!({"ids": [1], "coverDensity": 0}),
        json!({"ids": [1], "coverInterval": "3"}),
        json!({"ids": [1], "coverPitch": 0.1}),
        json!({"ids": [1], "coverTimes": 9}),
        json!({"ids": [1], "chainShape": "square"}),
        json!({"ids": [1], "output": "merge"}),
        json!({"ids": [1], "distance": -2}),
        json!({"ids": [1], "distance": 51}),
        json!({"ids": [1], "output": "replace", "distance": 2}),
        json!({"ids": [1], "holes": "no"}),
        json!({"ids": [1], "color": "plaid"}),
        // Replace needs closed outlines.
        json!({"ids": [2], "output": "replace"}),
        json!({"ids": [1, 2], "output": "replace"}),
    ];
    for params in bad {
        let r = s.execute("wizard.applique", Some(&params));
        assert!(r.is_err(), "{params} should fail");
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
}

// MARK: embroidered patch

#[test]
fn patch_border_and_cutting_line_sit_at_their_distances() {
    let mut s = circle_session();
    let before = s.design().clone();
    let r = run(
        &mut s,
        "wizard.patch",
        json!({"ids": [1], "width": 3, "density": 4.5, "distance": 5, "cutDistance": 3}),
    );
    let new = ids(&r);
    assert_eq!(new.len(), 2);
    let d = s.design().clone();
    // The patch is sewn after the pattern, border before cutting line.
    assert_eq!(
        d.objects.iter().map(|o| o.id).collect::<Vec<_>>(),
        [1, new[0], new[1]]
    );
    assert_eq!(d.objects[1].group, d.objects[2].group);
    // Border centre line: 10 + 5 + 1.5 mm; cutting line: 10 + 5 + 3 + 3 mm.
    for p in points_mm(&d, new[0]) {
        assert!((p.distance(v2(50.0, 50.0)) - 16.5).abs() < 0.05, "{p:?}");
    }
    for p in points_mm(&d, new[1]) {
        assert!((p.distance(v2(50.0, 50.0)) - 21.0).abs() < 0.05, "{p:?}");
    }
    let pattern = compile(&d);
    assert_eq!(
        block_names(&pattern),
        ["Red", "Black", "Patch cutting line"]
    );
    assert_eq!(pattern.count(Command::ColorChange), 2);
    // The satin covers 15…18 mm from the centre; its density is 4.5 lines/mm.
    let blocks = needle_blocks(&pattern);
    let radii: Vec<f64> = blocks[1]
        .iter()
        .map(|p| p.distance(v2(50.0, 50.0)))
        .collect();
    let (lo, hi) = radii
        .iter()
        .fold((f64::MAX, 0.0f64), |(a, b), &r| (a.min(r), b.max(r)));
    assert!(
        (lo - 15.0).abs() < 0.1 && (hi - 18.0).abs() < 0.1,
        "{lo} {hi}"
    );
    let circumference = 2.0 * std::f64::consts::PI * 16.5;
    let swings = blocks[1].len() as f64;
    assert!(
        (swings / circumference - 4.5).abs() < 0.5,
        "{} lines/mm",
        swings / circumference
    );
    assert_one_undo(&mut s, &before);
}

#[test]
fn patch_can_cut_with_needles_or_skip_the_border() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 30, "y": 30, "width": 40, "height": 20, "fill": "Blue"}),
    );
    run(
        &mut s,
        "wizard.patch",
        json!({"ids": [1], "cutting": "cutting", "color": "Red"}),
    );
    let d = s.design();
    let cut = d.objects[2].line.as_ref().expect("line");
    assert_eq!(cut.kind.id(), "cutting");
    assert!(cut.leave_uncut);
    let names = block_names(&compile(d));
    assert_eq!(names[..2], ["Blue", "Red"]);
    assert!(names[2..].iter().all(|n| n.starts_with("Cutwork needle")));
    // Rounded corners: the cut outline has more than four needle directions' worth of pieces.
    assert!(names.len() >= 5, "{names:?}");

    run(&mut s, "history.undo", json!({}));
    let r = run(
        &mut s,
        "wizard.patch",
        json!({"ids": [1], "border": false, "cutDistance": 4}),
    );
    let new = ids(&r);
    assert_eq!(new.len(), 1);
    // Without a border the cutting line is 4 mm from the rectangle.
    let b = s.design().objects[1].bounds().expect("bounds");
    assert!(
        (b.min_x - 260.0).abs() < 0.5 && (b.max_y - 540.0).abs() < 0.5,
        "{b:?}"
    );
}

#[test]
fn bad_patch_parameters_fail_cleanly() {
    let mut s = circle_session();
    let before = s.design().clone();
    let bad: Vec<Value> = vec![
        json!({}),
        json!({"ids": []}),
        json!({"ids": [7]}),
        json!({"ids": [1], "border": "yes"}),
        json!({"ids": [1], "width": 0}),
        json!({"ids": [1], "width": 11}),
        json!({"ids": [1], "density": 0.5}),
        json!({"ids": [1], "density": 1e308}),
        json!({"ids": [1], "distance": -1}),
        json!({"ids": [1], "distance": "5"}),
        json!({"ids": [1], "cutting": "scissors"}),
        json!({"ids": [1], "cutDistance": 21}),
        json!({"ids": [1], "color": "#gg0000"}),
        json!({"ids": [1], "border": false, "cutting": "none"}),
    ];
    for params in bad {
        let r = s.execute("wizard.patch", Some(&params));
        assert!(r.is_err(), "{params} should fail");
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
}

// MARK: cutwork

#[test]
fn cutwork_stages_cut_lines_and_needle_blocks() {
    let mut s = circle_session();
    let before = s.design().clone();
    let r = run(
        &mut s,
        "wizard.cutwork",
        json!({"ids": [1], "color": "White", "netPattern": 2, "netSpacing": 3, "netDirection": 30}),
    );
    assert_eq!(r["removed"], json!([1]));
    let new = ids(&r);
    let d = s.design().clone();
    let names: Vec<&str> = d.objects.iter().filter_map(|o| o.name.as_deref()).collect();
    assert_eq!(
        names,
        [
            "Cutwork stabilizing run",
            "Cutwork cutting line",
            "Cutwork stabilizer run",
            "Cutwork net fill",
            "Cutwork cover underlay",
            "Cutwork cover"
        ]
    );
    assert_eq!(d.objects.iter().map(|o| o.id).collect::<Vec<_>>(), new);
    // Distances from the cut (r = 10 mm): stabilizing run +1 mm, cut on the outline,
    // stabilizer run +0.5 mm, net region +1 mm (a quarter of the 4 mm satin).
    for (i, r) in [
        (0, 11.0),
        (1, 10.0),
        (2, 10.5),
        (3, 11.0),
        (4, 10.0),
        (5, 10.0),
    ] {
        for p in points_mm(&d, new[i]) {
            assert!(
                (p.distance(v2(50.0, 50.0)) - r).abs() < 0.05,
                "stage {i}: {p:?}"
            );
        }
    }
    let net = d.objects[3].region.as_ref().expect("net");
    assert_eq!(net.kind.id(), "netFill");
    assert_eq!(net.pattern.as_deref(), Some("diamond"));
    assert_eq!((net.spacing, net.angle), (Some(3.0), 30.0));
    let cover = d.objects[5].line.as_ref().expect("cover");
    assert_eq!((cover.width, cover.density), (4.0, 6.5));

    // Colour blocks: the run, the four needles (a circle has edges in every direction), then
    // the stabilizer run, net and satin in one block: five colour changes, i.e. stops.
    let pattern = compile(&d);
    assert_eq!(
        block_names(&pattern),
        [
            "White",
            "Cutwork needle 1 (0°)",
            "Cutwork needle 2 (45°)",
            "Cutwork needle 3 (90°)",
            "Cutwork needle 4 (135°)",
            "White"
        ]
    );
    assert_eq!(pattern.count(Command::ColorChange), 5);
    let blocks = needle_blocks(&pattern);
    // Cut points lie on the outline; net stitches stay within 1 mm outside it; the satin
    // covers 8…12 mm.
    for b in &blocks[1..5] {
        for p in b {
            assert!((p.distance(v2(50.0, 50.0)) - 10.0).abs() < 0.05, "{p:?}");
        }
    }
    let last = &blocks[5];
    let far = last
        .iter()
        .map(|p| p.distance(v2(50.0, 50.0)))
        .fold(0.0, f64::max);
    assert!((far - 12.0).abs() < 0.1, "{far}");
    assert!(
        last.iter().any(|p| p.distance(v2(50.0, 50.0)) < 5.0),
        "the net crosses the cutout"
    );
    assert_one_undo(&mut s, &before);
}

#[test]
fn cutwork_options_can_be_turned_off_and_cut_by_hand() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 20, "y": 20, "width": 30, "height": 20, "fill": "Blue"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 60, "y": 20, "width": 20, "height": 20, "fill": "Blue"}),
    );
    let r = run(
        &mut s,
        "wizard.cutwork",
        json!({"ids": [1, 2], "cutting": "running", "stabilize": false, "secureStabilizer": false, "netFill": false, "cover": false}),
    );
    assert_eq!(ids(&r).len(), 1);
    let d = s.design();
    assert_eq!(d.objects.len(), 1);
    // Both cutouts in one cutting line, on a named thread.
    assert_eq!(d.objects[0].contours.len(), 2);
    let pattern = compile(d);
    assert_eq!(block_names(&pattern), ["Cutwork cutting line"]);

    // Rectangles only need the 0° and 90° needles.
    run(&mut s, "history.undo", json!({}));
    run(
        &mut s,
        "wizard.cutwork",
        json!({"ids": [1, 2], "stabilize": false, "secureStabilizer": false, "netFill": false, "cover": false}),
    );
    assert_eq!(
        block_names(&compile(s.design())),
        ["Cutwork needle 1 (0°)", "Cutwork needle 3 (90°)"]
    );
}

#[test]
fn bad_cutwork_parameters_fail_cleanly() {
    let mut s = circle_session();
    // 2: open line, 3: text, 4: a bow tie that crosses itself.
    run(
        &mut s,
        "shape.path",
        json!({"points": [[0, 0], [10, 10]], "closed": false}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "I", "x": 80, "y": 90, "size": 10}),
    );
    run(
        &mut s,
        "shape.polygon",
        json!({"points": [[5, 70], [25, 90], [25, 70], [5, 90]]}),
    );
    let before = s.design().clone();
    let bad: Vec<Value> = vec![
        json!({}),
        json!({"ids": []}),
        json!({"ids": [99]}),
        json!({"ids": [2]}),
        json!({"ids": [1, 3]}),
        json!({"ids": [4]}),
        json!({"ids": [1], "stabilize": 1}),
        json!({"ids": [1], "cutting": "laser"}),
        json!({"ids": [1], "secureStabilizer": null, "netPattern": 0}),
        json!({"ids": [1], "netPattern": 6}),
        json!({"ids": [1], "netPattern": 1.5}),
        json!({"ids": [1], "netSpacing": 1}),
        json!({"ids": [1], "netSpacing": 11}),
        json!({"ids": [1], "netDirection": -1}),
        json!({"ids": [1], "netDirection": 91}),
        json!({"ids": [1], "coverWidth": 2}),
        json!({"ids": [1], "coverWidth": 7}),
        json!({"ids": [1], "coverDensity": 4}),
        json!({"ids": [1], "coverDensity": "6"}),
        json!({"ids": [1], "color": []}),
    ];
    for params in bad {
        let r = s.execute("wizard.cutwork", Some(&params));
        assert!(r.is_err(), "{params} should fail");
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
}
