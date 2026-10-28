//! Tests for the stitch types added after running/triple/zigzag/fill/satin: the pattern
//! library and its format, line types, satin/fill options and region types.

use crate::Compiler;
use crate::generate::Run;
use crate::geometry::{Contour, Vec2, v2};
use crate::lines::{self, MotifLine, StrokeLine};
use crate::model::*;
use crate::patterns::{self, PatternKind};
use openstitch_core::{Command, Thread};

const RED: Thread = Thread::rgb(0xED171F);

fn longest(run: &[Vec2]) -> f64 {
    run.windows(2)
        .map(|w| w[0].distance(w[1]))
        .fold(0.0, f64::max)
}

/// Horizontal open line from (0, 0) to (len, 0).
fn hline(len: f64) -> Vec<Vec2> {
    vec![v2(0.0, 0.0), v2(len, 0.0)]
}

fn circle() -> Contour {
    Contour::ellipse(v2(200.0, 200.0), 150.0, 150.0, 64)
}

// ---------------------------------------------------------------- pattern library

#[test]
fn library_parses_with_every_kind_and_unique_ids() {
    let lib = patterns::library();
    for kind in [
        PatternKind::Motif,
        PatternKind::Fill,
        PatternKind::Decorative,
    ] {
        let ids: Vec<&str> = lib
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| p.id.as_str())
            .collect();
        assert!(ids.len() >= 6, "{kind:?}: {ids:?}");
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
    }
    assert_eq!(
        patterns::find(PatternKind::Motif, "heart").map(|p| p.paths.len()),
        Some(1)
    );
    assert!(patterns::find(PatternKind::Fill, "heart").is_none());
}

#[test]
fn pattern_format_rejects_bad_documents() {
    let ok = r#"{"format":"openstitch.pattern","version":1,"kind":"motif","id":"m","name":"M","paths":[[[0,0],[0.5,1],[1,0]]]}"#;
    assert!(patterns::parse(ok).is_ok());
    for bad in [
        "{}",
        "not json",
        r#"{"format":"other","version":1,"kind":"motif","id":"m","name":"M","paths":[[[0,0],[1,0]]]}"#,
        r#"{"format":"openstitch.pattern","version":2,"kind":"motif","id":"m","name":"M","paths":[[[0,0],[1,0]]]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"motif","id":"m","name":"M","paths":[[[0,0],[0.9,0]]]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"motif","id":"m","name":"M","paths":[[[0,0],[0.5,3],[1,0]]]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"motif","id":"Bad Id","name":"M","paths":[[[0,0],[1,0]]]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"fill","id":"f","name":"F","paths":[]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"fill","id":"f","name":"F","paths":[[[0,0]]]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"decorative","id":"d","name":"D","paths":[[[0,0.2],[1,0.8]]]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"fill","id":"f","name":"F","rowShift":2,"paths":[[[0,0],[1,1]]]}"#,
        r#"{"format":"openstitch.pattern","version":1,"kind":"fill","id":"f","name":"F","extra":1,"paths":[[[0,0],[1,1]]]}"#,
    ] {
        assert!(patterns::parse(bad).is_err(), "{bad}");
    }
    let many = format!(
        r#"{{"format":"openstitch.pattern","version":1,"kind":"fill","id":"f","name":"F","paths":[{}]}}"#,
        vec!["[[0,0],[1,1]]"; 65].join(",")
    );
    assert!(patterns::parse(&many).is_err());
}

// ---------------------------------------------------------------- line types

#[test]
fn motif_line_repeats_inside_its_band_with_short_stitches() {
    let pattern = patterns::find(PatternKind::Motif, "peak").expect("peak");
    let m = MotifLine {
        pattern,
        width: 40.0,
        height: 30.0,
        gap: 10.0,
        pitch: 20.0,
        arrange: Arrange::Normal,
    };
    let run = lines::motif(&hline(500.0), false, &m);
    assert!(longest(&run) <= 20.0 + 1e-9);
    assert!(run.iter().all(|p| p.y.abs() <= 30.0 + 1e-9));
    assert_eq!(run.first().copied(), Some(v2(0.0, 0.0)));
    assert_eq!(run.last().copied(), Some(v2(500.0, 0.0)));
    // (500 + 10) / 50 = 10 peaks, all on one side.
    let peaks = run
        .iter()
        .filter(|p| (p.y.abs() - 27.0).abs() < 1e-6)
        .count();
    assert_eq!(peaks, 10);
    assert!(run.iter().all(|p| p.y <= 1e-9));
    let mirrored = lines::motif(
        &hline(500.0),
        false,
        &MotifLine {
            arrange: Arrange::Mirror,
            ..m
        },
    );
    assert!(mirrored.iter().all(|p| p.y >= -1e-9));
    let alternate = lines::motif(
        &hline(500.0),
        false,
        &MotifLine {
            arrange: Arrange::Alternate,
            ..m
        },
    );
    assert!(alternate.iter().any(|p| p.y > 20.0) && alternate.iter().any(|p| p.y < -20.0));
}

#[test]
fn motif_line_closes_on_a_closed_contour() {
    let c = circle();
    for id in ["wave", "scallop", "heart", "leaf", "loop", "diamond"] {
        let pattern = patterns::find(PatternKind::Motif, id).expect("motif");
        let run = lines::motif(
            &c.path(),
            true,
            &MotifLine {
                pattern,
                width: 40.0,
                height: 30.0,
                gap: 0.0,
                pitch: 15.0,
                arrange: Arrange::Normal,
            },
        );
        assert!(longest(&run) <= 15.0 + 1e-9, "{id}");
        let (a, b) = (run[0], *run.last().expect("points"));
        assert!(a.distance(b) < 1e-6, "{id}: {a:?} {b:?}");
        // Every point within the motif height of the circle.
        assert!(
            run.iter()
                .all(|p| (p.distance(v2(200.0, 200.0)) - 150.0).abs() <= 31.0),
            "{id}"
        );
    }
}

#[test]
fn stem_stitches_slant_across_the_line() {
    // PE-DESIGN's stem: 21 stations (300 / 15 = 20 intervals), each sewn top, bottom, top,
    // bottom, between the line's start and end points.
    let run = lines::stem(&hline(300.0), 20.0, 15.0, 45.0, false);
    assert_eq!(run.len(), 21 * 4 + 2);
    assert_eq!(run.first().copied(), Some(v2(0.0, 0.0)));
    assert_eq!(run.last().copied(), Some(v2(300.0, 0.0)));
    assert!(run.iter().all(|p| p.y.abs() <= 11.0 + 1e-9));
    let slant = 20.0 / 45f64.to_radians().sin();
    assert!((run[1].distance(run[2]) - slant).abs() < 1e-9);
    // 45°: from the top right to the bottom left (y down).
    assert!(run[1].x > run[2].x && run[1].y < run[2].y);
    let single = lines::stem(&hline(300.0), 20.0, 15.0, 90.0, true);
    assert_eq!(single.len(), 21 * 2 + 2);
    assert!(
        (single[1].x - single[2].x).abs() < 1e-9,
        "90° is perpendicular"
    );
}

