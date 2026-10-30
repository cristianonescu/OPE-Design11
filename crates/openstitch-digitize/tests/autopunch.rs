mod common;

use common::*;
use openstitch_design::{Compiler, Design, DesignPage, RegionType};
use openstitch_digitize::autopunch::{self, AutoPunch, small_region_floor};
use openstitch_digitize::palette::{self, CENSUS_COLORS, Palette};
use openstitch_digitize::regions;
use openstitch_digitize::trace::{marching_squares, signed_area};
use openstitch_digitize::{DigitizeError, Frame, Raster};

#[test]
fn marching_squares_orients_outer_and_holes() {
    let one = marching_squares(&[true], 1, 1);
    assert_eq!(one.len(), 1);
    assert!((signed_area(&one[0]) - 0.5).abs() < 1e-9);

    #[rustfmt::skip]
    let ring = [
        true, true, true,
        true, false, true,
        true, true, true,
    ];
    let loops = marching_squares(&ring, 3, 3);
    assert_eq!(loops.len(), 2);
    let areas: Vec<f64> = loops.iter().map(|l| signed_area(l)).collect();
    assert_eq!(areas.iter().filter(|a| **a > 0.0).count(), 1);
    assert_eq!(areas.iter().filter(|a| **a < 0.0).count(), 1);

    // Diagonal neighbours are separate (4-connectivity).
    let diagonal = marching_squares(&[true, false, false, true], 2, 2);
    assert_eq!(diagonal.len(), 2);
}

fn two_shapes() -> Raster {
    // 60×40 px: a 20×30 red block and a 20×4 blue bar on white.
    rects(
        60,
        40,
        WHITE,
        &[(5, 5, 25, 35, RED), (35, 10, 55, 14, BLUE)],
    )
}

#[test]
fn regions_colours_and_sew_types() {
    let frame = Frame::new(10.0, 20.0, 60.0, 60); // 1 mm per pixel
    let out = autopunch::convert(
        &two_shapes(),
        &frame,
        &AutoPunch {
            colors: 3,
            ..Default::default()
        },
    )
    .expect("converts");
    assert_eq!(out.objects.len(), 2);
    // PEC chart "White" is #f0f0f0.
    assert_eq!(
        out.background.as_ref().map(|t| t.hex()),
        Some("#f0f0f0".into())
    );
    // Largest colour first: red block, then the blue bar.
    let red = out.objects[0].region.as_ref().expect("region");
    let blue = out.objects[1].region.as_ref().expect("region");
    assert_eq!(red.kind, RegionType::Fill);
    assert_eq!(blue.kind, RegionType::Satin);
    assert_eq!(blue.angle, 90.0, "satin stitches run across the bar");
    assert!(
        red.thread.red > 150 && red.thread.blue < 100,
        "{:?}",
        red.thread
    );
    assert!(
        blue.thread.blue > 120 && blue.thread.red < 100,
        "{:?}",
        blue.thread
    );
    assert_eq!(out.threads.len(), 2);
    for o in &out.objects {
        assert_eq!(o.contours.len(), 1);
        assert!(o.contours[0].closed);
    }
    // Outlines stay inside the image frame (10..70 mm × 20..60 mm) and hug their blocks.
    assert!(all_inside(&out.objects, 100.0, 200.0, 700.0, 600.0, 0.0));
    assert!(all_inside(
        &out.objects[..1],
        150.0,
        250.0,
        350.0,
        550.0,
        1.0
    ));
    assert!(all_inside(
        &out.objects[1..],
        450.0,
        300.0,
        650.0,
        340.0,
        1.0
    ));
}

#[test]
fn holes_are_traced() {
    let img = rects(
        40,
        40,
        WHITE,
        &[(5, 5, 35, 35, RED), (15, 15, 25, 25, WHITE)],
    );
    let out = autopunch::convert(&img, &Frame::new(0.0, 0.0, 40.0, 40), &AutoPunch::default())
        .expect("converts");
    assert_eq!(out.objects.len(), 1);
    assert_eq!(out.objects[0].contours.len(), 2, "outer ring and hole");
}

