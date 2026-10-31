//! OpenStitch mode's perceptual colour (docs/research/beyond/image-digitizing.md).

use openstitch_digitize::autopunch::{self, AutoPunch};
use openstitch_digitize::palette::{self, Palette};
use openstitch_digitize::perceptual::{de2000, kmeans, lab, nearest, rgb};
use openstitch_digitize::{Frame, Mode, Raster};

/// CIEDE2000 against the published test data (Sharma, Wu and Dalal 2005, pairs 1, 7 and 25).
#[test]
fn de2000_matches_the_published_test_pairs() {
    let pairs = [
        ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
        ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
        (
            [60.2574, -34.0099, 36.2677],
            [60.4626, -34.1751, 39.4387],
            1.2644,
        ),
    ];
    for (p, q, want) in pairs {
        let d = de2000(p, q);
        assert!((d - want).abs() < 1e-3, "{p:?} {q:?}: {d} != {want}");
        assert!((de2000(q, p) - d).abs() < 1e-9, "symmetric");
    }
    assert_eq!(de2000([40.0, 10.0, -5.0], [40.0, 10.0, -5.0]), 0.0);
}

#[test]
fn lab_round_trips_and_has_the_white_point() {
    let w = lab([255, 255, 255]);
    assert!((w[0] - 100.0).abs() < 0.01 && w[1].abs() < 0.05 && w[2].abs() < 0.05);
    for c in [[0, 0, 0], [253, 120, 11], [20, 40, 200], [128, 128, 128]] {
        assert_eq!(rgb(lab(c)), c);
    }
}

#[test]
fn kmeans_moves_centres_to_their_weighted_members() {
    let pts = [
        ([10.0, 0.0, 0.0], 1.0),
        ([12.0, 0.0, 0.0], 3.0),
        ([80.0, 0.0, 0.0], 2.0),
    ];
    let c = kmeans(&pts, &[[0.0, 0.0, 0.0], [100.0, 0.0, 0.0]], 10);
    assert!((c[0][0] - 11.5).abs() < 1e-9);
    assert!((c[1][0] - 80.0).abs() < 1e-9);
    assert_eq!(nearest(&c, [30.0, 0.0, 0.0]), 0);
}

fn flat(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Raster {
    Raster::from_fn(w, h, |x, y| {
        let [r, g, b] = f(x, y);
        [r, g, b, 255]
    })
    .expect("size")
}

/// A dark-green patch next to a dark grey one: RGB distance sends both to grey threads in PE
/// mode's matching, CIEDE2000 keeps the green a green thread.
#[test]
fn perceptual_palette_keeps_hues_apart() {
    let img = flat(
        40,
        20,
        |x, _| if x < 20 { [40, 90, 50] } else { [70, 70, 70] },
    );
    let census = palette::census(&img);
    let pal = Palette::from_census_perceptual(&census, 2);
    assert_eq!(
        pal.threads.len(),
        2,
        "two distinct colours keep two threads"
    );
    let green = &pal.threads[pal.classify([40, 90, 50])];
    let v = green.rgb_value();
    let (r, g, b) = ((v >> 16) & 255, (v >> 8) & 255, v & 255);
    assert!(
        g > r && g > b,
        "the green patch is sewn in a green thread, got #{v:06x}"
    );
    assert_eq!(
        pal,
        Palette::from_census_perceptual(&census, 2),
        "deterministic"
    );
}

/// OpenStitch mode merges regions under 3 mm²: specks that PE mode keeps as their own objects
/// (each sewn with a jump and trim) join the region around them.
#[test]
fn open_stitch_mode_merges_specks() {
    // 100 × 100 px at 0.5 mm/px: a red field with 2 × 2 px (1 mm²) and 3 × 3 px (2.25 mm²)
    // blue specks, on a white border.
    let img = flat(100, 100, |x, y| {
        let border = x < 5 || y < 5 || x >= 95 || y >= 95;
        let speck =
            (x % 20 < 3 && y % 20 < 3 && x > 10 && y > 10) || (x % 20 == 15 && y % 20 == 15);
        if border {
            [255, 255, 255]
        } else if speck {
            [20, 40, 200]
        } else {
            [220, 20, 30]
        }
    });
    let frame = Frame::new(0.0, 0.0, 50.0, 100);
    let pe = autopunch::convert(&img, &frame, &AutoPunch::default()).expect("pe");
    let os = autopunch::convert(
        &img,
        &frame,
        &AutoPunch {
            mode: Mode::OpenStitch,
            min_area_mm2: 3.0,
            ..AutoPunch::default()
        },
    )
    .expect("openstitch");
    assert!(pe.objects.len() > 1, "PE mode keeps the specks");
    assert_eq!(os.objects.len(), 1, "OpenStitch mode sews the field alone");
}

/// Line art keeps its lines in OpenStitch mode (the round-5 mandala, g11): short, thin pieces
/// of a dark line with light-grey edges, on white, are not merged as specks, and their light
/// edges are not left out with the white background that shares their thread.
#[test]
fn open_stitch_mode_keeps_thin_lines() {
    // 200 × 200 px at 0.25 mm/px: dashes 1 px (0.25 mm) wide and 12 px (3 mm) long, each with
    // a light-grey shade on both sides, on white.
    let img = flat(200, 200, |x, y| {
        let dash = (x / 16) % 2 == 0 && x > 8 && x < 192;
        match y % 40 {
            20 if dash => [30, 30, 30],
            19 | 21 if dash => [215, 215, 215],
            _ => [255, 255, 255],
        }
    });
    let frame = Frame::new(0.0, 0.0, 50.0, 200);
    let os = autopunch::convert(
        &img,
        &frame,
        &AutoPunch {
            mode: Mode::OpenStitch,
            min_area_mm2: 3.0,
            ..AutoPunch::default()
        },
    )
    .expect("the dashes are sewn");
    // Five lines of six dashes each.
    assert_eq!(os.objects.len(), 30, "every dash of every line stays");
}
