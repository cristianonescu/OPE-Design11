//! Stitch Design Factory sewing-stitch patterns: generation, validation, the
//! `openstitch.sewing` format, SVG and PMV (against pyembroidery fixtures).

use openstitch_design::sewing::{self, MAX_FEED, SewingKind, SewingStitch};
use openstitch_design::v2;
use std::path::Path;

fn longest_feed(s: &SewingStitch) -> f64 {
    s.needles()
        .windows(2)
        .map(|w| (w[1].x - w[0].x).abs())
        .fold(0.0, f64::max)
}

#[test]
fn every_kind_starts_valid_inside_its_width() {
    for kind in SewingKind::ALL {
        let s = SewingStitch::new(kind);
        s.validate().unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let needles = s.needles();
        assert!(needles.len() >= 5, "{kind:?}");
        for p in &needles {
            assert!(p.y.abs() <= s.width / 2.0 + 1e-9, "{kind:?} {p:?}");
            assert!(p.x >= 0.0 && p.x <= sewing::MAX_LENGTH);
        }
        assert!(longest_feed(&s) <= MAX_FEED + 1e-9, "{kind:?}");
        let doc = s.to_json(false);
        assert_eq!(sewing::parse(&doc).expect("round trip"), s, "{kind:?}");
        // The export form (with needles) reads back too.
        assert_eq!(sewing::parse(&s.to_json(true)).expect("with needles"), s);
    }
}

#[test]
fn satin_alternates_edges_at_the_density() {
    let mut s = SewingStitch::new(SewingKind::Satin);
    s.profile = vec![[0.0, -2.0, 1.0], [10.0, -3.0, 3.0]];
    s.density = 2.0;
    let n = s.needles();
    assert_eq!(n.len(), 21);
    assert_eq!(n[0], v2(0.0, -2.0));
    assert_eq!(n[1], v2(0.5, 1.1));
    assert_eq!(n[20], v2(10.0, -3.0));
    assert!((s.length() - 10.0).abs() < 1e-9);
}

#[test]
fn decorative_satin_fills_the_outline_across_the_feed() {
    let mut s = SewingStitch::new(SewingKind::DecorativeSatin);
    s.outline = vec![v2(0.0, -2.0), v2(8.0, -2.0), v2(8.0, 2.0), v2(0.0, 2.0)];
    s.density = 4.0;
    let n = s.needles();
    assert_eq!(n.len(), 32);
    for (i, p) in n.iter().enumerate() {
        assert_eq!(p.y, if i % 2 == 0 { -2.0 } else { 2.0 });
    }
}

#[test]
fn cross_cells_are_sewn_corner_to_corner() {
    let mut s = SewingStitch::new(SewingKind::Cross);
    s.cells = vec![[0, 0], [3, 2], [1, 1]];
    let n = s.needles();
    assert_eq!(n.len(), 3 * 4);
    let diagonal = 2.0 * std::f64::consts::SQRT_2;
    for pair in n.chunks(2) {
        assert!((pair[0].distance(pair[1]) - diagonal).abs() < 1e-9);
    }
    // Column order along the feed.
    assert!(n.windows(2).all(|w| w[1].x >= w[0].x - 2.0 - 1e-9));
    s.triple = true;
    assert_eq!(s.needles().len(), 3 * 8);
}

#[test]
fn bad_patterns_are_refused() {
    let base = SewingStitch::new(SewingKind::Decorative);
    let mut cases: Vec<(&str, SewingStitch)> = Vec::new();
    let mut case = |what: &'static str, f: &dyn Fn(&mut SewingStitch)| {
        let mut s = base.clone();
        f(&mut s);
        cases.push((what, s));
    };
    case("id", &|s| s.id = "Bad Id".into());
    case("empty id", &|s| s.id = String::new());
    case("name", &|s| s.name = String::new());
    case("narrow", &|s| s.width = 0.5);
    case("wide", &|s| s.width = 10.0);
    case("nan width", &|s| s.width = f64::NAN);
    case("outside band", &|s| s.points[1].y = 5.0);
    case("negative x", &|s| s.points[0].x = -1.0);
    case("long feed", &|s| s.points[1].x = 9.0);
    case("one point", &|s| s.points.truncate(1));
    case("nan point", &|s| s.points[2].x = f64::NAN);
    case("too many", &|s| {
        s.points = (0..2001).map(|i| v2(f64::from(i % 50), 0.0)).collect();
    });
    for (what, s) in &cases {
        assert!(s.validate().is_err(), "{what}");
    }
    let mut satin = SewingStitch::new(SewingKind::Satin);
    satin.profile = vec![[0.0, 1.0, -1.0], [5.0, -1.0, 1.0]];
    assert!(satin.validate().is_err(), "left > right");
    satin.profile = vec![[5.0, -1.0, 1.0], [0.0, -1.0, 1.0]];
    assert!(satin.validate().is_err(), "x decreasing");
    satin.profile = vec![[0.0, -1.0, 1.0], [5.0, -1.0, 1.0]];
    satin.density = 0.0;
    assert!(satin.validate().is_err(), "density");
    let mut ds = SewingStitch::new(SewingKind::DecorativeSatin);
    ds.outline = vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(2.0, 0.0)];
    assert!(ds.validate().is_err(), "no area");
    let mut cross = SewingStitch::new(SewingKind::Cross);
    cross.cells = vec![[0, 3]];
    assert!(cross.validate().is_err(), "row outside");
    cross.cells = vec![[0, 0]];
    cross.rows = 4;
    assert!(cross.validate().is_err(), "rows do not fit the width");
    cross.rows = 3;
    cross.cell_size = 0.1;
    assert!(cross.validate().is_err(), "cell size");
    cross.cell_size = 2.0;
    cross.cells.clear();
    assert!(cross.validate().is_err(), "no cells");
}

