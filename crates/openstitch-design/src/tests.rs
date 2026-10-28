use crate::Compiler;
use crate::generate::{self, RegionParams};
use crate::geometry::{self, Contour, Transform, v2};
use crate::model::*;
use openstitch_core::{Command, StitchPattern, Thread};

const RED: Thread = Thread::rgb(0xED171F);
const BLUE: Thread = Thread::rgb(0x0A55A3);

fn params(angle: f64, spacing: f64, len: f64, pull: f64) -> RegionParams {
    RegionParams {
        angle,
        row_spacing: spacing,
        stitch_length: len,
        pull_compensation: pull,
    }
}

#[test]
fn scanlines_of_square_are_evenly_spaced() {
    let rows = geometry::scan_rows(
        &[Contour::rectangle(0.0, 0.0, 100.0, 100.0)],
        0.0,
        10.0,
        &[],
    );
    assert_eq!(rows.len(), 10);
    assert_eq!(rows[0].y, 5.0);
    assert!(rows.iter().all(|r| r.spans == vec![0.0..=100.0]));
}

#[test]
fn hole_contour_splits_rows() {
    let outer = Contour::rectangle(0.0, 0.0, 100.0, 100.0);
    let hole = Contour::rectangle(40.0, 40.0, 20.0, 20.0);
    let rows = geometry::scan_rows(&[outer.clone(), hole.clone()], 0.0, 10.0, &[]);
    let middle = rows.iter().find(|r| r.y == 45.0).expect("row at 45");
    assert_eq!(middle.spans, vec![0.0..=40.0, 60.0..=100.0]);
    assert!(!geometry::contains(
        v2(50.0, 50.0),
        &[outer.clone(), hole.clone()]
    ));
    assert!(geometry::contains(v2(10.0, 50.0), &[outer, hole]));
}

#[test]
fn span_subtraction() {
    assert_eq!(
        geometry::subtract(&[2.0..=3.0, 5.0..=20.0], &[0.0..=10.0]),
        vec![0.0..=2.0, 3.0..=5.0]
    );
    assert_eq!(geometry::subtract(&[], &[0.0..=10.0]), vec![0.0..=10.0]);
    assert!(geometry::subtract(&[-5.0..=15.0], &[0.0..=10.0]).is_empty());
}

#[test]
fn rotated_rows_map_back_inside() {
    let rows = geometry::scan_rows(
        &[Contour::rectangle(0.0, 0.0, 100.0, 100.0)],
        45.0,
        10.0,
        &[],
    );
    for r in &rows {
        for s in &r.spans {
            for x in [*s.start(), *s.end()] {
                let p = geometry::from_rotated(v2(x, r.y), 45.0);
                assert!(
                    p.x > -1e-3 && p.x < 100.001 && p.y > -1e-3 && p.y < 100.001,
                    "{p:?}"
                );
            }
        }
    }
}

#[test]
fn transforms() {
    let r = Transform::rotation(90.0, v2(0.0, 0.0)).apply(v2(10.0, 0.0));
    assert!(r.x.abs() < 1e-9 && (r.y - 10.0).abs() < 1e-9);
    assert_eq!(
        Transform::scaling(2.0, 3.0, v2(10.0, 10.0)).apply(v2(20.0, 20.0)),
        v2(30.0, 40.0)
    );
}

#[test]
fn running_keeps_corners_and_pitch() {
    let run = generate::running(&Contour::rectangle(0.0, 0.0, 100.0, 50.0), 25.0, 1);
    for c in [v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 50.0), v2(0.0, 50.0)] {
        assert!(run.contains(&c));
    }
    assert_eq!(run.first(), run.last());
    assert!(run.windows(2).all(|w| w[0].distance(w[1]) <= 25.0 + 1e-9));
    assert_eq!(run.len(), 13);
}

