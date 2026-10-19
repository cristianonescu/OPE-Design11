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
    let run = lines::stem(&hline(300.0), 20.0, 15.0, 45.0, false);
    // 21 stitches, each sewn three times (four points), from below to above the line.
    assert_eq!(run.len(), 21 * 4);
    assert!(run.iter().all(|p| p.y.abs() <= 10.0 + 1e-9));
    let slant = 20.0 / 45f64.to_radians().sin();
    assert!(longest(&run) <= slant + 15.0 + 1e-9);
    assert!((run[0].distance(run[1]) - slant).abs() < 1e-9);
    let single = lines::stem(&hline(300.0), 20.0, 15.0, 90.0, true);
    assert_eq!(single.len(), 21 * 2);
    assert!(
        (single[0].x - single[1].x).abs() < 1e-9,
        "90° is perpendicular"
    );
}

#[test]
fn candlewick_knots_sit_on_the_line() {
    let run = lines::candlewick(&hline(200.0), 20.0, 6, 50.0, 25.0);
    assert!(longest(&run) <= 25.0 + 1e-9);
    assert!(run.iter().all(|p| p.y.abs() <= 10.0 + 1e-9));
    // Knots at 0, 50, 100, 150, 200: six spoke tips each.
    let tips = run
        .iter()
        .filter(|p| {
            [0.0, 50.0, 100.0, 150.0, 200.0]
                .iter()
                .any(|&x| (v2(x, 0.0).distance(**p) - 10.0).abs() < 1e-9)
        })
        .count();
    assert_eq!(tips, 5 * 6);
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
    let v = lines::v_stitch(&hline(300.0), &e);
    let apexes = v.iter().filter(|p| (p.y.abs() - 25.0).abs() < 1e-9).count();
    assert_eq!(apexes, 10 * 2);
    assert!(longest(&v) <= 25.0f64.hypot(7.5) + 1e-9);
    assert_eq!(v.last().copied(), Some(v2(300.0, 0.0)));
}

#[test]
fn chain_links_stay_within_their_size() {
    for shape in [ChainShape::Triangle, ChainShape::Diamond] {
        let run = lines::chain(&hline(200.0), 20.0, shape, 2, Arrange::Normal, 10.0);
        assert!(run.iter().all(|p| p.y.abs() <= 10.0 + 1e-9));
        assert!(longest(&run) <= 10.0f64.hypot(10.0) + 1e-9);
        let peaks = run
            .iter()
            .filter(|p| (p.y.abs() - 10.0).abs() < 1e-9)
            .count();
        let per_pass = if shape == ChainShape::Triangle {
            10
        } else {
            20
        };
        assert_eq!(peaks, per_pass * 2, "{shape:?}");
    }
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
    for run in runs {
        for w in run.windows(2) {
            let d = w[0].distance(w[1]);
            assert!(d <= 50.0 + 1e-6, "stitch {d} longer than the narrow side");
        }
    }
    assert_inside(runs, &[wide], "auto satin");
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
    let graded = crate::region::row_positions(&c, 0.0, 4.0, Some(25.0));
    assert_eq!(even.len(), 50);
    assert!(graded.len() < 40 && graded.len() > 25, "{}", graded.len());
    let gaps: Vec<f64> = graded.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.windows(2).all(|g| g[1] >= g[0]));
    assert!(gaps[gaps.len() - 1] > 3.0 * gaps[0]);
    let mut r = region(RegionType::Satin);
    r.gradation = Some(25.0);
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    assert_inside(&runs, &c, "gradation");
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
    let mut r = region(RegionType::Satin);
    r.split_length = Some(5.0);
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    for run in &runs {
        assert!(longest(run) <= 50.0 + 1e-6);
    }
    assert_inside(&runs, &c, "split");
    let plain = crate::region::layers(&c, &region(RegionType::Satin), &[]);
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
    let plain = crate::region::layers(&ring, &r, &[]).pop().expect("top");
    assert!(plain.len() > 1, "fill without travel jumps between parts");
    r.travel = true;
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
    r.travel = false;
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
        r.travel = false;
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
    r.travel = false;
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
    plain.travel = false;
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
    r.travel = false;
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
    r.travel = false;
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
    r.travel = false;
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
    r.travel = false;
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
    r.travel = false;
    let runs = crate::region::layers(&c, &r, &[]).pop().expect("top");
    let pts: Vec<&Vec2> = runs.iter().flatten().collect();
    // Peaks are 3 mm (0.9 of the height) above each row baseline: rows every 4 mm.
    let peaks = pts
        .iter()
        .filter(|p| ((p.y + 27.0) / 40.0).fract().abs() < 1e-6)
        .count();
    assert!(peaks >= 25, "{peaks}");
}
