mod common;

use common::*;
use openstitch_core::{Command, Thread};
use openstitch_design::DesignObject;
use openstitch_digitize::photo::{self, PhotoMode, PhotoStitch, PhotoStyle};
use openstitch_digitize::{DigitizeError, Frame, Raster};

/// 100×20 px, black on the left fading to white on the right.
fn gradient() -> Raster {
    Raster::from_fn(100, 20, |x, _| {
        let v = (f64::from(x) / 99.0 * 255.0).round() as u8;
        [v, v, v, 255]
    })
    .expect("size")
}

fn needles(o: &DesignObject) -> Vec<(f64, f64)> {
    o.stitch_block
        .as_ref()
        .expect("block")
        .stitches
        .iter()
        .filter(|s| s.command == Command::Stitch)
        .map(|s| (s.x, s.y))
        .collect()
}

fn black() -> Thread {
    Thread::rgb(0)
}

#[test]
fn mono_fill_density_follows_tone() {
    // 50 mm wide: x 0..500 in 0.1 mm.
    let frame = Frame::new(0.0, 0.0, 50.0, 100);
    let p = PhotoStitch::new(PhotoStyle::Fill, PhotoMode::Mono(black()));
    let out = photo::convert(&gradient(), &frame, &p).expect("converts");
    assert_eq!(out.objects.len(), 1);
    let pts = needles(&out.objects[0]);
    // Sewn length per band (stitches within one row only).
    let stitches = &out.objects[0]
        .stitch_block
        .as_ref()
        .expect("block")
        .stitches;
    let band = |a: f64, b: f64| -> f64 {
        stitches
            .windows(2)
            .filter(|w| w[0].command == Command::Stitch && w[1].command == Command::Stitch)
            .filter(|w| (w[0].x + w[1].x) / 2.0 >= a && (w[0].x + w[1].x) / 2.0 < b)
            .map(|w| (w[1].x - w[0].x).abs())
            .sum()
    };
    let (dark, mid, light) = (band(0.0, 100.0), band(200.0, 300.0), band(420.0, 500.0));
    assert!(dark > mid && mid > light, "{dark} {mid} {light}");
    assert!(
        pts.iter()
            .all(|p| p.0 >= 0.0 && p.0 <= 500.0 && p.1 >= 0.0 && p.1 <= 100.0)
    );
    assert_eq!(
        out,
        photo::convert(&gradient(), &frame, &p).expect("converts"),
        "deterministic"
    );
}

#[test]
fn colour_fill_separates_threads() {
    let img = rects(40, 20, RED, &[(20, 0, 40, 20, BLUE)]);
    let frame = Frame::new(0.0, 0.0, 40.0, 40);
    let p = PhotoStitch::new(PhotoStyle::Fill, PhotoMode::Color(2));
    let out = photo::convert(&img, &frame, &p).expect("converts");
    assert_eq!(out.objects.len(), 2);
    for (o, t) in out.objects.iter().zip(&out.threads) {
        let pts = needles(o);
        assert!(!pts.is_empty());
        // Error diffusion may spill a cell across the edge; nearly all stay on their side.
        let share = |f: &dyn Fn(f64) -> bool| {
            pts.iter().filter(|p| f(p.0)).count() as f64 / pts.len() as f64
        };
        if t.red > t.blue {
            assert!(share(&|x| x <= 200.0 + 1e-9) > 0.9, "red stays left");
        } else {
            assert!(share(&|x| x >= 200.0 - 1e-9) > 0.9, "blue stays right");
        }
    }
}

#[test]
fn mono_lines_swing_with_darkness_and_break_on_white() {
    let frame = Frame::new(0.0, 0.0, 50.0, 100);
    let p = PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Mono(black()));
    let out = photo::convert(&gradient(), &frame, &p).expect("converts");
    let pts = needles(&out.objects[0]);
    // 10 mm high at 1.2 mm spacing → 8 rows of 1.25 mm.
    let spacing = 12.5;
    let swing = |a: f64, b: f64| {
        pts.iter()
            .filter(|p| p.0 >= a && p.0 < b)
            .map(|p| {
                let centre = ((p.1 / spacing - 0.5).round() + 0.5) * spacing;
                (p.1 - centre).abs()
            })
            .fold(0.0, f64::max)
    };
    assert!(swing(0.0, 100.0) > 4.0);
    assert!(swing(0.0, 100.0) > swing(250.0, 350.0));
    assert_eq!(
        pts.iter().filter(|p| p.0 > 470.0).count(),
        0,
        "white breaks the lines"
    );
}

#[test]
fn colour_lines_one_layer_per_thread() {
    let img = rects(40, 20, RED, &[(20, 0, 40, 20, BLUE)]);
    let p = PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(2));
    let out = photo::convert(&img, &Frame::new(0.0, 0.0, 40.0, 40), &p).expect("converts");
    assert_eq!(out.objects.len(), 2);
    assert_eq!(out.stitches.len(), 2);
    assert!(out.stitches.iter().all(|&n| n > 10));
}

#[test]
fn hostile_inputs_fail_cleanly() {
    let p = PhotoStitch::new(PhotoStyle::Fill, PhotoMode::Mono(black()));
    let img = rects(4, 4, BLACK, &[]);
    assert!(matches!(
        photo::convert(&img, &Frame::new(0.0, 0.0, 2000.0, 4), &p),
        Err(DigitizeError::TooLarge(_))
    ));
    let frame = Frame::new(0.0, 0.0, 10.0, 4);
    assert!(matches!(
        photo::convert(&rects(4, 4, CLEAR, &[]), &frame, &p),
        Err(DigitizeError::Empty(_))
    ));
    assert!(matches!(
        photo::convert(&rects(4, 4, WHITE, &[]), &frame, &p),
        Err(DigitizeError::Empty(_))
    ));
    let colour = PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(4));
    assert!(matches!(
        photo::convert(&rects(4, 4, WHITE, &[]), &frame, &colour),
        Err(DigitizeError::Empty(_))
    ));
}
