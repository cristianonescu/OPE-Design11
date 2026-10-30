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

/// Photo Stitch 2 Mono: one thread's line screen; the zigzag thickens with darkness and the
/// straight baseline runs on through white, as PE-DESIGN's screen covers the whole image.
#[test]
fn mono_screen_follows_darkness_and_runs_on_through_white() {
    let frame = Frame::new(0.0, 0.0, 50.0, 100);
    let p = PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Mono(black()));
    assert_eq!(
        (p.row_spacing_mm, p.stitch_mm, p.max_density),
        (2.0, 2.5, 4.5)
    );
    let out = photo::convert(&gradient(), &frame, &p).expect("converts");
    assert_eq!(out.objects.len(), 1);
    let pts = needles(&out.objects[0]);
    let band = |a: f64, b: f64| pts.iter().filter(|p| p.0 >= a && p.0 < b).count();
    let (dark, mid, light) = (band(0.0, 100.0), band(200.0, 300.0), band(400.0, 500.0));
    assert!(dark > mid && mid > light, "{dark} {mid} {light}");
    assert!(light > 0, "the baseline runs on through white");
    let b = out.objects[0].stitch_block.as_ref().expect("block");
    assert_eq!(
        b.stitches
            .iter()
            .filter(|s| s.command == Command::Jump)
            .count(),
        1,
        "one path"
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

/// PE-DESIGN 11.50's Photo Stitch 1 Mono has no Sew Page Color: light areas stay unsewn
/// (round-4 `photostitch1-mono-grey-tonal.pes`).
#[test]
fn mono_leaves_light_areas_unsewn_by_default() {
    let p = PhotoStitch::new(PhotoStyle::Fill, PhotoMode::Mono(black()));
    assert!(!p.sew_page);
    let out = photo::convert(&gradient(), &Frame::new(0.0, 0.0, 50.0, 100), &p).expect("converts");
    assert_eq!(out.threads.len(), 1);
    assert!(
        needles(&out.objects[0]).iter().all(|p| p.0 < 495.0),
        "light edge bare"
    );
}

/// Photo Stitch 1 sews a colour in few long paths, as PE-DESIGN 11.50 does (round-4 captures:
/// 1–5 jumps a colour): runs are visited nearest first, short hops to the next row turn like
/// a fill row, and only a run with nothing close is jumped to.
#[test]
fn runs_are_joined_nearest_first() {
    let frame = Frame::new(0.0, 0.0, 40.0, 40);
    let p = bare(PhotoStyle::Fill, PhotoMode::Mono(black()));
    let block = |out: &photo::Converted| out.objects[0].stitch_block.clone().expect("block");
    // A solid black square on white is one path: one jump in.
    let square = rects(40, 40, WHITE, &[(10, 10, 30, 30, BLACK)]);
    let out = photo::convert(&square, &frame, &p).expect("converts");
    let b = block(&out);
    assert_eq!(
        b.stitches
            .iter()
            .filter(|s| s.command == Command::Jump)
            .count(),
        1
    );
    // No travel outruns the join reach: two cells across and two rows down at most.
    let (cw, ch) = (p.stitch_mm * 10.0, p.row_spacing_mm * 10.0);
    let longest = photo::JOIN_CELLS * cw;
    for w in b.stitches.windows(2) {
        if w[1].command == Command::Stitch {
            let len = (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
            assert!(len <= longest.hypot(2.0 * ch) + 1.0, "{len} {w:?}");
        }
    }
    // A dithered mid-grey breaks into many runs, but they are sewn in far fewer paths.
    let grey = rects(40, 40, [128, 128, 128, 255], &[]);
    let out = photo::convert(&grey, &frame, &p).expect("converts");
    let b = block(&out);
    let jumps = b
        .stitches
        .iter()
        .filter(|s| s.command == Command::Jump)
        .count();
    let stitches = b.stitches.len() - jumps;
    assert!(
        jumps * 20 < stitches,
        "{jumps} jumps for {stitches} needle points"
    );
}

/// The thread's straight baseline stitches: 2.5 mm stitches continued by another 2.5 mm stitch
/// in the same direction (a running stitch, not a zigzag stitch that happens to be as long).
fn baseline(o: &DesignObject) -> Vec<((f64, f64), (f64, f64))> {
    let st = &o.stitch_block.as_ref().expect("block").stitches;
    st.windows(3)
        .filter(|w| w.iter().all(|s| s.command == Command::Stitch))
        .filter_map(|w| {
            let (d1, d2) = (
                (w[1].x - w[0].x, w[1].y - w[0].y),
                (w[2].x - w[1].x, w[2].y - w[1].y),
            );
            let (l1, l2) = (d1.0.hypot(d1.1), d2.0.hypot(d2.1));
            let straight = (d1.0 * d2.1 - d1.1 * d2.0).abs() < 0.02 * l1 * l2
                && d1.0 * d2.0 + d1.1 * d2.1 > 0.0;
            ((l1 - 25.0).abs() < 1.0 && (l2 - 25.0).abs() < 1.0 && straight)
                .then_some(((w[0].x, w[0].y), (w[1].x, w[1].y)))
        })
        .collect()
}

/// #129 acceptance check against PE-DESIGN 11.50's Photo Stitch 2 (round-4 capture
/// `photostitch2-color-owner-shapes.pes`, measured with tools/screencmp.py; criteria in
/// docs/research/photo-stitch-2-halftone.md), on the owner's four-shape image rebuilt here:
/// line angles 0/45/90/135° in sewing order within 3°, lines 2.0 ± 0.1 mm apart, each thread
/// within 10 % of PE's ≈297 needle points per cm² in its own colour area and within 25 % of
/// PE's ≈21 on white, one jump a thread, and the total within 10 % of PE's 14 133.
#[test]
fn line_screen_meets_pe_design_capture() {
    let img = shapes(false);
    let frame = Frame::new(0.0, 0.0, 84.7, 240);
    let p = PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(photo::LINE_COLORS));
    let out = photo::convert(&img, &frame, &p).expect("converts");
    assert_eq!(out.objects.len(), 4, "{:?}", out.threads);
    let total: usize = out.stitches.iter().sum();
    assert!(
        (total as f64 - 14_133.0).abs() / 14_133.0 < 0.10,
        "{total} stitches against PE's 14 133"
    );
    // Colour areas of the source image, in cm².
    let mm_per_px = 84.7 / 240.0;
    let area_of = |px: [u8; 4]| -> usize {
        [
            WHITE,
            [220, 30, 30, 255],
            [30, 60, 200, 255],
            [240, 200, 40, 255],
            [30, 160, 60, 255],
        ]
        .iter()
        .position(|c| *c == px)
        .unwrap_or(0)
    };
    let mut cm2 = [0.0f64; 5];
    for y in 0..240 {
        for x in 0..240 {
            cm2[area_of(img.pixel(x, y))] += mm_per_px * mm_per_px / 100.0;
        }
    }
    for (layer, o) in out.objects.iter().enumerate() {
        let b = o.stitch_block.as_ref().expect("block");
        assert_eq!(
            b.stitches
                .iter()
                .filter(|s| s.command == Command::Jump)
                .count(),
            1,
            "thread {layer}: one jump"
        );
        // Line angle: the straight baseline stitches run at the thread's screen angle.
        let want = [0.0, 45.0, 90.0, 135.0][layer];
        let runs = baseline(o);
        assert!(runs.len() > 50, "thread {layer} has a baseline");
        for (a, c) in &runs {
            let ang = (-(c.1 - a.1))
                .atan2(c.0 - a.0)
                .to_degrees()
                .rem_euclid(180.0);
            let off = ((ang - want + 90.0).rem_euclid(180.0) - 90.0).abs();
            assert!(off <= 3.0, "thread {layer}: {ang}° against {want}°");
        }
        // Line spacing: baseline needle points sit on lines 2.0 mm apart.
        let t = want.to_radians();
        let across = |p: (f64, f64)| p.0 * t.sin() + p.1 * t.cos();
        let first = across(runs[0].0);
        let on_grid = runs
            .iter()
            .filter(|(a, _)| {
                let k = (across(*a) - first) / 20.0;
                (k - k.round()).abs() * 20.0 <= 1.0
            })
            .count();
        assert!(
            on_grid * 20 >= runs.len() * 19,
            "thread {layer}: lines 2.0 mm apart"
        );
        // Needle points per cm² by colour area.
        let mut counts = [0usize; 5];
        for (x, y) in needles(o) {
            let (px, py) = ((x / 10.0 / mm_per_px) as u32, (y / 10.0 / mm_per_px) as u32);
            if px < 240 && py < 240 {
                counts[area_of(img.pixel(px, py))] += 1;
            }
        }
        let dens: Vec<f64> = (0..5).map(|k| counts[k] as f64 / cm2[k]).collect();
        let own = dens[1..].iter().copied().fold(0.0, f64::max);
        assert!(
            (own - 297.0).abs() / 297.0 < 0.10,
            "thread {layer}: own area {own:.0}/cm²"
        );
        assert!(
            (dens[0] - 21.0).abs() / 21.0 < 0.25,
            "thread {layer}: {:.0}/cm² on white",
            dens[0]
        );
    }
}

/// The screen's colour shares: a thread's own colour is all that thread, a colour between two
/// threads is shared, and the fabric colour is none.
#[test]
fn screen_shares_mix_threads_over_the_fabric() {
    use openstitch_digitize::screen::mix;
    let (red, blue, white) = ([220.0, 30.0, 30.0], [30.0, 60.0, 200.0], [255.0; 3]);
    let s = mix(red, &[red, blue], white);
    assert!((s[0] - 1.0).abs() < 1e-3 && s[1] < 1e-3, "{s:?}");
    let s = mix(white, &[red, blue], white);
    assert!(s.iter().all(|v| *v < 1e-6), "{s:?}");
    let half = [0, 1, 2].map(|k| (red[k] + blue[k]) / 2.0);
    let s = mix(half, &[red, blue], white);
    assert!(
        (s[0] - 0.5).abs() < 0.02 && (s[1] - 0.5).abs() < 0.02,
        "{s:?}"
    );
    assert!(s.iter().sum::<f64>() <= 1.0 + 1e-9);
}

/// The screen angles are a fixed set in sewing order, turned by the Direction setting.
#[test]
fn screen_angles_are_a_fixed_set() {
    use openstitch_digitize::screen::angle;
    assert_eq!(
        (0..4).map(|l| angle(l, 0.0)).collect::<Vec<_>>(),
        vec![0.0, 45.0, 90.0, 135.0]
    );
    assert_eq!(angle(4, 0.0), 22.5);
    assert_eq!(angle(0, 30.0), 30.0);
    assert_eq!(angle(3, 60.0), 15.0);
}

/// PE-DESIGN 11.50's named Color Sets (round-4 `photostitch2-cmyk/-rgbk/-crvk/-bmyk.pes`): four
/// fixed chart threads sewn in the set's order, the screen angles following that order.
#[test]
fn named_color_sets_sew_their_threads_in_order() {
    use openstitch_digitize::photo::ColorSet;
    let img = shapes(false);
    let frame = Frame::new(0.0, 0.0, 84.7, 240);
    for (set, pec) in [
        (ColorSet::Cmyk, ["32", "51", "13", "20"]),
        (ColorSet::Rgbk, ["5", "2", "14", "20"]),
        (ColorSet::Cryk, ["32", "5", "13", "20"]),
        (ColorSet::Bmyk, ["2", "51", "13", "20"]),
    ] {
        let p = PhotoStitch {
            color_set: set,
            ..PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(photo::LINE_COLORS))
        };
        let out = photo::convert(&img, &frame, &p).expect("converts");
        let got: Vec<&str> = out
            .threads
            .iter()
            .map(|t| t.catalog_number.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(got, pec, "{set:?}");
        for (layer, o) in out.objects.iter().enumerate() {
            let want = [0.0, 45.0, 90.0, 135.0][layer];
            for (a, c) in baseline(o) {
                let ang = (-(c.1 - a.1))
                    .atan2(c.0 - a.0)
                    .to_degrees()
                    .rem_euclid(180.0);
                let off = ((ang - want + 90.0).rem_euclid(180.0) - 90.0).abs();
                assert!(off <= 3.0, "{set:?} thread {layer}: {ang}°");
            }
        }
    }
    // Auto Select still reduces the image's own colours.
    assert_eq!(
        PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(4)).color_set,
        ColorSet::Auto
    );
}

