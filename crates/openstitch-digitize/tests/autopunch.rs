mod common;

use common::*;
use openstitch_design::{Compiler, Design, DesignPage, RegionType};
use openstitch_digitize::autopunch::{self, AutoPunch};
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
