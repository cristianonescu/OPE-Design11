mod common;

use common::*;
use openstitch_core::{Command, Thread};
use openstitch_design::DesignObject;
use openstitch_digitize::photo::{self, PhotoMode, PhotoStitch, PhotoStyle};
use openstitch_digitize::raster::luma601;
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

/// Settings that leave the fabric bare (Sew Page Color off), so tone shows as coverage.
fn bare(style: PhotoStyle, mode: PhotoMode) -> PhotoStitch {
    PhotoStitch {
        sew_page: false,
        ..PhotoStitch::new(style, mode)
    }
}

#[test]
fn mono_fill_density_follows_tone() {
    // 50 mm wide: x 0..500 in 0.1 mm.
    let frame = Frame::new(0.0, 0.0, 50.0, 100);
    let p = bare(PhotoStyle::Fill, PhotoMode::Mono(black()));
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
    // 2 mm cells, so the colour edge at 20 mm falls between cells.
    let p = PhotoStitch {
        stitch_mm: 2.0,
        row_spacing_mm: 0.4,
        ..bare(PhotoStyle::Fill, PhotoMode::Color(2))
    };
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
    let p = bare(PhotoStyle::Fill, PhotoMode::Mono(black()));
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

/// #128: integer Rec. 601 luminance, `(30 R + 59 G + 11 B) / 100`.
#[test]
fn luminance_uses_the_30_59_11_weights() {
    assert_eq!(luma601(255, 0, 0), 76);
    assert_eq!(luma601(0, 255, 0), 150);
    assert_eq!(luma601(0, 0, 255), 28);
    assert_eq!(luma601(255, 255, 255), 255);
    assert_eq!(luma601(0, 0, 0), 0);
    // Truncated, not rounded: (30·1 + 59·2 + 11·3) / 100 = 1.81 → 1.
    assert_eq!(luma601(1, 2, 3), 1);
    assert_eq!(photo::luma([255.0, 0.0, 0.0]), 76.0);
}

/// #128: the mono stretch is full min/max auto-levels with the (1, 255) range guard.
#[test]
fn mono_stretch_is_full_range_auto_levels() {
    // A gradient 40..=200 maps onto 0..=255 by (g − 40) · 255 / 160.
    let mut g: Vec<u8> = (40..=200).collect();
    photo::auto_levels(&mut g);
    assert_eq!(g[0], 0);
    assert_eq!(g.last().copied(), Some(255));
    assert_eq!(g[80], (80 * 255 / 160) as u8);
    assert_eq!(g[1], 1, "(1 · 255) / 160 truncates to 1");
    // Every level is reached in order: a monotone map with no shuffling.
    assert!(g.windows(2).all(|w| w[0] <= w[1]));
    // Range 2 stretches (the smallest range that does).
    let mut two = vec![10u8, 11, 12];
    photo::auto_levels(&mut two);
    assert_eq!(two, vec![0, 127, 255]);
    // Range 1, range 0 and the full range are left alone.
    for untouched in [
        vec![100u8, 101],
        vec![7u8, 7, 7],
        vec![0u8, 128, 255],
        vec![0u8, 255],
    ] {
        let mut g = untouched.clone();
        photo::auto_levels(&mut g);
        assert_eq!(g, untouched);
    }
    // A single sample is a flat image.
    let mut one = vec![77u8];
    photo::auto_levels(&mut one);
    assert_eq!(one, vec![77]);
}

/// #128: a low-contrast mono photo sews as densely as a full-contrast one, because the
/// stretch maps its darkest grey to black and its lightest to white.
#[test]
fn low_contrast_gradient_sews_like_the_full_one() {
    let frame = Frame::new(0.0, 0.0, 50.0, 100);
    let p = bare(PhotoStyle::Fill, PhotoMode::Mono(black()));
    // 40..=200 against 0..=255: after the stretch they are the same grey ramp.
    let narrow = Raster::from_fn(100, 20, |x, _| {
        let v = 40 + (f64::from(x) / 99.0 * 160.0).round() as u8;
        [v, v, v, 255]
    })
    .expect("size");
    let full = photo::convert(&gradient(), &frame, &p).expect("converts");
    let low = photo::convert(&narrow, &frame, &p).expect("converts");
    let (a, b) = (full.stitches[0] as f64, low.stitches[0] as f64);
    assert!((a - b).abs() / a < 0.03, "{a} vs {b} needle points");
    // The weights show in colour: pure red (luminance 76) sews denser than pure green (150).
    let halves = rects(
        100,
        20,
        [0, 255, 0, 255],
        &[(0, 0, 50, 20, [255, 0, 0, 255])],
    );
    let out = photo::convert(&halves, &frame, &p).expect("converts");
    let pts = needles(&out.objects[0]);
    let (red, green) = (
        pts.iter().filter(|p| p.0 < 250.0).count(),
        pts.iter().filter(|p| p.0 >= 250.0).count(),
    );
    assert!(red > green * 2, "red {red} green {green}");
    // The left (darkest) column is sewn solid, the right (lightest) column is bare, in both.
    for out in [&full, &low] {
        let pts = needles(&out.objects[0]);
        assert!(pts.iter().any(|p| p.0 < 10.0), "dark edge sewn");
        assert!(pts.iter().all(|p| p.0 < 495.0), "light edge bare");
    }
}

/// PE-DESIGN 11.50's Photo Stitch 1 defaults (round-4 capture `photostitch1-default-plus-cross.pes`):
/// Run Pitch 3.0 mm, 10 colours, Sew Page Color ticked: the page colour is a thread of its own,
/// sewn first, and every opaque cell is sewn.
#[test]
fn page_colour_is_sewn_first_by_default() {
    let p = PhotoStitch::new(PhotoStyle::Fill, PhotoMode::Color(photo::COLORS));
    assert!(p.sew_page);
    assert_eq!(p.stitch_mm, photo::RUN_PITCH_MM);
    assert_eq!(photo::RUN_PITCH_MM, 3.0);
    assert_eq!(photo::COLORS, 10);
    assert!(!PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(4)).sew_page);
    // A red square on white: the white is sewn in the chart's White, before the red.
    let img = rects(40, 40, WHITE, &[(10, 10, 30, 30, RED)]);
    let frame = Frame::new(0.0, 0.0, 40.0, 40);
    let out = photo::convert(&img, &frame, &p).expect("converts");
    assert!(out.threads.len() >= 2, "{:?}", out.threads);
    assert_eq!(out.threads[0].hex(), "#f0f0f0", "page colour first");
    let white = needles(&out.objects[0]);
    let colour: Vec<(f64, f64)> = out.objects[1..].iter().flat_map(needles).collect();
    let inside = |p: &&(f64, f64)| p.0 > 110.0 && p.0 < 290.0 && p.1 > 110.0 && p.1 < 290.0;
    assert!(
        white.iter().filter(inside).count() * 20 < white.len(),
        "white stays outside"
    );
    // Run ends sit on the square's edges, so allow one cell of slack there.
    let on_square = |p: &&(f64, f64)| p.0 > 70.0 && p.0 < 330.0 && p.1 > 95.0 && p.1 < 305.0;
    assert!(
        colour.iter().filter(on_square).count() * 10 > colour.len() * 9,
        "red stays inside"
    );
    // Both areas sewn about equally densely: 1200 mm² of white around 400 mm² of red.
    let (w, r) = (white.len() as f64 / 1200.0, colour.len() as f64 / 400.0);
    assert!(
        (w / r - 1.0).abs() < 0.3,
        "{w} vs {r} needle points per mm²"
    );
    // Off, the white stays bare.
    let off = PhotoStitch {
        sew_page: false,
        ..p
    };
    let bare_out = photo::convert(&img, &frame, &off).expect("converts");
    assert!(bare_out.threads.iter().all(|t| t.hex() != "#f0f0f0"));
}
