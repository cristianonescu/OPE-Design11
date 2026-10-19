mod common;

use common::*;
use openstitch_core::Thread;
use openstitch_design::{Compiler, Design, DesignPage, LineType, RegionType};
use openstitch_digitize::center::{self, FigureOptions, LineImageOptions, RegionSew, SewSettings};
use openstitch_digitize::{DigitizeError, Frame, Raster};

/// 60×60 px white with a 3-px black square outline (outer 10..50); `inside` fills the middle.
fn square(inside: [u8; 4]) -> Raster {
    rects(
        60,
        60,
        WHITE,
        &[(10, 10, 50, 50, BLACK), (13, 13, 47, 47, inside)],
    )
}

fn run(
    img: &Raster,
    settings: &SewSettings,
) -> (center::Figure, Vec<openstitch_design::DesignObject>) {
    let frame = Frame::new(0.0, 0.0, 60.0, 60); // 1 mm per pixel
    let li = center::line_image(img, &LineImageOptions::default());
    let fig = center::figure(img, &li, &frame, &FigureOptions::default()).expect("figure");
    let objects = center::sew(&fig, settings);
    (fig, objects)
}

#[test]
fn line_image_thresholds_dark_pixels() {
    let li = center::line_image(&square(WHITE), &LineImageOptions::default());
    assert_eq!((li.width, li.height), (60, 60));
    assert_eq!(li.line_pixels(), 40 * 40 - 34 * 34);
    let none = center::line_image(
        &square(WHITE),
        &LineImageOptions {
            threshold: 0.0,
            ..Default::default()
        },
    );
    assert_eq!(none.line_pixels(), 0);
}

#[test]
fn thinning_leaves_a_one_pixel_line() {
    // A 5-pixel-wide horizontal bar, 30 long.
    let mut mask = vec![false; 40 * 9];
    for y in 2..7 {
        for x in 5..35 {
            mask[y * 40 + x] = true;
        }
    }
    let s = center::thin(&mask, 40, 9);
    for x in 8..32 {
        assert_eq!((0..9).filter(|&y| s[y * 40 + x]).count(), 1, "column {x}");
    }
    let paths = center::skeleton_paths(&s, 40, 9);
    assert_eq!(paths.len(), 1);
    assert!(!paths[0].1);
}

#[test]
fn outline_becomes_one_closed_line_and_one_region() {
    let (fig, objects) = run(&square(WHITE), &SewSettings::default());
    assert_eq!(fig.lines.len(), 1);
    assert!(fig.lines[0].closed);
    assert_eq!(fig.regions.len(), 1);
    assert!(
        (fig.line_width_mm - 3.0).abs() <= 1.0,
        "{}",
        fig.line_width_mm
    );
    // The centre line of the 3-px ring lies 11.5 px from the image edge.
    for p in fig.lines[0].path() {
        assert!(
            p.x >= 100.0 && p.x <= 500.0 && p.y >= 100.0 && p.y <= 500.0,
            "{p:?}"
        );
    }
    // Auto leaves the white (paper) region unsewn: only the line object.
    assert_eq!(objects.len(), 1);
    assert_eq!(
        objects[0].line.as_ref().map(|l| l.kind),
        Some(LineType::Running)
    );
    assert!(objects[0].region.is_none());
}

