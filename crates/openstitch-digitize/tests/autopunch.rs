mod common;

use common::*;
use openstitch_design::{Compiler, Design, DesignPage, RegionType};
use openstitch_digitize::autopunch::{self, AutoPunch, small_region_floor};
use openstitch_digitize::palette::{self, CENSUS_COLORS, Palette};
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