#[test]
fn keeping_the_background_adds_its_region() {
    let p = AutoPunch {
        colors: 3,
        skip_background: false,
        ..Default::default()
    };
    let out =
        autopunch::convert(&two_shapes(), &Frame::new(0.0, 0.0, 60.0, 60), &p).expect("converts");
    assert_eq!(out.objects.len(), 3);
    assert!(out.background.is_none());
    // The white background has two holes where the shapes sit.
    assert_eq!(out.objects[0].contours.len(), 3);
}

#[test]
fn small_specks_merge_into_neighbours() {
    let img = rects(
        40,
        40,
        WHITE,
        &[
            (0, 0, 40, 40, RED),
            (10, 10, 11, 11, BLUE),
            (30, 20, 31, 22, BLUE),
        ],
    );
    let p = AutoPunch {
        colors: 2,
        skip_background: false,
        smooth: 0,
        min_area_mm2: 4.0,
        ..Default::default()
    };
    let out = autopunch::convert(&img, &Frame::new(0.0, 0.0, 40.0, 40), &p).expect("converts");
    assert_eq!(out.objects.len(), 1);
    assert_eq!(out.objects[0].contours.len(), 1, "specks filled in");
}

#[test]
fn curves_are_fitted_on_round_shapes_and_optional() {
    let img = Raster::from_fn(80, 80, |x, y| {
        let (dx, dy) = (x as f64 - 39.5, y as f64 - 39.5);
        if dx * dx + dy * dy < 30.0 * 30.0 {
            BLUE
        } else {
            WHITE
        }
    })
    .expect("size");
    let frame = Frame::new(0.0, 0.0, 80.0, 80);
    let curved = autopunch::convert(&img, &frame, &AutoPunch::default()).expect("converts");
    let c = &curved.objects[0].contours[0];
    assert!(c.has_curves());
    // The flattened outline stays near the true circle (radius 30 mm around 40, 40).
    for p in c.path() {
        let r = ((p.x - 400.0).powi(2) + (p.y - 400.0).powi(2)).sqrt();
        assert!((r - 300.0).abs() < 12.0, "radius {r}");
    }
    let mut p = AutoPunch::default();
    p.trace.curves = false;
    let straight = autopunch::convert(&img, &frame, &p).expect("converts");
    assert!(!straight.objects[0].contours[0].has_curves());
}

#[test]
fn deterministic_and_compiles() {
    let p = AutoPunch {
        colors: 3,
        ..Default::default()
    };
    let frame = Frame::new(0.0, 0.0, 60.0, 60);
    let a = autopunch::convert(&two_shapes(), &frame, &p).expect("converts");
    let b = autopunch::convert(&two_shapes(), &frame, &p).expect("converts");
    assert_eq!(a, b);
    let mut d = Design::new(
        "t",
        DesignPage {
            width: 700.0,
            height: 700.0,
        },
    );
    for o in a.objects {
        d.add(o);
    }
    let pattern = Compiler::default().compile(&d);
    assert!(pattern.stitches.len() > 100);
    assert_eq!(pattern.threads.len(), 2);
}