#[test]
fn running_passes_and_triple() {
    let line = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0)], false);
    let xs: Vec<f64> = generate::running(&line, 50.0, 3)
        .iter()
        .map(|p| p.x)
        .collect();
    assert_eq!(xs, [0.0, 50.0, 100.0, 50.0, 0.0, 50.0, 100.0]);
    let short = Contour::new(vec![v2(0.0, 0.0), v2(60.0, 0.0)], false);
    let xs: Vec<f64> = generate::triple(&short, 30.0).iter().map(|p| p.x).collect();
    assert_eq!(xs, [0.0, 30.0, 0.0, 30.0, 60.0, 30.0, 60.0]);
}

#[test]
fn zigzag_alternates_sides() {
    let run = generate::zigzag(
        &Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0)], false),
        20.0,
        5.0,
    );
    assert_eq!(run.len(), 21);
    assert!(
        run.windows(2)
            .all(|w| w[0].y == -w[1].y && w[0].y.abs() == 10.0)
    );
}

#[test]
fn fill_inside_rows_bounded_and_staggered() {
    let runs = generate::fill(
        &[Contour::rectangle(0.0, 0.0, 200.0, 100.0)],
        &params(0.0, 4.0, 40.0, 0.0),
        &[],
    );
    assert_eq!(runs.len(), 1);
    let pts: Vec<_> = runs.concat();
    assert!(
        pts.iter()
            .all(|p| p.x >= -1e-9 && p.x <= 200.0 + 1e-9 && (0.0..=100.0).contains(&p.y))
    );
    let mut ys: Vec<i64> = pts.iter().map(|p| (p.y * 1000.0) as i64).collect();
    ys.dedup();
    ys.sort();
    ys.dedup();
    assert_eq!(ys.len(), 25);
    assert!(
        pts.windows(2)
            .filter(|w| w[0].y == w[1].y)
            .all(|w| w[0].distance(w[1]) <= 40.0 + 1e-9)
    );
    let row = |y: f64| -> Vec<f64> {
        pts.iter()
            .filter(|p| p.y == y)
            .map(|p| p.x)
            .filter(|x| *x != 0.0 && *x != 200.0)
            .collect()
    };
    let (r0, r1) = (row(2.0), row(6.0));
    assert!(r0.iter().all(|x| !r1.contains(x)));
}

#[test]
fn separated_parts_become_separate_runs() {
    let shape = [
        Contour::rectangle(0.0, 0.0, 50.0, 50.0),
        Contour::rectangle(200.0, 0.0, 50.0, 50.0),
    ];
    assert_eq!(
        generate::fill(&shape, &params(0.0, 4.0, 30.0, 0.0), &[]).len(),
        2
    );
}

#[test]
fn satin_one_needle_point_per_row_with_pull_compensation() {
    let runs = generate::satin(
        &[Contour::rectangle(0.0, 0.0, 40.0, 20.0)],
        &params(90.0, 4.0, 40.0, 2.0),
        &[],
    );
    let pts: Vec<_> = runs.concat();
    // Ten rows: the first sewn across, then one needle point per row on alternate edges.
    assert_eq!(pts.len(), 11);
    for w in pts.windows(2) {
        assert!(
            ((w[0].y - w[1].y).abs() - 24.0).abs() < 1e-9,
            "{w:?} runs along an edge"
        );
        assert!((w[0].x - w[1].x).abs() <= 4.0 + 1e-9);
    }
    let ys: Vec<f64> = pts
        .iter()
        .map(|p| (p.y * 1000.0).round() / 1000.0)
        .collect();
    assert_eq!(ys.iter().cloned().fold(f64::INFINITY, f64::min), -2.0);
    assert_eq!(ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max), 22.0);
}

#[test]
fn deterministic() {
    let shape = [Contour::ellipse(v2(100.0, 100.0), 80.0, 50.0, 72)];
    let p = params(30.0, 4.0, 35.0, 1.0);
    assert_eq!(
        generate::fill(&shape, &p, &[]),
        generate::fill(&shape, &p, &[])
    );
}