#[test]
fn candlewick_knots_sit_on_the_line() {
    // Knots at 0, 50, 100, 150, 200 (200 / (20 + 30) = 4 intervals), each a {24/11} star
    // visiting all 24 points of its circle.
    let run = lines::candlewick(&hline(200.0), 20.0, 24, 30.0, 25.0);
    assert!(longest(&run) <= 25.0 + 1e-9);
    assert!(run.iter().all(|p| p.y.abs() <= 10.0 + 1e-9));
    for x in [0.0, 50.0, 100.0, 150.0, 200.0] {
        let mut on: Vec<Vec2> = run
            .iter()
            .copied()
            .filter(|p| (v2(x, 0.0).distance(*p) - 10.0).abs() < 1e-9)
            .collect();
        on.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        on.dedup_by(|a, b| a.distance(*b) < 1e-6);
        assert_eq!(on.len(), 24, "knot at {x}");
    }
}

#[test]
fn e_and_v_strokes_reach_the_stroke_width() {
    let e = StrokeLine {
        interval: 30.0,
        stroke: 25.0,
        pitch: 25.0,
        run_times: 1,
        stroke_times: 2,
        arrange: Arrange::Normal,
    };
    let run = lines::e_stitch(&hline(300.0), &e);
    assert!(longest(&run) <= 25.0 + 1e-9);
    let tips = run
        .iter()
        .filter(|p| (p.y.abs() - 25.0).abs() < 1e-9)
        .count();
    assert_eq!(tips, 11 * 2, "11 strokes, sewn twice");
    assert_eq!(run.last().copied(), Some(v2(300.0, 0.0)));
    let triple = lines::e_stitch(&hline(300.0), &StrokeLine { run_times: 2, ..e });
    assert_eq!(
        triple.last().copied(),
        Some(v2(300.0, 0.0)),
        "even passes end forward"
    );
    // PE-DESIGN's V: one apex at every multiple of the interval, start and end included, its
    // arms meeting the line a fifth of the interval to either side.
    let v = lines::v_stitch(&hline(300.0), &e);
    let apexes: Vec<f64> = v
        .iter()
        .filter(|p| (p.y.abs() - 25.0).abs() < 1e-9)
        .map(|p| p.x)
        .collect();
    assert_eq!(
        apexes,
        (0..=10).map(|k| k as f64 * 30.0).collect::<Vec<_>>()
    );
    assert!(longest(&v) <= 25.0f64.hypot(6.0) + 1e-9);
    assert_eq!(v.first().copied(), Some(v2(0.0, 0.0)));
    assert_eq!(v.last().copied(), Some(v2(300.0, 0.0)));
}

#[test]
fn chain_links_stay_within_their_size() {
    // Triangles: floor(200 / 15) = 13 links, each with its base 20 ahead and 20/3 to either
    // side, sewn twice and then to its left corner: 5 corners per link.
    let run = lines::chain(
        &hline(200.0),
        20.0,
        ChainShape::Triangle,
        2,
        Arrange::Normal,
    );
    let h = 20.0 / 3.0;
    assert!(run.iter().all(|p| p.y.abs() <= h + 1e-9));
    assert!(longest(&run) <= 20.0f64.hypot(h) + 1e-9);
    let corners = run.iter().filter(|p| (p.y.abs() - h).abs() < 1e-9).count();
    assert_eq!(corners, 13 * 5);
    let run = lines::chain(&hline(200.0), 20.0, ChainShape::Diamond, 2, Arrange::Normal);
    assert!(run.iter().all(|p| p.y.abs() <= 10.0 + 1e-9));
    assert!(longest(&run) <= 10.0f64.hypot(10.0) + 1e-9);
    let peaks = run
        .iter()
        .filter(|p| (p.y.abs() - 10.0).abs() < 1e-9)
        .count();
    assert_eq!(peaks, 20 * 2);
}

/// The rectangle of compat line-*-startend-*: (10,10)-(50,10)-(50,30)-(10,30) mm, closed; its
/// bottom edge is sewn towards -x.
fn startend_rect() -> Vec<Vec2> {
    Contour::rectangle(100.0, 100.0, 400.0, 200.0).path()
}

#[test]
fn moved_points_keep_chain_links_in_place_and_start_where_pe_does() {
    let path = startend_rect();
    let at = |x: f64| lines::arc_of(&path, v2(x, 300.0));
    let link = |j: f64| v2(500.0 - j * 400.0 / 13.0, 300.0);
    let line = lines::LoopLine::Chain {
        size: 40.0,
        times: 5,
        arrange: Arrange::Normal,
    };
    let in_place = lines::chain(&path, 40.0, ChainShape::Triangle, 5, Arrange::Normal);
    let same_points = |run: &Run| {
        run.iter()
            .all(|p| in_place.iter().any(|q| q.distance(*p) < 1e-6))
    };
    // Feed: the pattern starts with the link of the exit point (L2 at 25.5 mm: link 7) and
    // goes round to the one before.
    let (feed, lead) = lines::start_loop(&path, line, at(255.0), None).expect("feed");
    assert!(!lead && same_points(&feed));
    assert!(feed[0].distance(link(7.0)) < 1e-6);
    assert_eq!(feed.len(), in_place.len());
    // Run: from the entry point's link (L1 at 29.8 mm: link 6) one stitch on to link 7.
    let (run, lead) = lines::start_loop(&path, line, at(255.0), Some(at(298.0))).expect("run");
    assert!(lead && same_points(&run));
    assert!(run[0].distance(link(6.0)) < 1e-6);
    assert_eq!(&run[1..], &feed[..]);
    // The shorter way may be backwards: L2 at the corner (50, 30), L1 at 23.8 mm (link 8)
    // leads back over links 7 to 0 (line-chain-startend-l2side-run.pes).
    let (run, _) = lines::start_loop(&path, line, at(500.0), Some(at(238.0))).expect("back");
    for (k, j) in (0..=8).rev().enumerate() {
        assert!(run[k].distance(link(f64::from(j))) < 1e-6);
    }
}

#[test]
fn moved_points_start_stem_and_candlewick_at_the_exit_element_and_lead_in_with_run() {
    let path = startend_rect();
    let (exit, entry) = (
        lines::arc_of(&path, v2(255.0, 300.0)),
        Some(lines::arc_of(&path, v2(298.0, 300.0))),
    );
    let stem = lines::LoopLine::Stem {
        width: 20.0,
        spacing: 15.0,
        angle: 45.0,
        single: false,
    };
    // Stations 400/27 apart on the bottom edge: L2's is the 16th (263.0), L1's the 13th (307.4).
    let station = |j: f64| v2(500.0 - j * 400.0 / 27.0, 300.0);
    let (feed, _) = lines::start_loop(&path, stem, exit, None).expect("feed");
    assert!(feed[0].distance(station(16.0)) < 1e-6);
    let (run, lead) = lines::start_loop(&path, stem, exit, entry).expect("run");
    assert!(lead);
    for (k, j) in [13.0, 14.0, 15.0, 16.0].into_iter().enumerate() {
        assert!(run[k].distance(station(j)) < 1e-6);
    }
    assert_eq!(&run[4..], &feed[1..]);
    // Candlewicking starts with the running stitch into L2's knot (centred 4 × 400/7 along
    // the bottom edge, from where that knot begins) and sews that knot last.
    let knots = lines::LoopLine::Candlewick {
        size: 50.0,
        points: 24,
        spacing: 10.0,
        pitch: 25.0,
    };
    let (feed, lead) = lines::start_loop(&path, knots, exit, None).expect("candlewick");
    assert!(lead);
    assert!(feed[0].distance(v2(500.0 - 4.0 * 400.0 / 7.0 + 25.0, 300.0)) < 1e-6);
    let front = v2(500.0 - 4.0 * 400.0 / 7.0 - 25.0, 300.0);
    assert_eq!(
        feed[1..].iter().position(|p| p.distance(front) < 1e-6),
        Some(1)
    );
    assert!(feed.last().expect("stitches").distance(front) < 1e-6);
}