#[test]
fn hostile_images_fail_cleanly() {
    assert!(matches!(
        Raster::from_rgba(0, 10, vec![]),
        Err(DigitizeError::Empty(_))
    ));
    assert!(matches!(
        Raster::from_rgba(9000, 1, vec![0; 36000]),
        Err(DigitizeError::TooLarge(_))
    ));
    assert!(matches!(
        Raster::from_rgba(5000, 5000, vec![]),
        Err(DigitizeError::TooLarge(_))
    ));
    assert!(Raster::from_rgba(2, 2, vec![0; 3]).is_err());
    assert!(matches!(
        Raster::decode(b"not an image"),
        Err(DigitizeError::Decode(_))
    ));
    assert!(Raster::decode(&[]).is_err());
    let good = png(&two_shapes());
    assert_eq!(Raster::decode(&good).expect("decodes"), two_shapes());
    assert!(
        Raster::decode(&good[..good.len() / 2]).is_err(),
        "truncated"
    );
    // A header claiming 100 000 × 100 000 pixels is refused before decoding.
    let mut huge = good.clone();
    huge[16..20].copy_from_slice(&100_000u32.to_be_bytes());
    huge[20..24].copy_from_slice(&100_000u32.to_be_bytes());
    let crc = png_crc(&huge[12..29]);
    huge[29..33].copy_from_slice(&crc.to_be_bytes());
    assert!(Raster::decode(&huge).is_err());
    // Corrupt bytes after a valid header.
    let mut corrupt = good.clone();
    for b in corrupt.iter_mut().skip(40).step_by(7) {
        *b ^= 0x5A;
    }
    assert!(Raster::decode(&corrupt).is_err());
    assert!(matches!(
        Raster::decode(&vec![0u8; (32 << 20) + 1]),
        Err(DigitizeError::TooLarge(_))
    ));

    let clear = rects(10, 10, CLEAR, &[]);
    let frame = Frame::new(0.0, 0.0, 10.0, 10);
    assert!(matches!(
        autopunch::convert(&clear, &frame, &AutoPunch::default()),
        Err(DigitizeError::Empty(_))
    ));
    let plain = rects(10, 10, WHITE, &[]);
    assert!(matches!(
        autopunch::convert(&plain, &frame, &AutoPunch::default()),
        Err(DigitizeError::Empty(_))
    ));
    // A 1×1 image converts (or reports empty) without panicking.
    let dot = rects(1, 1, RED, &[]);
    let _ = autopunch::convert(
        &dot,
        &frame,
        &AutoPunch {
            skip_background: false,
            ..Default::default()
        },
    );
}

#[test]
fn noisy_image_is_bounded() {
    // Pseudo-random noise at the largest working resolution stays within the region cap.
    let img = Raster::from_fn(600, 600, |x, y| {
        let s = (x * 7919 + y * 104_729).wrapping_mul(2_654_435_761);
        let v = (s >> 16) as u8;
        [v, v.wrapping_mul(7), v.wrapping_mul(13), 255]
    });
    let img = img.expect("size");
    let p = AutoPunch {
        colors: 64,
        resolution: 600,
        min_area_mm2: 0.0,
        smooth: 0,
        ..Default::default()
    };
    match autopunch::convert(&img, &Frame::new(0.0, 0.0, 100.0, 600), &p) {
        Ok(out) => assert!(out.objects.len() <= autopunch::MAX_REGIONS),
        Err(e) => assert!(matches!(e, DigitizeError::TooLarge(_)), "{e}"),
    }
}