fn two_squares() -> Design {
    let mut d = Design::new("two", DesignPage::default());
    d.add(DesignObject::shape(
        vec![Contour::rectangle(0.0, 0.0, 100.0, 100.0)],
        Some(RegionSewing::new(RegionType::Fill, RED)),
        None,
    ));
    d.add(DesignObject::shape(
        vec![Contour::rectangle(300.0, 0.0, 100.0, 100.0)],
        Some(RegionSewing::new(RegionType::Fill, BLUE)),
        Some(LineSewing::new(LineType::Running, BLUE)),
    ));
    d
}

#[test]
fn compiler_colour_changes_trims_end_and_max_stitch() {
    let p = Compiler::default().compile(&two_squares());
    assert_eq!(p.threads, vec![RED, BLUE]);
    assert_eq!(p.count(Command::ColorChange), 1);
    assert_eq!(p.stitches.last().map(|s| s.command), Some(Command::End));
    assert_eq!(p.stitches.first().map(|s| s.command), Some(Command::Jump));
    assert!(p.count(Command::Trim) >= 2);
    let max = p
        .stitches
        .windows(2)
        .filter(|w| w[1].command == Command::Stitch)
        .map(|w| v2(w[0].x, w[0].y).distance(v2(w[1].x, w[1].y)))
        .fold(0.0, f64::max);
    assert!(max <= 120.0 + 1e-9);
}

#[test]
fn short_moves_between_objects_are_sewn_across_and_longer_ones_trimmed() {
    // PE-DESIGN 11.50 sews a 2.24 mm needle-to-needle move between same-colour parts as a
    // plain stitch and trims a 2.33 mm one (compat machine-trim-fine, machine-trim-bound).
    let squares = |gap: f64| {
        let mut d = Design::new("gap", DesignPage::default());
        for x in [0.0, 100.0 + gap] {
            let mut fill = RegionSewing::new(RegionType::Fill, RED);
            fill.underlay = false;
            // Entry on the left edge, exit on the right: the move between the squares is the gap.
            fill.entry = Some(v2(x, 50.0));
            fill.exit = Some(v2(x + 100.0, 50.0));
            d.add(DesignObject::shape(
                vec![Contour::rectangle(x, 0.0, 100.0, 100.0)],
                Some(fill),
                None,
            ));
        }
        d
    };
    for (gap, trims) in [(10.0, 0), (22.0, 0), (24.0, 1), (60.0, 1)] {
        let p = Compiler::default().compile(&squares(gap));
        assert_eq!(p.count(Command::ColorChange), 0);
        // One trim at the end of the design, plus the one between the squares.
        assert_eq!(p.count(Command::Trim), trims + 1, "gap {gap}");
        let interior_jumps = p
            .stitches
            .iter()
            .skip(1)
            .filter(|s| s.command == Command::Jump)
            .count();
        assert_eq!(
            interior_jumps, trims,
            "gap {gap}: a short move is a stitch, not a jump"
        );
    }
}

#[test]
fn no_zero_length_stitch_where_a_jump_lands() {
    // The jump's end is the first needle point. PE-DESIGN 11 never writes a zero-length stitch
    // there and drops one on open (compat pes-default-v6: 932 → 930 stitches).
    let p = Compiler::default().compile(&two_squares());
    assert!(p.count(Command::Jump) >= 2);
    for w in p.stitches.windows(2) {
        if w[0].command == Command::Jump && w[1].command == Command::Stitch {
            assert_ne!((w[0].x, w[0].y), (w[1].x, w[1].y), "{w:?}");
        }
    }
}

#[test]
fn hidden_objects_are_not_sewn() {
    let mut d = two_squares();
    d.objects[1].visible = false;
    assert_eq!(Compiler::default().compile(&d).threads, vec![RED]);
}