/// Direction turns the whole screen (PE-DESIGN's Direction 30° capture: 30°, 75°, 120°, 165°),
/// and the Line interval and Max. Density set the spacing and the zigzag density (its capture at
/// 3.0 mm and 2.5 lines/mm: lines 3.0 mm apart, about 112 needle points per cm² in a thread's
/// own area against 297 at the defaults).
#[test]
fn direction_interval_and_density_shape_the_screen() {
    let img = shapes(false);
    let frame = Frame::new(0.0, 0.0, 84.7, 240);
    let base = PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(4));
    let turned = photo::convert(
        &img,
        &frame,
        &PhotoStitch {
            direction_deg: 30.0,
            ..base.clone()
        },
    )
    .expect("converts");
    for (layer, o) in turned.objects.iter().enumerate() {
        let want = [30.0, 75.0, 120.0, 165.0][layer];
        let runs = baseline(o);
        assert!(!runs.is_empty());
        for (a, c) in runs {
            let ang = (-(c.1 - a.1))
                .atan2(c.0 - a.0)
                .to_degrees()
                .rem_euclid(180.0);
            let off = ((ang - want + 90.0).rem_euclid(180.0) - 90.0).abs();
            assert!(off <= 3.0, "thread {layer}: {ang}° against {want}°");
        }
    }
    // Coarser: 3 mm lines at 2.5 lines/mm sew about 6800 stitches where PE sews 6842.
    let coarse = photo::convert(
        &img,
        &frame,
        &PhotoStitch {
            row_spacing_mm: 3.0,
            max_density: 2.5,
            ..base
        },
    )
    .expect("converts");
    let total: usize = coarse.stitches.iter().sum();
    assert!(
        (total as f64 - 6842.0).abs() / 6842.0 < 0.10,
        "{total} against PE's 6842"
    );
}