fn png_crc(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// #127: the colour census takes each pixel's 3×3 modal colour and tracks at most 100 colours.
#[test]
fn census_is_modal_and_capped_at_100_colours() {
    // A lone blue pixel in red takes the window's majority: red.
    let speck = rects(9, 9, RED, &[(4, 4, 5, 5, BLUE)]);
    let c = palette::census(&speck);
    assert_eq!(c.colors, vec![([220, 20, 30], 81)]);
    assert!(c.modal.iter().all(|m| *m == Some([220, 20, 30])));
    // A 2×2 checkerboard ties 5:4 at every centre, so each pixel keeps its own colour.
    let check =
        Raster::from_fn(8, 8, |x, y| if (x + y) % 2 == 0 { RED } else { BLUE }).expect("size");
    let c = palette::census(&check);
    assert_eq!(c.modal[0], Some([220, 20, 30]));
    assert_eq!(c.modal[1], Some([20, 40, 200]));
    assert_eq!(c.pixels(), 64);
    // Transparent pixels are neither counted nor voting.
    let clear = rects(5, 5, CLEAR, &[(2, 2, 3, 3, BLUE)]);
    let c = palette::census(&clear);
    assert_eq!(c.colors, vec![([20, 40, 200], 1)]);
    assert_eq!(c.modal.iter().flatten().count(), 1);
    // 400 distinct colours (one per 2×2 block of a 40×40 image) → 100 tracked, every opaque
    // pixel counted under one of them, the first met first.
    let many = Raster::from_fn(40, 40, |x, y| {
        let k = (y / 2) * 20 + x / 2;
        [
            (k % 256) as u8,
            (k / 2 % 256) as u8,
            (k * 3 % 256) as u8,
            255,
        ]
    })
    .expect("size");
    let c = palette::census(&many);
    assert_eq!(c.colors.len(), CENSUS_COLORS);
    assert_eq!(c.pixels(), 1600);
    let [r, g, b, _] = many.pixel(0, 0);
    assert_eq!(c.colors[0].0, [r, g, b]);
    // The palette reduces the census, weighted by its counts.
    let pal = Palette::from_census(&c, 4);
    assert!(pal.threads.len() <= 4 && !pal.threads.is_empty());
    assert_eq!(pal, Palette::from_census(&c, 4), "deterministic");
}

/// #127: the small-region floor is `max(4, round(param · 100 / mmPerPx²))`, in pixels, with
/// the parameter in mm² (`param` in cm²); regions of at most that many pixels go.
#[test]
fn small_region_floor_formula() {
    assert_eq!(
        small_region_floor(1.0, 1.0),
        4,
        "1 px rounds up to the 4 px floor"
    );
    assert_eq!(small_region_floor(1.0, 0.1), 100);
    assert_eq!(small_region_floor(4.5, 1.0), 5, "rounded, not truncated");
    assert_eq!(small_region_floor(4.4, 1.0), 4);
    assert_eq!(small_region_floor(0.0, 0.25), 4);
    assert_eq!(small_region_floor(1.0, 0.0), 4);
    assert_eq!(small_region_floor(-3.0, 0.5), 4);
    assert_eq!(small_region_floor(f64::NAN, 0.5), 4);
    // 0.25 mm² at 0.1 mm/px = 25 px; 1 cm² (param 1) at 0.5 mm/px = 400 px.
    assert_eq!(small_region_floor(0.25, 0.1), 25);
    assert_eq!(small_region_floor(100.0, 0.5), 400);
}

/// #127: a region of at most the floor's pixels is recoloured into its neighbour; one pixel
/// more stays. 4-connectivity: the modal filter leaves a 3×3 speck as a 5-pixel plus.
#[test]
fn regions_at_the_floor_go_and_above_it_stay() {
    let img = rects(30, 30, RED, &[(10, 10, 13, 13, BLUE)]);
    let frame = Frame::new(0.0, 0.0, 30.0, 30); // 1 mm per pixel
    let convert = |min_area_mm2: f64| {
        autopunch::convert(
            &img,
            &frame,
            &AutoPunch {
                colors: 2,
                skip_background: false,
                min_area_mm2,
                ..Default::default()
            },
        )
        .expect("converts")
    };
    // Floor 4 (1 mm² → 1 px → 4): the 5-pixel plus survives as a second region.
    let kept = convert(1.0);
    assert_eq!(kept.objects.len(), 2, "{:?}", kept.threads);
    // Floor 5: a 5-pixel region is at the floor and goes.
    let gone = convert(5.0);
    assert_eq!(gone.objects.len(), 1);
    assert_eq!(
        gone.objects[0].contours.len(),
        1,
        "no hole where the speck was"
    );
    // Two blue pixels touching only at a corner are two 1-pixel regions at 4-connectivity,
    // both under the floor, even with a floor-sized total.
    let corner = rects(
        30,
        30,
        RED,
        &[(10, 10, 12, 12, BLUE), (12, 12, 14, 14, BLUE)],
    );
    let out = autopunch::convert(
        &corner,
        &frame,
        &AutoPunch {
            colors: 2,
            skip_background: false,
            min_area_mm2: 1.0,
            ..Default::default()
        },
    )
    .expect("converts");
    assert_eq!(out.objects.len(), 1, "specks gone");
}

/// #127: satin or fill is decided by the region's stroke width, not by its longest scan span:
/// a bent 4 mm stroke is satin (its span across the principal axis is long), a 10 mm blob is
/// fill, and a long thin bar stays satin across its length.
#[test]
fn satin_is_decided_by_stroke_width() {
    let frame = Frame::new(0.0, 0.0, 60.0, 60); // 1 mm per pixel
    let p = AutoPunch {
        colors: 2,
        ..Default::default()
    };
    // An L: 4 px wide, 30 px along each leg.
    let bent = rects(
        60,
        60,
        WHITE,
        &[(10, 10, 14, 40, BLUE), (10, 36, 40, 40, BLUE)],
    );
    let out = autopunch::convert(&bent, &frame, &p).expect("converts");
    assert_eq!(out.objects.len(), 1);
    let r = out.objects[0].region.as_ref().expect("region");
    assert_eq!(
        r.kind,
        RegionType::Satin,
        "a 4 mm stroke is satin however it bends"
    );
    // A 10×10 blob is wider than 6 mm: fill.
    let blob = rects(60, 60, WHITE, &[(20, 20, 30, 30, BLUE)]);
    let out = autopunch::convert(&blob, &frame, &p).expect("converts");
    let r = out.objects[0].region.as_ref().expect("region");
    assert_eq!(r.kind, RegionType::Fill);
    // A 6 mm wide, 40 mm long bar is satin at 6 mm; a 7 mm one is fill.
    for (wide, kind) in [(6, RegionType::Satin), (7, RegionType::Fill)] {
        let bar = rects(60, 60, WHITE, &[(10, 20, 50, 20 + wide, BLUE)]);
        let out = autopunch::convert(&bar, &frame, &p).expect("converts");
        let r = out.objects[0].region.as_ref().expect("region");
        assert_eq!(r.kind, kind, "{wide} mm bar");
        if kind == RegionType::Satin {
            assert_eq!(r.angle, 90.0, "stitches run across the bar");
        }
    }
}

/// Black horizontal bars on white as the round-4 bar captures' inputs were drawn (pe-refs-4's
/// `wizimg.py`, 10 px/mm): an image `w_mm` × `h_mm`, bars from x = 10 mm `len_mm` long, the
/// first at y = 5 mm, `gap_mm` apart.
fn bars(w_mm: u32, h_mm: u32, len_mm: u32, gap_mm: u32, widths: &[f64]) -> Raster {
    let mut rs = Vec::new();
    let mut y = 50;
    for &w in widths {
        let h = (w * 10.0).round() as u32;
        rs.push((100, y, 100 + len_mm * 10, y + h, BLACK));
        y += h + gap_mm * 10;
    }
    rects(w_mm * 10, h_mm * 10, WHITE, &rs)
}

/// PE-DESIGN 11.50 on the same inputs (round-4 `autopunch-thin-strokes.pes`: 0.75–4 mm satin
/// with Auto direction and no under sewing, 0.5 mm fill with under sewing, 2018 stitches;
/// `autopunch-thin-strokes-wide.pes`: 5 and 6 mm satin, 7–12 mm fill across the bar at 90°,
/// 14 mm fill at 45°, 3197 stitches); one colour; the widest bar sewn first.
#[test]
fn bar_widths_choose_satin_and_fill_as_pe_design() {
    let p = AutoPunch::default();
    let widths = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0];
    let thin = bars(60, 70, 40, 5, &widths);
    let out =
        autopunch::convert(&thin, &Frame::new(0.0, 0.0, 59.9, thin.width()), &p).expect("converts");
    assert_eq!(out.threads.len(), 1, "automatic colours: black only");
    let kinds: Vec<(RegionType, bool, bool)> = out
        .objects
        .iter()
        .map(|o| {
            let r = o.region.as_ref().expect("region");
            (r.kind, r.auto_direction, r.underlay)
        })
        .collect();
    // Widest first: 4 mm … 0.75 mm satin, then the 0.5 mm fill.
    let mut want = vec![(RegionType::Satin, true, false); 8];
    want.push((RegionType::Fill, false, true));
    assert_eq!(kinds, want);
    let wide = bars(50, 96, 30, 4, &[5.0, 6.0, 7.0, 8.0, 10.0, 12.0, 14.0]);
    let out =
        autopunch::convert(&wide, &Frame::new(0.0, 0.0, 50.0, wide.width()), &p).expect("converts");
    let got: Vec<(RegionType, f64)> = out
        .objects
        .iter()
        .map(|o| {
            let r = o.region.as_ref().expect("region");
            (r.kind, r.angle)
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (RegionType::Fill, 45.0),
            (RegionType::Fill, 90.0),
            (RegionType::Fill, 90.0),
            (RegionType::Fill, 90.0),
            (RegionType::Fill, 90.0),
            (RegionType::Satin, 90.0),
            (RegionType::Satin, 90.0),
        ]
    );
}