#[test]
fn imported_stitches_keep_colour_blocks() {
    let mut p = StitchPattern {
        threads: vec![RED, BLUE],
        ..Default::default()
    };
    p.stitch_to(0.0, 0.0);
    p.stitch_to(10.0, 0.0);
    p.push(Command::ColorChange);
    p.stitch_to(10.0, 10.0);
    p.stitch_to(0.0, 10.0);
    p.push(Command::End);
    let mut d = Design::default();
    d.add(DesignObject::stitches(StitchBlock::from(&p)));
    let out = Compiler::default().compile(&d);
    assert_eq!(out.threads, vec![RED, BLUE]);
    // Each block's first point is where its jump lands, not a zero-length stitch after it
    // (PE-DESIGN 11 counts it the same way, compat pes-default-v6); every point is still there.
    assert_eq!(out.count(Command::Stitch), 2);
    let points: Vec<(f64, f64)> = out
        .stitches
        .iter()
        .filter(|s| matches!(s.command, Command::Stitch | Command::Jump))
        .map(|s| (s.x, s.y))
        .collect();
    assert_eq!(points, [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
}

#[test]
fn empty_design_compiles_to_nothing() {
    assert!(
        Compiler::default()
            .compile(&Design::default())
            .stitches
            .is_empty()
    );
}

#[test]
fn hole_sewing_skips_upper_shape() {
    let mut d = Design::default();
    let base = d.add(DesignObject::shape(
        vec![Contour::rectangle(0.0, 0.0, 200.0, 200.0)],
        Some(RegionSewing::new(RegionType::Fill, RED)),
        None,
    ));
    d.add(DesignObject::shape(
        vec![Contour::rectangle(50.0, 50.0, 100.0, 100.0)],
        Some(RegionSewing::new(RegionType::Fill, BLUE)),
        None,
    ));
    let inside =
        |s: &openstitch_core::Stitch| s.x > 51.0 && s.x < 149.0 && s.y > 51.0 && s.y < 149.0;
    let first_block = |d: &Design| -> Vec<openstitch_core::Stitch> {
        Compiler::default().compile(d).color_blocks()[0]
            .1
            .iter()
            .filter(|s| s.command == Command::Stitch)
            .copied()
            .collect()
    };
    let plain = first_block(&d);
    assert!(plain.iter().any(inside));
    let i = d.index_of(base).expect("base");
    d.objects[i].hole_sewing = true;
    let holed = first_block(&d);
    assert!(!holed.iter().any(inside));
    assert!(holed.len() < plain.len());
}

#[test]
fn design_json_uses_osd_field_names() {
    let json = serde_json::to_value(two_squares()).expect("serialize");
    assert_eq!(json["nextObjectID"], 3);
    assert_eq!(json["objects"][0]["region"]["type"], "fill");
    assert_eq!(json["objects"][0]["region"]["stitchLength"], 4.0);
    let back: Design = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, two_squares());
}

// MARK: Bézier segments

/// Circle of radius `r` as four cubic segments (handle length k·r, k = 0.5523).
fn bezier_circle(c: crate::Vec2, r: f64) -> Contour {
    use crate::Bezier;
    let k = 0.552_284_75 * r;
    let pts = vec![
        v2(c.x + r, c.y),
        v2(c.x, c.y + r),
        v2(c.x - r, c.y),
        v2(c.x, c.y - r),
    ];
    let mut contour = Contour::new(pts, true);
    let handles = [
        (v2(c.x + r, c.y + k), v2(c.x + k, c.y + r)),
        (v2(c.x - k, c.y + r), v2(c.x - r, c.y + k)),
        (v2(c.x - r, c.y - k), v2(c.x - k, c.y - r)),
        (v2(c.x + k, c.y - r), v2(c.x + r, c.y - k)),
    ];
    for (i, (c1, c2)) in handles.into_iter().enumerate() {
        contour.set_curve(i, Some(Bezier { c1, c2 }));
    }
    contour
}

#[test]
fn bezier_circle_flattens_within_tolerance() {
    let centre = v2(500.0, 500.0);
    let circle = bezier_circle(centre, 200.0);
    let path = circle.path();
    assert!(path.len() > 20, "curves are subdivided");
    assert_eq!(path.first(), path.last());
    for w in path.windows(2) {
        let mid = w[0].lerp(w[1], 0.5);
        for q in [w[0], mid] {
            assert!((q.distance(centre) - 200.0).abs() < 0.6, "{q:?}");
        }
    }
}

#[test]
fn straight_contours_are_unchanged_and_serialize_without_curves() {
    let r = Contour::rectangle(0.0, 0.0, 10.0, 10.0);
    assert_eq!(r.path().len(), 5);
    let json = serde_json::to_value(&r).expect("json");
    assert!(json.get("curves").is_none());
    let mut curved = r.clone();
    curved.set_curve(
        1,
        Some(crate::Bezier {
            c1: v2(15.0, 3.0),
            c2: v2(15.0, 7.0),
        }),
    );
    let json = serde_json::to_value(&curved).expect("json");
    assert_eq!(json["curves"].as_array().map(Vec::len), Some(4));
    let back: Contour = serde_json::from_value(json).expect("back");
    assert_eq!(back, curved);
    curved.set_curve(1, None);
    assert!(
        curved.curves.is_empty(),
        "clearing the last curve empties the list"
    );
}

#[test]
fn bounds_include_curve_bulge() {
    let mut c = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0)], false);
    c.set_curve(
        0,
        Some(crate::Bezier {
            c1: v2(0.0, -60.0),
            c2: v2(100.0, -60.0),
        }),
    );
    let o = DesignObject::shape(vec![c], None, None);
    let b = o.bounds().expect("bounds");
    assert!((b.min_y + 45.0).abs() < 0.6, "{}", b.min_y);
}