#[test]
fn hostile_documents_are_refused() {
    let good = SewingStitch::new(SewingKind::Satin).to_json(false);
    assert!(sewing::parse(&good).is_ok());
    let doc: serde_json::Value = serde_json::from_str(&good).expect("json");
    let edit = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut d = doc.clone();
        f(&mut d);
        d.to_string()
    };
    for bad in [
        String::new(),
        "{".into(),
        "[]".into(),
        edit(&|d| d["format"] = "other".into()),
        edit(&|d| d["version"] = 2.into()),
        edit(&|d| d["kind"] = "sparkle".into()),
        edit(&|d| d["width"] = "7".into()),
        edit(&|d| d["extra"] = 1.into()),
        edit(&|d| d["profile"] = serde_json::json!([[0, -1, 1]])),
        edit(&|d| d["density"] = serde_json::json!(1e300)),
        " ".repeat(sewing::MAX_DOCUMENT + 1),
    ] {
        assert!(sewing::parse(&bad).is_err(), "{bad:.80}");
    }
    let huge = format!(
        "{{\"format\":\"openstitch.sewing\",\"version\":1,\"kind\":\"decorative\",\"id\":\"x\",\"name\":\"x\",\"width\":7,\"points\":[[0,0],[1,0]],\"needles\":[{}]}}",
        vec!["[0,0]"; 2001].join(",")
    );
    assert!(sewing::parse(&huge).is_err());
}

#[test]
fn svg_draws_every_needle_of_every_repeat() {
    let s = SewingStitch::new(SewingKind::Decorative);
    let svg = sewing::svg(&s, 3);
    assert!(svg.starts_with("<svg"));
    assert_eq!(svg.matches("<circle").count(), 5 * 3);
    assert!(svg.contains("stroke-dasharray"));
}

#[test]
fn pmv_matches_pyembroidery_byte_for_byte() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pmv");
    for name in ["zigzag", "wide", "flat", "wave"] {
        let pts: Vec<[f64; 2]> = serde_json::from_slice(
            &std::fs::read(dir.join(format!("{name}.json"))).expect("fixture"),
        )
        .expect("json");
        let pts: Vec<(f64, f64)> = pts.iter().map(|&[x, y]| (x, y)).collect();
        let expected = std::fs::read(dir.join(format!("{name}.pmv"))).expect("fixture");
        assert_eq!(sewing::pmv_bytes(&pts), expected, "{name}");
    }
}

#[test]
fn pmv_export_limits_needle_count() {
    let s = SewingStitch::new(SewingKind::Satin);
    let bytes = sewing::pmv(&s).expect("small pattern");
    assert!(bytes.starts_with(b"#PMV0001"));
    let mut long = SewingStitch::new(SewingKind::Satin);
    long.profile = vec![[0.0, -3.0, 3.0], [40.0, -3.0, 3.0]];
    assert!(long.needles().len() > sewing::PMV_MAX_NEEDLES);
    assert!(sewing::pmv(&long).is_err());
}

#[test]
fn swap_sews_the_same_needles_from_the_other_end() {
    for kind in SewingKind::ALL {
        let mut s = SewingStitch::new(kind);
        let before = s.needles();
        s.swap();
        let mut after = s.needles();
        after.reverse();
        assert_eq!(after, before, "{kind:?}");
        // Kept in the document, and only written when on.
        assert!(s.to_json(false).contains("\"swapped\":true"));
        assert_eq!(sewing::parse(&s.to_json(false)).expect("round trip"), s);
        s.swap();
        assert!(!s.to_json(false).contains("swapped"));
    }
}

#[test]
fn duplicate_places_a_copy_where_the_pattern_ends() {
    let s = SewingStitch::new(SewingKind::Decorative);
    let d = s.duplicate().expect("decorative");
    assert_eq!(d.points.len(), 2 * s.points.len());
    assert_eq!(d.points[s.points.len()], v2(8.0, 0.0));
    assert_eq!(d.length(), 16.0);

    let s = SewingStitch::new(SewingKind::Satin);
    let d = s.duplicate().expect("satin");
    assert_eq!(d.profile.len(), 5, "the shared station is not repeated");
    assert_eq!(d.profile[4], [16.0, -0.5, 0.5]);

    let s = SewingStitch::new(SewingKind::Cross);
    let d = s.duplicate().expect("cross");
    assert!(d.cells.contains(&[3, 1]) && d.cells.contains(&[5, 1]));

    assert!(
        SewingStitch::new(SewingKind::DecorativeSatin)
            .duplicate()
            .is_err()
    );
    // Duplicating until the pattern is too long fails without changing it.
    let mut s = SewingStitch::new(SewingKind::Decorative);
    while let Ok(d) = s.duplicate() {
        s = d;
    }
    assert!(s.validate().is_ok() && s.length() <= sewing::MAX_LENGTH);
}

#[test]
fn translate_moves_every_point_inside_the_band_only() {
    let mut s = SewingStitch::new(SewingKind::Decorative);
    s.translate(v2(2.0, 0.5)).expect("inside");
    assert_eq!(s.points[0], v2(2.0, 0.5));
    let before = s.clone();
    assert!(s.translate(v2(0.0, 1.0)).is_err(), "3.5 mm is the edge");
    assert!(s.translate(v2(-5.0, 0.0)).is_err(), "x stays >= 0");
    assert_eq!(s, before);

    let mut c = SewingStitch::new(SewingKind::Cross);
    c.translate(v2(4.1, 0.0)).expect("two cells");
    assert!(c.cells.contains(&[2, 1]));
    assert!(c.translate(v2(-20.0, 0.0)).is_err());
}