/// The owner's four shapes (red disc, green triangle, yellow rectangle, blue square on white)
/// in two layouts with every shape moved and recoloured.
fn shapes(moved: bool) -> Raster {
    let (r, g, b, y) = (
        [220, 30, 30, 255],
        [30, 160, 60, 255],
        [30, 60, 200, 255],
        [240, 200, 40, 255],
    );
    Raster::from_fn(240, 240, |px, py| {
        let (fx, fy) = (f64::from(px), f64::from(py));
        let disc = |cx: f64, cy: f64, rad: f64| (fx - cx).powi(2) + (fy - cy).powi(2) < rad * rad;
        let tri = |x0: f64, x1: f64, base: f64, apex: f64| {
            let t = (fy - apex) / (base - apex);
            (0.0..=1.0).contains(&t) && (fx - (x0 + x1) / 2.0).abs() <= t * (x1 - x0) / 2.0
        };
        let rect = |x0, y0, x1, y1| px >= x0 && px < x1 && py >= y0 && py < y1;
        let hit = if moved {
            [
                (rect(20, 20, 100, 100), g),
                (rect(130, 40, 210, 90), b),
                (tri(20.0, 120.0, 220.0, 150.0), r),
                (disc(170.0, 170.0, 49.0), y),
            ]
        } else {
            [
                (disc(60.0, 60.0, 49.0), r),
                (tri(130.0, 230.0, 100.0, 30.0), g),
                (rect(20, 150, 100, 200), y),
                (rect(130, 130, 210, 210), b),
            ]
        };
        hit.iter().find(|h| h.0).map_or(WHITE, |h| h.1)
    })
    .expect("size")
}