#[test]
fn running_on_curves_keeps_vertices_and_pitch() {
    let circle = bezier_circle(v2(0.0, 0.0), 200.0);
    let run = generate::running(&circle, 25.0, 1);
    for v in &circle.points {
        assert!(run.contains(v), "vertex {v:?} kept");
    }
    assert!(run.windows(2).all(|w| w[0].distance(w[1]) <= 25.0 + 0.5));
    let circumference = std::f64::consts::TAU * 200.0;
    let expected = (circumference / 25.0).ceil() as usize;
    assert!(
        (run.len() as isize - 1 - expected as isize).abs() <= 4,
        "{} stitches",
        run.len() - 1
    );
    let triple = generate::triple(&circle, 25.0);
    assert_eq!(triple.len(), 1 + 3 * (run.len() - 1));
}

#[test]
fn fill_stays_inside_curved_region() {
    let circle = bezier_circle(v2(300.0, 300.0), 150.0);
    let p = params(30.0, 4.0, 35.0, 0.0);
    let pts: Vec<_> = generate::fill(std::slice::from_ref(&circle), &p, &[]).concat();
    assert!(!pts.is_empty());
    assert!(
        pts.iter()
            .all(|q| q.distance(v2(300.0, 300.0)) <= 150.0 + 0.6)
    );
}

#[test]
fn insert_point_splits_curves_exactly() {
    let mut c = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0)], false);
    let b = crate::Bezier {
        c1: v2(0.0, -60.0),
        c2: v2(100.0, -60.0),
    };
    c.set_curve(0, Some(b));
    let before = c.path();
    let i = c.insert_point(0, 0.5);
    assert_eq!(i, 1);
    assert_eq!(c.points.len(), 3);
    assert!((c.points[1].x - 50.0).abs() < 1e-9 && (c.points[1].y + 45.0).abs() < 1e-9);
    assert!(c.curve(0).is_some() && c.curve(1).is_some());
    // Every new flattened point lies on the original curve (within flattening tolerance).
    for q in c.path() {
        let nearest = before
            .windows(2)
            .map(|w| seg_dist(q, w[0], w[1]))
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 1.0, "{q:?} off curve by {nearest}");
    }
    let mut straight = Contour::rectangle(0.0, 0.0, 100.0, 100.0);
    assert_eq!(straight.insert_point(3, 0.25), 4);
    assert_eq!(straight.points[4], v2(0.0, 75.0));
    assert!(straight.curves.is_empty());
}