#[test]
fn cutting_lines_go_to_the_needle_matching_their_direction() {
    let square = Contour::rectangle(0.0, 0.0, 100.0, 100.0);
    let needles = lines::cutting(&square.path(), true, false);
    let total = |n: &Vec<Run>| -> f64 {
        n.iter()
            .map(|r| r.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>())
            .sum()
    };
    assert!((total(&needles[0]) - 200.0).abs() < 1e-9);
    assert!((total(&needles[2]) - 200.0).abs() < 1e-9);
    assert!(needles[1].is_empty() && needles[3].is_empty());
    assert!(
        needles
            .iter()
            .flatten()
            .all(|r| longest(r) <= lines::CUT_PITCH + 1e-9)
    );
    let diagonal = lines::cutting(&[v2(0.0, 0.0), v2(100.0, 100.0)], false, false);
    assert!(!diagonal[1].is_empty() && diagonal[0].is_empty());
    let bridged = lines::cutting(&square.path(), true, true);
    let cut: f64 = bridged.iter().map(total).sum();
    assert!(cut < 400.0 - 3.0 * lines::CUT_BRIDGE && cut > 400.0 - 5.0 * lines::CUT_BRIDGE);
}

#[test]
fn auto_satin_ends_on_its_start_point_and_travels_forward_along_the_outline() {
    // The 5 mm band bent through 60° of compat satin-auto-bend-60 (0.1 mm): PE-DESIGN 11.50
    // starts at the first vertex, runs 0.5 mm inside the outer (top) edge to the far end, sews
    // the satin back with one needle point per rung and ends on the start point.
    let band = Contour::new(
        vec![
            v2(0.0, 0.0),
            v2(300.0, 0.0),
            v2(386.6, 150.0),
            v2(343.3, 175.0),
            v2(271.1, 50.0),
            v2(0.0, 50.0),
        ],
        true,
    );
    let mut r = region(RegionType::Satin);
    r.auto_direction = true;
    r.underlay = false;
    r.density = 3.0;
    let run = crate::region::layers(std::slice::from_ref(&band), &r, &[])
        .pop()
        .expect("top")
        .pop()
        .expect("run");
    let last = run[run.len() - 1];
    assert!(last.distance(v2(0.0, 0.0)) < 2.0, "ends at {last:?}");
    // The lead travel (before the first rung, which is at the far end) stays by the top edge.
    let first_rung = run
        .iter()
        .position(|p| p.x > 330.0 && p.y > 100.0)
        .expect("far end reached");
    let lead = &run[..first_rung];
    assert!(lead.len() > 10, "{lead:?}");
    for p in lead.iter().filter(|p| p.x > 50.0 && p.x < 250.0) {
        assert!(
            p.y > 0.0 && p.y < 10.0,
            "lead by the top edge, not {p:?}: {lead:?}"
        );
    }
    // Along the straight leg the needle points alternate between the edges at exactly a
    // spacing (3.33 mm / 10 → 0.333 mm): the rungs are square to the leg and on one lattice.
    let leg: Vec<Vec2> = run[first_rung..]
        .iter()
        .copied()
        .filter(|p| p.x > 10.0 && p.x < 200.0 && (p.y.abs() < 0.5 || (p.y - 50.0).abs() < 0.5))
        .collect();
    assert!(leg.len() > 50, "{}", leg.len());
    for w in leg.windows(2) {
        assert!((w[0].y - w[1].y).abs() > 40.0, "alternate edges: {w:?}");
        assert!(
            ((w[0].x - w[1].x).abs() - 10.0 / 3.0).abs() < 0.05,
            "a spacing apart: {w:?}"
        );
    }
}

#[test]
fn half_stitch_thins_the_inner_side_of_bends() {
    let straight = Contour::new(hline(200.0), false);
    assert_eq!(
        lines::zigzag_half(&straight.path(), 20.0, 4.0),
        crate::generate::zigzag(&straight, 20.0, 4.0),
        "nothing crowds on a straight line"
    );
    let tight = Contour::ellipse(v2(0.0, 0.0), 15.0, 15.0, 48);
    let plain = crate::generate::zigzag(&tight, 20.0, 3.0);
    let half = lines::zigzag_half(&tight.path(), 20.0, 3.0);
    assert_eq!(plain.len(), half.len());
    let near_centre = |r: &Run| r.iter().filter(|p| (p.length() - 15.0).abs() < 0.1).count();
    assert!(near_centre(&half) >= near_centre(&plain) + 5);
}

fn shape_with_line(c: Contour, line: LineSewing) -> Design {
    let mut d = Design::new("t", DesignPage::default());
    d.add(DesignObject::shape(vec![c], None, Some(line)));
    d
}

#[test]
fn every_line_type_compiles_deterministically() {
    for kind in LineType::ALL {
        let d = shape_with_line(circle(), LineSewing::new(kind, RED));
        let a = Compiler::default().compile(&d);
        let b = Compiler::default().compile(&d);
        assert_eq!(a, b, "{kind:?}");
        assert!(a.count(Command::Stitch) > 20, "{kind:?}");
    }
}

#[test]
fn cutting_needles_are_separate_colour_blocks() {
    let d = shape_with_line(
        Contour::new(
            vec![
                v2(0.0, 0.0),
                v2(100.0, 0.0),
                v2(200.0, 100.0),
                v2(200.0, 200.0),
            ],
            false,
        ),
        LineSewing::new(LineType::Cutting, RED),
    );
    let p = Compiler::default().compile(&d);
    assert_eq!(p.threads.len(), 3, "needles 1, 2 and 3");
    assert_eq!(p.count(Command::ColorChange), 2);
    assert!(
        p.threads[0]
            .name
            .as_deref()
            .is_some_and(|n| n.starts_with("Cutwork needle 1"))
    );
}

// ---------------------------------------------------------------- satin and fill options