/// PE-DESIGN 11.50 sews the four colours yellow, blue, red, green on both layouts
/// (`autopunch-default.pes`, `autopunch-owner-shapes-moved.pes`): by the colour's R + G + B,
/// highest first, not by area or position; and finds 4 colours plus white by itself.
#[test]
fn colours_sew_in_pe_design_order_wherever_they_are() {
    for moved in [false, true] {
        let out = autopunch::convert(
            &shapes(moved),
            &Frame::new(0.0, 0.0, 84.7, 240),
            &AutoPunch::default(),
        )
        .expect("converts");
        let order: Vec<String> = out
            .threads
            .iter()
            .map(|t| t.name.as_deref().unwrap_or_default().to_string())
            .collect();
        assert_eq!(out.threads.len(), 4, "{order:?}");
        let names = |t: &openstitch_core::Thread| (t.red, t.green, t.blue);
        let (y, b, r, g) = (
            names(&out.threads[0]),
            names(&out.threads[1]),
            names(&out.threads[2]),
            names(&out.threads[3]),
        );
        assert!(
            y.0 > 200 && y.1 > 150 && y.2 < 100,
            "yellow first: {order:?}"
        );
        assert!(b.2 > 150 && b.0 < 60, "blue second: {order:?}");
        assert!(r.0 > 200 && r.1 < 60, "red third: {order:?}");
        assert!(g.1 > 100 && g.0 < 80 && g.2 < 80, "green last: {order:?}");
        assert!(
            out.objects
                .iter()
                .all(|o| { o.region.as_ref().map(|r| r.kind) == Some(RegionType::Fill) })
        );
    }
}

/// The working copy keeps the image's own colours (most common colour per box), so a black
/// bar on white never gains grey edge regions.
#[test]
fn working_copy_adds_no_blended_colours() {
    let img = bars(50, 20, 40, 5, &[0.75, 2.0]);
    let small = img.fit_mode(120);
    assert!(small.width() <= 120 && small.height() <= 120);
    for y in 0..small.height() {
        for x in 0..small.width() {
            let p = small.pixel(x, y);
            assert!(p == BLACK || p == WHITE, "{p:?} at {x},{y}");
        }
    }
    assert_eq!(img.fit_mode(10_000), img);
    // The census of a picture with five flat colours counts five; a photo-like gradient is
    // capped.
    let c = palette::census(&shapes(false).fit_mode(400));
    assert_eq!(c.auto_colors(), 5);
    let ramp = Raster::from_fn(256, 64, |x, _| [x as u8, (255 - x) as u8, 128, 255]).expect("size");
    assert!(palette::census(&ramp).auto_colors() <= palette::AUTO_MAX);
}