fn seg_dist(p: crate::Vec2, a: crate::Vec2, b: crate::Vec2) -> f64 {
    let ab = b - a;
    let t = (((p - a).x * ab.x + (p - a).y * ab.y) / (ab.x * ab.x + ab.y * ab.y)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[test]
fn delete_point_keeps_curve_bookkeeping() {
    let mut circle = bezier_circle(v2(0.0, 0.0), 100.0);
    circle.delete_point(0);
    assert_eq!(circle.points.len(), 3);
    assert_eq!(circle.segment_count(), 3);
    assert_eq!(circle.curves.len(), 3);
    assert!(circle.curves.iter().all(Option::is_some));
    // The joined closing segment keeps the outer handles of its two neighbours.
    let joined = circle.curve(2).expect("closing curve");
    let k = 0.552_284_75 * 100.0;
    // Removed vertex (r, 0) sat between handles (k, -r) and (k, r); the outer ones remain.
    assert!(joined.c1.distance(v2(k, -100.0)) < 1e-9, "{joined:?}");
    assert!(joined.c2.distance(v2(k, 100.0)) < 1e-9, "{joined:?}");
    let mut square = Contour::rectangle(0.0, 0.0, 10.0, 10.0);
    square.delete_point(2);
    assert_eq!(
        square.points,
        vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(0.0, 10.0)]
    );
    assert!(square.curves.is_empty());
}

#[test]
fn transform_moves_handles() {
    let c = bezier_circle(v2(0.0, 0.0), 10.0).transformed(&Transform::translation(5.0, 0.0));
    let b = c.curve(0).expect("curve");
    assert!((b.c1.x - 15.0).abs() < 1e-9);
}

#[test]
fn programmable_fill_sews_embossed_and_engraved_tile_areas() {
    use crate::patterns::{PatternRegion, RegionEffect, parse};
    let tile = |regions: &[PatternRegion]| {
        let mut p = parse(
            r#"{"format": "openstitch.pattern", "version": 1, "kind": "fill", "id": "strips",
            "name": "Strips", "paths": [[[0, 0.25], [1, 0.25]], [[0, 0.75], [1, 0.75]]]}"#,
        )
        .expect("valid");
        p.regions = regions.to_vec();
        p
    };
    let sew = |regions: &[PatternRegion]| {
        let mut d = Design::new(
            "t",
            DesignPage {
                width: 600.0,
                height: 400.0,
            },
        );
        let mut fill = RegionSewing::new(RegionType::ProgrammableFill, Thread::rgb(0));
        fill.pattern = Some("strips".into());
        fill.pattern_width = Some(24.0);
        fill.pattern_height = Some(24.0);
        fill.stitch_length = 2.0;
        d.add(DesignObject::shape(
            vec![Contour::rectangle(20.0, 20.0, 560.0, 360.0)],
            Some(fill),
            None,
        ));
        d.patterns.push(tile(regions));
        Compiler::default().compile(&d).count(Command::Stitch)
    };
    let at = |effect| PatternRegion {
        effect,
        at: v2(0.5, 0.5),
    };
    let plain = sew(&[]);
    let embossed = sew(&[at(RegionEffect::Emboss)]);
    let engraved = sew(&[at(RegionEffect::Engrave)]);
    assert!(embossed < plain * 9 / 10, "{embossed} vs {plain}");
    assert!(engraved > plain * 11 / 10, "{engraved} vs {plain}");
}