/// Inside the region (even-odd) or within `tol` of its outline.
fn inside_or_on(p: Vec2, contours: &[Contour], tol: f64) -> bool {
    if crate::geometry::contains(p, contours) {
        return true;
    }
    contours.iter().flat_map(Contour::edges).any(|(a, b)| {
        let d = b - a;
        let l2 = d.x * d.x + d.y * d.y;
        let t = if l2 > 0.0 {
            (((p - a).x * d.x + (p - a).y * d.y) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        p.distance(a + d * t) <= tol
    })
}

/// Every needle point and every stitch midpoint inside the region.
fn assert_inside(runs: &[Run], contours: &[Contour], what: &str) {
    for r in runs {
        for w in r.windows(2) {
            for p in [w[0], w[1], w[0].lerp(w[1], 0.5)] {
                assert!(inside_or_on(p, contours, 0.6), "{what}: {p:?} outside");
            }
        }
    }
}

fn region(kind: RegionType) -> RegionSewing {
    RegionSewing::new(kind, RED)
}

#[test]
fn auto_direction_sews_across_the_narrow_side() {
    let wide = Contour::rectangle(0.0, 0.0, 200.0, 50.0);
    assert_eq!(crate::region::auto_angle(std::slice::from_ref(&wide)), 90.0);
    let tall = Contour::rectangle(0.0, 0.0, 50.0, 200.0);
    assert_eq!(crate::region::auto_angle(std::slice::from_ref(&tall)), 0.0);
    let tilted = wide.transformed(&crate::geometry::Transform::rotation(30.0, v2(0.0, 0.0)));
    assert!((crate::region::auto_angle(std::slice::from_ref(&tilted)) - -60.0).abs() < 0.01);
    let mut r = region(RegionType::Satin);
    r.auto_direction = true;
    let layers = crate::region::layers(std::slice::from_ref(&wide), &r, &[]);
    let runs = layers.last().expect("top");
    // A zigzag stitch crosses the narrow side, slanting one row (1/3 mm) along it.
    let slant = 50.0f64.hypot(10.0 / 3.0);
    for run in runs {
        for w in run.windows(2) {
            let d = w[0].distance(w[1]);
            assert!(d <= slant + 1e-6, "stitch {d} longer than the narrow side");
        }
    }
    assert_inside(runs, std::slice::from_ref(&wide), "auto satin");
    // Holes keep the single direction across the narrow side: a sawtooth whose slanted
    // stitches move one row (two crossings) along it. Travel between the parts (running
    // stitches of at most 2.5 mm) is left out.
    let hole = Contour::rectangle(80.0, 20.0, 40.0, 10.0);
    let holed = crate::region::layers(std::slice::from_ref(&wide), &r, &[hole]);
    let row = 2.0 * 10.0 / r.density;
    for run in holed.last().expect("top") {
        for w in run.windows(2).filter(|w| w[0].distance(w[1]) > 25.0 + 1e-6) {
            let d = w[0] - w[1];
            assert!(d.x.abs() <= row + 1e-6 || d.y.abs() < 1e-6, "{w:?}");
        }
    }
}

#[test]
fn underlay_styles_layer_inside_the_region() {
    let c = vec![circle()];
    let mut r = region(RegionType::Fill);
    r.underlay = true;
    for (style, layers) in [
        (UnderlayStyle::Edge, 2),
        (UnderlayStyle::Zigzag, 2),
        (UnderlayStyle::EdgeZigzag, 3),
        (UnderlayStyle::Double, 3),
    ] {
        r.underlay_style = style;
        let out = crate::region::layers(&c, &r, &[]);
        assert_eq!(out.len(), layers, "{style:?}");
        for l in &out {
            assert_inside(l, &c, &format!("{style:?}"));
        }
        if style != UnderlayStyle::Edge {
            // Rows are inset from the outline.
            let rows = &out[out.len() - 2];
            assert!(
                rows.iter()
                    .flatten()
                    .all(|p| p.distance(v2(200.0, 200.0)) <= 150.0 - 1.0),
                "{style:?}"
            );
        }
    }
    r.underlay_style = UnderlayStyle::Edge;
    let edge = crate::region::layers(&c, &r, &[]);
    assert_eq!(edge[0], vec![crate::generate::running(&c[0], 25.0, 1)]);
}

#[test]
fn gradation_spreads_rows_towards_the_far_side() {
    let c = vec![Contour::rectangle(0.0, 0.0, 100.0, 200.0)];
    let even = crate::region::row_positions(&c, 0.0, 4.0, None);
    let graded =
        crate::region::row_positions(&c, 0.0, 4.0, Some(&crate::Gradation::continuous(25.0)));
    assert_eq!(even.len(), 50);
    assert!(graded.len() < 40 && graded.len() > 25, "{}", graded.len());
    let gaps: Vec<f64> = graded.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.windows(2).all(|g| g[1] >= g[0]));
    assert!(gaps[gaps.len() - 1] > 3.0 * gaps[0]);
    let mut r = region(RegionType::Satin);
    r.gradation = Some(crate::Gradation::continuous(25.0));
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert_inside(&runs, &c, "gradation");
}

#[test]
fn gradation_pattern_sews_rows_at_equal_steps_of_the_cumulative_line_count() {
    // The U of compat region-travel-u-gradation: 30 mm tall at 4.5 lines/mm with PE-DESIGN's
    // default pattern (53 % at the top falling linearly to 5 % at the bottom). PE-DESIGN 11.50
    // sews 40 rows; a 30 × 30 mm square has the same rows.
    let c = vec![Contour::rectangle(100.0, 100.0, 300.0, 300.0)];
    let spacing = 10.0 / 4.5;
    let g = crate::Gradation::PE_DEFAULT;
    let ys = crate::region::row_positions(&c, 0.0, spacing, Some(&g));
    assert_eq!(ys.len(), 40, "{ys:?}");
    // First row on the start edge, last on the far edge (sampled just inside).
    assert!(ys[0] < 100.01 && ys[39] > 400.0 - 0.01, "{ys:?}");
    // PE-DESIGN's rows (page mm, from its reference file), every one within 0.12 mm.
    let pe = [
        10.0, 10.4, 10.8, 11.3, 11.7, 12.2, 12.6, 13.1, 13.5, 14.0, 14.5, 15.0, 15.5, 16.0, 16.5,
        17.0, 17.6, 18.1, 18.7, 19.3, 19.9, 20.5, 21.1, 21.8, 22.4, 23.1, 23.8, 24.6, 25.3, 26.1,
        27.0, 27.9, 28.8, 29.8, 30.9, 32.1, 33.5, 35.1, 37.1, 40.0,
    ];
    for (y, p) in ys.iter().zip(pe) {
        assert!((y / 10.0 - p).abs() <= 0.12, "{y} vs {p}: {ys:?}");
    }
    // Equal steps of the cumulative line count: the density integrated over every gap is the
    // same, 39 gaps for the 39.15 lines the profile integrates to.
    let lines = |y0: f64, y1: f64| {
        let f = |y: f64| g.fraction((y - 100.0) / 300.0);
        (y1 - y0) * (f(y0) + f(y1)) / 2.0 / spacing
    };
    let per_gap = lines(ys[0], ys[1]);
    assert!((per_gap - 39.15 / 39.0).abs() < 0.01, "{per_gap}");
    for w in ys.windows(2) {
        assert!((lines(w[0], w[1]) - per_gap).abs() < 1e-3, "{w:?}");
    }
    assert!((g.fraction(0.5) - 0.29).abs() < 1e-12);
    let mut r = region(RegionType::Fill);
    r.gradation = Some(crate::Gradation::PE_DEFAULT);
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert_inside(&runs, &c, "gradation pattern");
}

#[test]
fn satin_at_the_minimum_density_crosses_on_every_line() {
    // The 40 x 6 mm column of compat sweep-satin-density-min at 0.1 lines/mm, start point at
    // the middle of the bottom edge: PE-DESIGN 11.50 sews crossings 10 mm apart (every line)
    // with 11.7 mm diagonals, not 20 mm apart with 20.9 mm ones.
    let c = vec![Contour::rectangle(100.0, 100.0, 400.0, 60.0)];
    let mut r = region(RegionType::Satin);
    r.angle = 90.0;
    r.density = 0.1;
    r.underlay = false;
    r.entry = Some(v2(300.0, 160.0));
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    let run = &runs[0];
    let mut crossings: Vec<f64> = run
        .windows(2)
        .filter(|w| (w[0].x - w[1].x).abs() < 0.01 && (w[0].y - w[1].y).abs() > 59.0)
        .map(|w| w[0].x)
        .collect();
    crossings.sort_by(f64::total_cmp);
    assert_eq!(
        crossings,
        [100.0, 200.0, 300.0, 300.0, 400.0, 500.0],
        "{run:?}"
    );
    let longest = run
        .windows(2)
        .map(|w| w[0].distance(w[1]))
        .fold(0.0_f64, f64::max);
    assert!((longest - 116.6).abs() < 0.5, "{longest}");
    // At 3 lines/mm the sawtooth skips a line: crossings 0.67 mm apart.
    r.density = 3.0;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    let xs: Vec<f64> = runs[0]
        .windows(2)
        .filter(|w| (w[0].x - w[1].x).abs() < 0.01 && (w[0].y - w[1].y).abs() > 59.0)
        .map(|w| w[0].x)
        .collect();
    assert!((xs[1] - xs[0] - 200.0 / 30.0).abs() < 0.05, "{xs:?}");
}

#[test]
fn satin_gradation_splits_on_every_other_lattice_line() {
    // The 20 x 5 mm rectangle of compat satin-gradation: satin at 3 lines/mm with PE-DESIGN's
    // default pattern, under sewing off, start point at the middle of the bottom edge.
    // PE-DESIGN 11.50 sews the left half from the left edge with vertical crossings on every
    // other line of the cumulative-line-count lattice (lines 0, 2, ... 10), hops one line (11)
    // and crosses, runs home; then the right half from the right edge (lines 17, 15, 13) and
    // the hop to the start point's line (12).
    let c = vec![Contour::rectangle(100.0, 100.0, 200.0, 50.0)];
    let g = crate::Gradation::PE_DEFAULT;
    assert_eq!(
        crate::region::row_positions(&c, 90.0, 10.0 / 3.0, Some(&g)).len(),
        18
    );
    let mut r = region(RegionType::Satin);
    r.angle = 90.0;
    r.density = 3.0;
    r.underlay = false;
    r.gradation = Some(g);
    r.entry = Some(v2(200.0, 150.0));
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert_eq!(runs.len(), 1, "one run, split at the start point");
    let run = &runs[0];
    let crossings: Vec<f64> = run
        .windows(2)
        .filter(|w| (w[0].x - w[1].x).abs() < 0.01 && (w[0].y - w[1].y).abs() > 49.0)
        .map(|w| w[0].x / 10.0)
        .collect();
    let pe = [
        10.0, 11.3, 12.7, 14.3, 15.9, 17.8, 18.8, 30.0, 24.3, 21.2, 20.0,
    ];
    assert_eq!(crossings.len(), pe.len(), "{crossings:?}");
    for (x, p) in crossings.iter().zip(pe) {
        assert!((x - p).abs() <= 0.15, "{x} vs {p}: {crossings:?}");
    }
    assert_eq!(run.len(), 32, "{run:?}"); // 37 stitches with the tie-in and lock
    assert_inside(&runs, &c, "satin gradation");
}

#[test]
fn fill_under_sewing_rows_stay_inside_narrow_points() {
    // The star of compat hole-sewing-pentagon-star: rows across its points are shorter than
    // the 1 mm inset at both ends; they must not collapse to a needle point outside the star.
    let pts = [
        (50.0, 42.0),
        (52.3511, 48.7639),
        (59.5106, 48.9098),
        (53.8042, 53.2361),
        (55.8779, 60.0902),
        (50.0, 56.0),
        (44.1221, 60.0902),
        (46.1958, 53.2361),
        (40.4894, 48.9098),
        (47.6489, 48.7639),
    ];
    let star = vec![Contour::new(
        pts.iter().map(|&(x, y)| v2(x * 10.0, y * 10.0)).collect(),
        true,
    )];
    let r = region(RegionType::Fill);
    assert!(r.underlay);
    let layers = crate::region::layers(&star, &r, &[]);
    for l in &layers {
        assert_inside(l, &star, "star");
    }
    // The under sewing is one run: no jump or trim inside the star.
    assert_eq!(layers[0].len(), 1);
}

#[test]
fn gradation_reads_the_old_single_percentage() {
    let old: crate::Gradation = serde_json::from_str("40").expect("number");
    assert_eq!(old, crate::Gradation::continuous(40.0));
    let pattern = serde_json::to_string(&crate::Gradation::PE_DEFAULT).expect("json");
    assert_eq!(pattern, r#"{"max":53.0,"min":5.0,"steps":9}"#);
    let back: crate::Gradation = serde_json::from_str(&pattern).expect("pattern");
    assert_eq!(back, crate::Gradation::PE_DEFAULT);
    // Continuous: the density falls linearly, as before patterns.
    let g = crate::Gradation::continuous(25.0);
    assert!((g.fraction(0.0) - 1.0).abs() < 1e-12 && (g.fraction(1.0) - 0.25).abs() < 1e-12);
    assert!((g.fraction(0.5) - 0.625).abs() < 1e-12);
}

#[test]
fn feathered_edges_shorten_row_ends_deterministically() {
    let c = vec![Contour::rectangle(0.0, 0.0, 200.0, 100.0)];
    let mut r = region(RegionType::Satin);
    r.feather = Some(Feather {
        side: FeatherSide::Start,
        width: 3.0,
    });
    let a = crate::region::layers(&c, &r, &[]);
    assert_eq!(a, crate::region::layers(&c, &r, &[]));
    let runs = a.last().expect("top");
    let xs: Vec<f64> = runs.iter().flatten().map(|p| p.x).collect();
    assert!(xs.iter().all(|&x| (0.0..=200.0).contains(&x)));
    // Start ends vary across 0…3 mm; far ends stay on the outline.
    let starts: Vec<f64> = xs.iter().copied().filter(|&x| x < 100.0).collect();
    assert!(starts.iter().all(|&x| x <= 30.0));
    assert!(starts.iter().copied().fold(0.0, f64::max) > 20.0);
    assert!(xs.iter().filter(|&&x| x > 100.0).all(|&x| x == 200.0));
    r.feather = Some(Feather {
        side: FeatherSide::Both,
        width: 3.0,
    });
    let both = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert!(both.iter().flatten().any(|p| p.x < 199.0 && p.x > 170.0));
}

#[test]
fn long_satin_stitches_split_and_stagger() {
    let c = vec![Contour::rectangle(0.0, 0.0, 200.0, 40.0)];
    // Top satin only, rows 1/3 mm apart.
    let plain_satin = || {
        let mut r = region(RegionType::Satin);
        r.density = 3.0;
        r.underlay = false;
        r
    };
    let mut r = plain_satin();
    r.split_length = Some(5.0);
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    // The first stitch of a row comes from the previous row's edge, one row (1/3 mm) away.
    let slant = 50.0f64.hypot(10.0 / 3.0);
    for run in &runs {
        assert!(longest(run) <= slant + 1e-6);
    }
    assert_inside(&runs, &c, "split");
    let plain = crate::region::layers(&c, &plain_satin(), &[]);
    assert!(
        longest(&plain[0][0]) > 190.0,
        "without split a row is one stitch"
    );
    // Split points of neighbouring rows do not line up.
    let pts: Vec<Vec2> = runs.iter().flatten().copied().collect();
    let interior = |y_lo: f64, y_hi: f64| -> Vec<f64> {
        let mut xs: Vec<f64> = pts
            .iter()
            .filter(|p| p.y > y_lo && p.y < y_hi && p.x > 1.0 && p.x < 199.0)
            .map(|p| (p.x * 1000.0).round() / 1000.0)
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        xs
    };
    let (row0, row1) = (interior(0.0, 3.0), interior(3.0, 6.0));
    assert!(!row0.is_empty() && !row1.is_empty());
    assert!(row0.iter().all(|x| !row1.contains(x)));
}

#[test]
fn travel_stays_inside_instead_of_jumping() {
    // A ring: the fill splits into parts on either side of the hole.
    let ring = vec![circle(), Contour::ellipse(v2(200.0, 200.0), 80.0, 80.0, 48)];
    let mut r = region(RegionType::Fill);
    r.running_stitch_path = None;
    let plain = crate::region::layers(&ring, &r, &[]).pop().expect("top");
    assert!(plain.len() > 1, "fill without travel jumps between parts");
    r.running_stitch_path = Some(crate::model::RunningStitchPath::Inside);
    let travelled = crate::region::layers(&ring, &r, &[]).pop().expect("top");
    assert_eq!(travelled.len(), 1);
    // The travel stitches (points not in the plain fill) stay inside the ring. (Fill rows
    // themselves may step across a narrow gap, as plain fill does.)
    let fill_points: Vec<Vec2> = plain.iter().flatten().copied().collect();
    let travel: Vec<Vec2> = travelled[0]
        .iter()
        .filter(|p| !fill_points.contains(p))
        .copied()
        .collect();
    assert!(travel.len() > 2);
    assert!(travel.iter().all(|&p| inside_or_on(p, &ring, 0.6)));
    let plain_longest = plain.iter().map(|r| longest(r)).fold(25.0, f64::max);
    assert!(longest(&travelled[0]) <= plain_longest + 1e-6);
    let mut d = Design::new("ring", DesignPage::default());
    d.add(DesignObject::shape(ring.clone(), Some(r.clone()), None));
    let p = Compiler::default().compile(&d);
    assert_eq!(p.count(Command::Jump), 1, "only the initial move");
    r.running_stitch_path = None;
    d.objects[0].region = Some(r);
    assert!(Compiler::default().compile(&d).count(Command::Jump) > 1);
}

#[test]
fn travel_graph_routes_around_a_notch() {
    let u = [Contour::new(
        vec![
            v2(0.0, 0.0),
            v2(60.0, 0.0),
            v2(60.0, 200.0),
            v2(140.0, 200.0),
            v2(140.0, 0.0),
            v2(200.0, 0.0),
            v2(200.0, 260.0),
            v2(0.0, 260.0),
        ],
        true,
    )];
    let path =
        crate::paths::inside_path(v2(30.0, 10.0), v2(170.0, 10.0), &u, &[]).expect("path inside");
    assert!(path.len() >= 4, "{path:?}");
    let len: f64 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    assert!(len > 2.0 * 190.0 && len < 2.0 * 190.0 + 200.0, "{len}");
    for w in path.windows(2) {
        for k in 0..=10 {
            assert!(inside_or_on(w[0].lerp(w[1], k as f64 / 10.0), &u, 0.6));
        }
    }
}

#[test]
fn outline_travel_walks_on_or_just_inside_the_outline() {
    use crate::paths::{OutlineTravel, TravelGraph};
    // The U of travel_graph_routes_around_a_notch; both ends on its bottom edges.
    let u = [Contour::new(
        vec![
            v2(0.0, 0.0),
            v2(60.0, 0.0),
            v2(60.0, 200.0),
            v2(140.0, 200.0),
            v2(140.0, 0.0),
            v2(200.0, 0.0),
            v2(200.0, 260.0),
            v2(0.0, 260.0),
        ],
        true,
    )];
    let (a, b) = (v2(30.0, 0.0), v2(170.0, 0.0));
    let mut g = TravelGraph::new(&u, &[]);
    g.outline = Some(OutlineTravel::On);
    let on = g.path(a, b).expect("on outline");
    let edges = u[0].edges();
    let to_outline = |p: Vec2| {
        edges
            .iter()
            .map(|&(e0, e1)| {
                let d = e1 - e0;
                let t = (((p - e0).x * d.x + (p - e0).y * d.y) / (d.x * d.x + d.y * d.y))
                    .clamp(0.0, 1.0);
                p.distance(e0 + d * t)
            })
            .fold(f64::INFINITY, f64::min)
    };
    // The short way round the notch: up, across, down, every point on the outline.
    assert!(on.iter().all(|&p| to_outline(p) < 1e-6), "{on:?}");
    assert!(on.iter().any(|p| (p.y - 200.0).abs() < 1e-6));
    g.outline = Some(OutlineTravel::Inset(5.0));
    let along = g.path(a, b).expect("along outline");
    let interior = &along[1..along.len() - 1];
    assert!(!interior.is_empty());
    // 0.5 mm inside along the edges; miter points at the reflex corners are up to √2 further.
    assert!(
        interior
            .iter()
            .all(|&p| (4.9..=5.0 * std::f64::consts::SQRT_2 + 0.1).contains(&to_outline(p))),
        "{along:?}"
    );
    for w in along.windows(2) {
        for k in 0..=10 {
            assert!(inside_or_on(w[0].lerp(w[1], k as f64 / 10.0), &u, 0.6));
        }
    }
}

// ---------------------------------------------------------------- region types

/// A flower-ish region with a hole: tests clipping, holes and travel together.
fn blob_with_hole() -> Vec<Contour> {
    let outer: Vec<Vec2> = (0..72)
        .map(|k| {
            let t = k as f64 / 72.0 * std::f64::consts::TAU;
            let r = 200.0 + 40.0 * (5.0 * t).sin();
            v2(300.0 + r * t.cos(), 300.0 + r * t.sin())
        })
        .collect();
    vec![
        Contour::new(outer, true),
        Contour::ellipse(v2(300.0, 300.0), 50.0, 50.0, 32),
    ]
}

const NEW_REGION_TYPES: [RegionType; 12] = [
    RegionType::ProgrammableFill,
    RegionType::MotifFill,
    RegionType::Piping,
    RegionType::CrossStitch,
    RegionType::ConcentricCircle,
    RegionType::Radial,
    RegionType::Spiral,
    RegionType::FlexibleSpiral,
    RegionType::Stippling,
    RegionType::NetFill,
    RegionType::ZigzagNetFill,
    RegionType::DecorativeFill,
];

/// The longest stitch a region type may make with default attributes (0.1 mm).
fn stitch_limit(r: &RegionSewing) -> f64 {
    let pitch = r.run_pitch_mm() * 10.0;
    match r.kind {
        RegionType::ProgrammableFill => r.stitch_length * 10.0,
        RegionType::CrossStitch => (r.spacing_mm() * 10.0 * std::f64::consts::SQRT_2).max(pitch),
        RegionType::ZigzagNetFill => {
            let w = r.pattern_size_mm().0 * 10.0;
            w.hypot(10.0 / r.density).max(pitch)
        }
        RegionType::DecorativeFill => pitch * 1.5,
        _ => pitch,
    }
}

#[test]
fn every_region_type_sews_inside_with_bounded_stitches() {
    let region_contours = blob_with_hole();
    for kind in NEW_REGION_TYPES {
        let mut r = region(kind);
        if kind == RegionType::DecorativeFill {
            r.random_shift = Some(30.0);
        }
        let layers = crate::region::layers(&region_contours, &r, &[]);
        assert_eq!(
            layers,
            crate::region::layers(&region_contours, &r, &[]),
            "{kind:?}"
        );
        let runs = layers.last().expect("top layer");
        let points: usize = runs.iter().map(Vec::len).sum();
        assert!(points > 100, "{kind:?}: {points} points");
        assert_eq!(
            runs.len(),
            1,
            "{kind:?}: travel joins everything into one run"
        );
        let limit = stitch_limit(&r);
        for run in runs {
            assert!(
                longest(run) <= limit + 1e-6,
                "{kind:?}: {} > {limit}",
                longest(run)
            );
        }
        let tol = if kind == RegionType::ZigzagNetFill {
            r.pattern_size_mm().0 * 5.0 + 0.6
        } else {
            0.6
        };
        for run in runs {
            for w in run.windows(2) {
                for p in [w[0], w[1], w[0].lerp(w[1], 0.5)] {
                    assert!(
                        inside_or_on(p, &region_contours, tol),
                        "{kind:?}: {p:?} outside"
                    );
                }
            }
        }
        // The compiler sews it as one colour block without jumps after the first.
        let mut d = Design::new("t", DesignPage::default());
        d.add(DesignObject::shape(region_contours.clone(), Some(r), None));
        let p = Compiler::default().compile(&d);
        assert_eq!(p.count(Command::Jump), 1, "{kind:?}");
    }
}

#[test]
fn region_types_without_travel_jump_between_parts() {
    let region_contours = blob_with_hole();
    for kind in [
        RegionType::NetFill,
        RegionType::Radial,
        RegionType::MotifFill,
    ] {
        let mut r = region(kind);
        r.running_stitch_path = None;
        let runs = crate::region::layers(&region_contours, &r, &[])
            .pop()
            .expect("top");
        assert!(runs.len() > 1, "{kind:?}");
    }
}

#[test]
fn every_pattern_of_every_patterned_type_sews() {
    let c = vec![circle()];
    let cases: Vec<(RegionType, Vec<String>)> = vec![
        (
            RegionType::ProgrammableFill,
            crate::patterns::library()
                .iter()
                .filter(|p| p.kind == PatternKind::Fill)
                .map(|p| p.id.clone())
                .collect(),
        ),
        (
            RegionType::MotifFill,
            crate::patterns::library()
                .iter()
                .filter(|p| p.kind == PatternKind::Motif)
                .map(|p| p.id.clone())
                .collect(),
        ),
        (
            RegionType::DecorativeFill,
            crate::patterns::library()
                .iter()
                .filter(|p| p.kind == PatternKind::Decorative)
                .map(|p| p.id.clone())
                .collect(),
        ),
        (
            RegionType::NetFill,
            crate::patterns::NET_PATTERNS
                .iter()
                .map(|n| n.id.to_string())
                .collect(),
        ),
        (
            RegionType::ZigzagNetFill,
            crate::patterns::NET_PATTERNS[..crate::patterns::ZIGZAG_NET_PATTERNS]
                .iter()
                .map(|n| n.id.to_string())
                .collect(),
        ),
    ];
    for (kind, ids) in cases {
        let mut outputs = Vec::new();
        for id in ids {
            let mut r = region(kind);
            r.pattern = Some(id.clone());
            let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
            assert!(
                runs.iter().map(Vec::len).sum::<usize>() > 50,
                "{kind:?} {id}"
            );
            assert!(
                runs.iter()
                    .all(|run| longest(run) <= stitch_limit(&r) + 1e-6),
                "{kind:?} {id}"
            );
            outputs.push(runs);
        }
        for i in 1..outputs.len() {
            assert_ne!(outputs[i], outputs[0], "{kind:?}: patterns differ");
        }
    }
}

#[test]
fn programmable_fill_puts_needle_points_on_the_pattern() {
    let c = vec![Contour::rectangle(0.0, 0.0, 400.0, 400.0)];
    let mut r = region(RegionType::ProgrammableFill);
    r.pattern = Some("diamonds".into());
    r.angle = 0.0;
    r.running_stitch_path = None;
    // The pattern and base sewing layers only, without under sewing.
    r.underlay = false;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    // Diamond lines in an 8 mm tile: |x mod 80 - 40| + |y mod 80 - 40|·… = 40, i.e. points
    // where x' + y' = 40 within the tile. Count needle points within 0.3 mm of a line.
    let on_line = |p: &Vec2| {
        let (u, v) = ((p.x / 80.0).fract(), (p.y / 80.0).fract());
        let d = ((u - 0.5).abs() + (v - 0.5).abs() - 0.5).abs() * 80.0 / std::f64::consts::SQRT_2;
        d < 3.0
    };
    let pts: Vec<&Vec2> = runs.iter().flatten().collect();
    let share = pts.iter().filter(|p| on_line(p)).count() as f64 / pts.len() as f64;
    let mut plain = region(RegionType::Fill);
    plain.running_stitch_path = None;
    let fill = crate::region::layers(&c, &plain, &[]).pop().expect("fill");
    let fpts: Vec<&Vec2> = fill.iter().flatten().collect();
    let fill_share = fpts.iter().filter(|p| on_line(p)).count() as f64 / fpts.len() as f64;
    assert!(share > fill_share * 1.5, "{share} vs {fill_share}");
    r.base_sewing = true;
    assert_eq!(
        crate::region::layers(&c, &r, &[]).len(),
        2,
        "base sewing layer"
    );
}

#[test]
fn piping_rows_shift_by_the_frequency() {
    let c = vec![Contour::rectangle(0.0, 0.0, 300.0, 100.0)];
    let mut r = region(RegionType::Piping);
    r.frequency = Some(50.0);
    r.running_stitch_path = None;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    let row = |y_lo: f64, y_hi: f64| -> Vec<f64> {
        let mut xs: Vec<f64> = runs
            .iter()
            .flatten()
            .filter(|p| p.y > y_lo && p.y < y_hi && p.x > 1.0 && p.x < 299.0)
            .map(|p| p.x)
            .collect();
        xs.sort_by(f64::total_cmp);
        xs
    };
    // Rows 1 mm apart, stitches 2 mm, shifted by half a stitch per row.
    let (a, b) = (row(0.0, 10.0), row(10.0, 20.0));
    assert!((a[0] - 20.0).abs() < 1e-6, "{a:?}");
    assert!((b[0] - 10.0).abs() < 1e-6, "{b:?}");
}

#[test]
fn cross_stitch_sews_an_x_in_every_cell() {
    let c = vec![Contour::rectangle(0.0, 0.0, 100.0, 100.0)];
    let mut r = region(RegionType::CrossStitch);
    r.running_stitch_path = None;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert_eq!(runs.len(), 4, "one run per row of cells");
    let diagonals = |runs: &[Run]| {
        runs.iter()
            .flat_map(|r| r.windows(2))
            .filter(|w| (w[0].distance(w[1]) - 25.0 * std::f64::consts::SQRT_2).abs() < 1e-6)
            .count()
    };
    assert_eq!(diagonals(&runs), 16 * 2);
    r.triple = true;
    let triple = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert_eq!(diagonals(&triple), 16 * 2 * 3);
    // A cell crossing the outline is skipped.
    let small = vec![Contour::rectangle(0.0, 0.0, 90.0, 90.0)];
    let partial = crate::region::layers(&small, &region(RegionType::CrossStitch), &[]);
    assert_eq!(diagonals(partial.last().expect("top")), 9 * 2);
}

#[test]
fn circles_and_spirals_start_where_asked() {
    let c = vec![circle()];
    let centre = v2(200.0, 200.0);
    for kind in [
        RegionType::ConcentricCircle,
        RegionType::Spiral,
        RegionType::FlexibleSpiral,
    ] {
        let mut r = region(kind);
        let outside = crate::region::layers(&c, &r, &[]).pop().expect("top");
        r.start_inside = true;
        let inside = crate::region::layers(&c, &r, &[]).pop().expect("top");
        let first = |runs: &[Run]| runs[0][0].distance(centre);
        assert!(first(&outside) > 120.0, "{kind:?}: {}", first(&outside));
        assert!(first(&inside) < 30.0, "{kind:?}: {}", first(&inside));
    }
    // Concentric rings sit at multiples of the spacing (1 / 1.5 mm).
    let rings = crate::region::layers(&c, &region(RegionType::ConcentricCircle), &[]);
    let spacing = 10.0 / 1.5;
    let on_ring = rings[0]
        .iter()
        .flatten()
        .filter(|p| {
            let k = (p.distance(centre) - spacing / 2.0) / spacing;
            (k - k.round()).abs() < 0.05
        })
        .count();
    let total: usize = rings[0].iter().map(Vec::len).sum();
    assert!(on_ring as f64 > total as f64 * 0.9);
    // Spirals turn either way.
    let mut cw = region(RegionType::Spiral);
    cw.start_inside = true;
    let mut ccw = cw.clone();
    ccw.counterclockwise = true;
    let turn = |r: &RegionSewing| {
        let runs = crate::region::layers(&c, r, &[]).pop().expect("top");
        let p = &runs[0];
        let (a, b) = (p[3] - centre, p[4] - centre);
        a.x * b.y - a.y * b.x
    };
    assert!(turn(&cw) > 0.0 && turn(&ccw) < 0.0);
}

#[test]
fn radial_lines_thin_out_towards_the_centre() {
    let c = vec![circle()];
    let runs = crate::region::layers(&c, &region(RegionType::Radial), &[]);
    let pts: Vec<&Vec2> = runs[0].iter().flatten().collect();
    let centre = v2(200.0, 200.0);
    let density = |r0: f64, r1: f64| {
        let n = pts
            .iter()
            .filter(|p| (r0..r1).contains(&p.distance(centre)))
            .count() as f64;
        n / (std::f64::consts::PI * (r1 * r1 - r0 * r0))
    };
    let (inner, outer) = (density(10.0, 40.0), density(110.0, 140.0));
    assert!(inner < outer * 4.0, "inner {inner} outer {outer}");
}

#[test]
fn stippling_is_a_closed_path_that_never_crosses_itself() {
    let c = vec![Contour::ellipse(v2(150.0, 150.0), 120.0, 90.0, 48)];
    let mut r = region(RegionType::Stippling);
    r.running_stitch_path = None;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert_eq!(runs.len(), 1, "one connected part, one loop");
    let run = &runs[0];
    assert!(
        run[0].distance(*run.last().expect("points")) < 1e-6,
        "closed"
    );
    let segs: Vec<(Vec2, Vec2)> = run.windows(2).map(|w| (w[0], w[1])).collect();
    let cross = |a: Vec2, b: Vec2, c: Vec2, d: Vec2| {
        let o = |p: Vec2, q: Vec2, r: Vec2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
        o(a, b, c) * o(a, b, d) < -1e-9 && o(c, d, a) * o(c, d, b) < -1e-9
    };
    for i in 0..segs.len() {
        for j in i + 2..segs.len() {
            if i == 0 && j == segs.len() - 1 {
                continue;
            }
            assert!(
                !cross(segs[i].0, segs[i].1, segs[j].0, segs[j].1),
                "segments {i} and {j} cross"
            );
        }
    }
}

#[test]
fn square_net_lines_are_straight_and_axis_aligned() {
    let c = vec![Contour::rectangle(0.0, 0.0, 200.0, 200.0)];
    let mut r = region(RegionType::NetFill);
    r.running_stitch_path = None;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    for run in &runs {
        for w in run.windows(2) {
            let d = w[1] - w[0];
            assert!(d.x.abs() < 1e-6 || d.y.abs() < 1e-6, "{d:?}");
        }
    }
    // 4 mm spacing over 20 mm: lines at 0, 4, … in both directions.
    let xs: std::collections::BTreeSet<i64> = runs
        .iter()
        .flatten()
        .filter(|p| p.y > 5.0 && p.y < 195.0)
        .filter(|p| p.x > 1.0 && p.x < 199.0)
        .map(|p| p.x.round() as i64)
        .filter(|x| x % 40 == 0)
        .collect();
    assert_eq!(xs.into_iter().collect::<Vec<_>>(), vec![40, 80, 120, 160]);
}

#[test]
fn motif_fill_repeats_the_motif_in_rows() {
    let c = vec![Contour::rectangle(0.0, 0.0, 300.0, 200.0)];
    let mut r = region(RegionType::MotifFill);
    r.pattern = Some("peak".into());
    r.running_stitch_path = None;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    let pts: Vec<&Vec2> = runs.iter().flatten().collect();
    // Peaks are 3 mm (0.9 of the height) above each row baseline: rows every 4 mm.
    let peaks = pts
        .iter()
        .filter(|p| ((p.y + 27.0) / 40.0).fract().abs() < 1e-6)
        .count();
    assert!(peaks >= 25, "{peaks}");
}