/// #127 leftover: the satin bounds hold at every working resolution. The chamfer width read an
/// even-width stroke one pixel short, so the captured bars' 0.75 mm bar turned fill at 300, 350,
/// 500, 700, 800 and 960 px and the 7 mm bar satin at 250 px; only 400 and 600 matched PE.
#[test]
fn bar_choices_hold_at_every_working_resolution() {
    let thin = bars(
        60,
        70,
        40,
        5,
        &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0],
    );
    let wide = bars(50, 96, 30, 4, &[5.0, 6.0, 7.0, 8.0, 10.0, 12.0, 14.0]);
    let kinds = |img: &Raster, w_mm: f64, resolution: u32| -> Vec<RegionType> {
        let p = AutoPunch {
            resolution,
            ..Default::default()
        };
        autopunch::convert(img, &Frame::new(0.0, 0.0, w_mm, img.width()), &p)
            .expect("converts")
            .objects
            .iter()
            .map(|o| o.region.as_ref().expect("region").kind)
            .collect()
    };
    let mut thin_want = vec![RegionType::Satin; 8];
    thin_want.push(RegionType::Fill);
    let mut wide_want = vec![RegionType::Fill; 5];
    wide_want.extend([RegionType::Satin; 2]);
    for resolution in [250, 300, 500, 800] {
        assert_eq!(
            kinds(&thin, 59.9, resolution),
            thin_want,
            "thin at {resolution} px"
        );
        assert_eq!(
            kinds(&wide, 50.0, resolution),
            wide_want,
            "wide at {resolution} px"
        );
    }
}

/// Stroke widths measure exactly whether they are an odd or an even number of pixels.
#[test]
fn stroke_width_is_exact_for_odd_and_even_widths() {
    for w in 1..=12usize {
        let (mw, mh) = (60, w + 4);
        let m: Vec<bool> = (0..mh)
            .flat_map(|y| (0..mw).map(move |x| (5..55).contains(&x) && (2..2 + w).contains(&y)))
            .collect();
        assert_eq!(regions::thickness(&m, mw, mh), w as f64, "{w} px bar");
    }
}