#[test]
fn coloured_region_is_filled_in_its_colour() {
    // Light enough (luma > 128) not to count as line pixels.
    let (fig, objects) = run(
        &square(YELLOW),
        &SewSettings {
            line: Some(LineType::Zigzag),
            ..Default::default()
        },
    );
    assert_eq!(fig.regions.len(), 1);
    assert_eq!(objects.len(), 2, "region, then lines");
    let region = objects[0].region.as_ref().expect("region");
    assert_eq!(region.kind, RegionType::Fill);
    assert!(
        region.thread.red > 150 && region.thread.green > 120 && region.thread.blue < 100,
        "{:?}",
        region.thread
    );
    let line = objects[1].line.as_ref().expect("line");
    assert_eq!(line.kind, LineType::Zigzag);
    assert!(
        (line.width - 3.0).abs() <= 1.0,
        "zigzag width follows the stroke: {}",
        line.width
    );
    // Region outline sits inside the ring.
    assert!(all_inside(&objects[..1], 125.0, 125.0, 475.0, 475.0, 1.0));
    let mut d = Design::new(
        "t",
        DesignPage {
            width: 600.0,
            height: 600.0,
        },
    );
    for o in objects {
        d.add(o);
    }
    assert_eq!(Compiler::default().compile(&d).threads.len(), 2);
}

#[test]
fn sew_settings_choose_types_and_threads() {
    let settings = SewSettings {
        line: None,
        region: RegionSew::Satin,
        region_thread: Some(Thread::rgb(0x00ff00)),
        ..Default::default()
    };
    let (_, objects) = run(&square(WHITE), &settings);
    assert_eq!(objects.len(), 1);
    let region = objects[0].region.as_ref().expect("region");
    assert_eq!(region.kind, RegionType::Satin);
    assert_eq!(region.thread.hex(), "#00ff00");
    let (_, none) = run(
        &square(WHITE),
        &SewSettings {
            line: None,
            region: RegionSew::None,
            ..Default::default()
        },
    );
    assert!(none.is_empty());
}

#[test]
fn junctions_split_open_lines() {
    // A "T": horizontal bar and a vertical bar hanging from its middle; nothing enclosed.
    let img = rects(
        60,
        60,
        WHITE,
        &[(5, 10, 55, 13, BLACK), (29, 10, 32, 55, BLACK)],
    );
    let (fig, objects) = run(&img, &SewSettings::default());
    assert!(fig.regions.is_empty());
    assert!(fig.lines.len() >= 2, "{}", fig.lines.len());
    assert!(fig.lines.iter().all(|c| !c.closed));
    assert_eq!(objects.len(), 1);
    assert!(all_inside(&objects, 50.0, 100.0, 550.0, 550.0, 5.0));
}

#[test]
fn deterministic_and_hostile() {
    let (a, _) = run(&square(YELLOW), &SewSettings::default());
    let (b, _) = run(&square(YELLOW), &SewSettings::default());
    assert_eq!(a, b);
    let frame = Frame::new(0.0, 0.0, 60.0, 60);
    let blank = rects(60, 60, WHITE, &[]);
    let li = center::line_image(&blank, &LineImageOptions::default());
    assert!(matches!(
        center::figure(&blank, &li, &frame, &FigureOptions::default()),
        Err(DigitizeError::Empty(_))
    ));
    let dot = rects(1, 1, BLACK, &[]);
    let li = center::line_image(&dot, &LineImageOptions::default());
    assert!(
        center::figure(
            &dot,
            &li,
            &Frame::new(0.0, 0.0, 1.0, 1),
            &FigureOptions::default()
        )
        .is_err()
    );
    // Noise: bounded result or a clean refusal.
    let noise = Raster::from_fn(500, 500, |x, y| {
        let v = ((x * 7919 + y * 104_729).wrapping_mul(2_654_435_761) >> 24) as u8;
        [v, v, v, 255]
    })
    .expect("size");
    let li = center::line_image(
        &noise,
        &LineImageOptions {
            resolution: 500,
            ..Default::default()
        },
    );
    match center::figure(
        &noise,
        &li,
        &Frame::new(0.0, 0.0, 100.0, 500),
        &FigureOptions {
            min_line_mm: 0.0,
            min_region_mm2: 0.0,
            ..Default::default()
        },
    ) {
        Ok(f) => assert!(f.lines.len() + f.regions.len() <= center::MAX_FIGURE_PARTS + 1),
        Err(e) => assert!(matches!(e, DigitizeError::TooLarge(_)), "{e}"),
    }
}