/// PE-DESIGN's Photo Stitch 2 Mono (round-4 grey capture): no zigzag below 17 % darkness, full
/// density from 86 %.
#[test]
fn mono_screen_ramps_from_light_to_full() {
    use openstitch_digitize::screen::mono_share;
    assert_eq!(mono_share(0.0), 0.0);
    assert_eq!(mono_share(0.17), 0.0);
    assert!((mono_share(0.515) - 0.5).abs() < 1e-9);
    assert_eq!(mono_share(0.86), 1.0);
    assert_eq!(mono_share(1.0), 1.0);
}

/// PE-DESIGN's Photo Stitch 2 sews the thread its screen uses most first (round-4 ramp captures
/// `photostitch2-ramp-cmyk/-rgbk.pes`: a hue ramp red → magenta sews CMYK as cyan, yellow,
/// magenta, black and RGBK as blue, lime green, red, black, where the owner's shapes sew them
/// cyan, magenta, yellow, black and red, blue, lime green, black).
#[test]
fn screen_sews_the_most_used_thread_first() {
    use openstitch_digitize::photo::ColorSet;
    // The ramp as pe-refs-4's wizimg.py draws it: 800 × 200 px, HSV hue 0–300° across, S = V = 1.
    let ramp = Raster::from_fn(800, 200, |x, _| {
        let h = 300.0 * f64::from(x) / 799.0 / 60.0;
        let (i, f) = (h.floor() as i32, h - h.floor());
        let (q, t) = (1.0 - f, f);
        let (r, g, b) = match i {
            0 => (1.0, t, 0.0),
            1 => (q, 1.0, 0.0),
            2 => (0.0, 1.0, t),
            3 => (0.0, q, 1.0),
            4 => (t, 0.0, 1.0),
            _ => (1.0, 0.0, q),
        };
        let c = |v: f64| (v * 255.0).round() as u8;
        [c(r), c(g), c(b), 255]
    })
    .expect("size");
    for (set, image, w_mm, want) in [
        (ColorSet::Cmyk, &ramp, 79.9, ["32", "13", "51", "20"]),
        (ColorSet::Rgbk, &ramp, 79.9, ["2", "14", "5", "20"]),
        (
            ColorSet::Cmyk,
            &shapes(false),
            84.7,
            ["32", "51", "13", "20"],
        ),
        (ColorSet::Rgbk, &shapes(false), 84.7, ["5", "2", "14", "20"]),
    ] {
        let p = PhotoStitch {
            color_set: set,
            ..PhotoStitch::new(PhotoStyle::Lines, PhotoMode::Color(photo::LINE_COLORS))
        };
        let out = photo::convert(image, &Frame::new(0.0, 0.0, w_mm, image.width()), &p)
            .expect("converts");
        let got: Vec<&str> = out
            .threads
            .iter()
            .map(|t| t.catalog_number.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(got, want, "{set:?} on a {} px image", image.width());
    }
}