/// The specks image of PE-DESIGN 11.50's round-4 capture `autopunch-specks-default.pes`, as
/// pe-refs-4's `wizimg.py` draws it: 240 × 240 px (84.7 mm), a red square and a blue disc, and
/// square specks of 1, 2, 3, 5 and 8 px in a red row and a blue row, on white.
fn specks() -> Raster {
    const R: [u8; 4] = [220, 30, 30, 255];
    const B: [u8; 4] = [30, 60, 200, 255];
    Raster::from_fn(240, 240, |x, y| {
        let (fx, fy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
        if (20..100).contains(&x) && (20..100).contains(&y) {
            return R;
        }
        if (fx - 180.0).powi(2) + (fy - 60.0).powi(2) <= 40.0 * 40.0 {
            return B;
        }
        for (colour, row) in [(R, 150u32), (B, 200u32)] {
            for (size, col) in [(1u32, 30u32), (2, 70), (3, 110), (5, 150), (8, 190)] {
                if (col..col + size).contains(&x) && (row..row + size).contains(&y) {
                    return colour;
                }
            }
        }
        WHITE
    })
    .expect("size")
}

/// PE-DESIGN 11.50 on the specks (round-4 `autopunch-specks-default.pes`; Noise reduction one
/// tick up gives the same file): the 1, 2 and 3 px specks go, the 5 and 8 px specks stay, in
/// both colours. Blue sews first, from the bottom specks (8 px, then 5 px) up to its disc;
/// red then sews its square and down to its 5 px and 8 px specks. The square fills at 45°,
/// the disc at 90°.
#[test]
fn specks_survive_and_sew_as_pe_design() {
    let out = autopunch::convert(
        &specks(),
        &Frame::new(0.0, 0.0, 84.7, 240),
        &AutoPunch::default(),
    )
    .expect("converts");
    let sizes: Vec<(bool, i64, Option<f64>)> = out
        .objects
        .iter()
        .map(|o| {
            let r = o.region.as_ref().expect("region");
            let xs: Vec<f64> = o
                .contours
                .iter()
                .flat_map(|c| c.path())
                .map(|p| p.x)
                .collect();
            let w = xs.iter().copied().fold(f64::MIN, f64::max)
                - xs.iter().copied().fold(f64::MAX, f64::min);
            let fill = (r.kind == RegionType::Fill).then_some(r.angle);
            (
                r.thread.blue > r.thread.red,
                (w / 10.0).round() as i64,
                fill,
            )
        })
        .collect();
    // (blue?, width in whole mm, fill angle): blue 8 px (2.8 mm), 5 px (1.8 mm), disc; then
    // red square, 5 px, 8 px.
    assert_eq!(
        sizes,
        vec![
            (true, 3, None),
            (true, 2, None),
            (true, 28, Some(90.0)),
            (false, 28, Some(45.0)),
            (false, 2, None),
            (false, 3, None),
        ]
    );
}

/// PE-DESIGN 11.50 fills a right triangle at 45° (round-4 `autopunch-right-triangle.pes`): its
/// rows then spread across the hypotenuse's 80 mm against 57 mm at 90°.
#[test]
fn right_triangle_fills_at_45_degrees() {
    let img = Raster::from_fn(240, 240, |x, y| {
        // (40,40) (40,200) (200,200): the right angle at the bottom left.
        if x >= 40 && y <= 200 && x <= y && y >= 40 {
            [220, 30, 30, 255]
        } else {
            WHITE
        }
    })
    .expect("size");
    let out = autopunch::convert(
        &img,
        &Frame::new(0.0, 0.0, 84.7, 240),
        &AutoPunch::default(),
    )
    .expect("converts");
    assert_eq!(out.objects.len(), 1);
    let r = out.objects[0].region.as_ref().expect("region");
    assert_eq!((r.kind, r.angle), (RegionType::Fill, 45.0));
}

/// The fill direction rule on its own: the direction whose rows spread furthest, a diagonal
/// winning over 90° only with at least one more row (0.22 mm).
#[test]
fn fill_direction_spreads_the_rows_furthest() {
    let mask = |w: usize, h: usize, f: &dyn Fn(usize, usize) -> bool| -> Vec<bool> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect()
    };
    let px = 0.1; // mm per pixel
    // 30 mm bars: 12 mm wide at 90°, 14 mm wide at 45° (PE's wide-bar capture).
    let bar12 = mask(300, 120, &|_, _| true);
    assert_eq!(autopunch::fill_angle(&bar12, 300, 120, px), 90.0);
    let bar14 = mask(300, 140, &|_, _| true);
    assert_eq!(autopunch::fill_angle(&bar14, 300, 140, px), 45.0);
    // A square at 45° (its diagonal), not 135° (an equal spread: the earlier angle wins).
    let square = mask(200, 200, &|_, _| true);
    assert_eq!(autopunch::fill_angle(&square, 200, 200, px), 45.0);
    // An isosceles triangle, base down, wider than tall: 90°.
    let tri = mask(356, 248, &|x, y| {
        let half = 178.0 * (y as f64 + 0.5) / 248.0;
        (x as f64 + 0.5 - 178.0).abs() <= half
    });
    assert_eq!(autopunch::fill_angle(&tri, 356, 248, px), 90.0);
    // A disc is a near tie: 90°.
    let disc = mask(200, 200, &|x, y| {
        (x as f64 + 0.5 - 100.0).powi(2) + (y as f64 + 0.5 - 100.0).powi(2) <= 100.0 * 100.0
    });
    assert_eq!(autopunch::fill_angle(&disc, 200, 200, px), 90.0);
}
