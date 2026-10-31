//! Cross-platform golden hashes of OpenStitch-mode output (B16.1).
//!
//! The image tools must give the same output on macOS, Windows and Linux (all their maths goes
//! through `openstitch_design::fmath::Det`; see the crate's source scan). This test converts
//! three small images with each tool's OpenStitch mode and pins a hash of the result. CI runs
//! it on all three systems, so:
//!
//! - it fails on **one** system only: the output is not deterministic there (a platform
//!   function, an unseeded random choice, iteration over a hash map…). Fix the cause, never
//!   the hash;
//! - it fails on **every** system with the same new hashes: an intended change to OpenStitch
//!   mode. Paste the printed table into `EXPECTED` in the same PR.
//!
//! Only PNG inputs and a synthetic gradient are used: JPEG decoders may take different SIMD
//! paths per CPU, which would test the decoder instead of the tools.

use openstitch_digitize::autopunch::{self, AutoPunch};
use openstitch_digitize::photo::{self, PhotoMode, PhotoStitch, PhotoStyle};
use openstitch_digitize::{Frame, Mode, Raster};
use std::path::Path;

/// FNV-1a, 64 bit: a dependency-free, platform-independent hash of the serialised output.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn example(name: &str) -> Raster {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/images")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Raster::decode(&bytes).expect("decodes").fit(96)
}

/// A smooth two-axis colour gradient, built in code (no decoder involved).
fn gradient() -> Raster {
    Raster::from_fn(96, 64, |x, y| {
        let (u, v) = (x as f64 / 95.0, y as f64 / 63.0);
        [
            (40.0 + 200.0 * u) as u8,
            (220.0 - 160.0 * v) as u8,
            (60.0 + 120.0 * u * v) as u8,
            255,
        ]
    })
    .expect("valid size")
}

fn inputs() -> Vec<(&'static str, Raster)> {
    vec![
        ("emblem", example("d02-emblem-small.png")),
        ("leaf", example("g10-leaf-transparent.png")),
        ("gradient", gradient()),
    ]
}

/// `(input, tool, hash)` for every input and OpenStitch-mode tool.
fn actual() -> Vec<(String, &'static str, u64)> {
    let mut out = Vec::new();
    for (name, img) in inputs() {
        let frame = Frame::new(0.0, 0.0, 30.0, img.width());

        let ap = AutoPunch {
            mode: Mode::OpenStitch,
            ..AutoPunch::default()
        };
        let r = autopunch::convert(&img, &frame, &ap).expect("auto punch converts");
        let json = serde_json::to_vec(&(&r.objects, &r.threads)).expect("serialise");
        out.push((name.to_string(), "auto-punch", fnv1a(&json)));

        for (tool, style) in [
            ("photo-stitch-1", PhotoStyle::Fill),
            ("photo-stitch-2", PhotoStyle::Lines),
        ] {
            let p = PhotoStitch {
                compat: Mode::OpenStitch,
                ..PhotoStitch::new(style, PhotoMode::Color(4))
            };
            let r = photo::convert(&img, &frame, &p).expect("photo stitch converts");
            let json = serde_json::to_vec(&(&r.objects, &r.threads)).expect("serialise");
            out.push((name.to_string(), tool, fnv1a(&json)));
        }
    }
    out
}

/// Pinned on macOS (aarch64); must match on Windows and Linux (x86_64).
const EXPECTED: &[(&str, &str, u64)] = &[
    ("emblem", "auto-punch", 0x9713bc188ca668df),
    ("emblem", "photo-stitch-1", 0x0531d39ae8eda862),
    ("emblem", "photo-stitch-2", 0x844c10dd1dfd47bf),
    ("leaf", "auto-punch", 0x5094818779b1647c),
    ("leaf", "photo-stitch-1", 0x08bb8cc16974e338),
    ("leaf", "photo-stitch-2", 0x9aed90ef3f219c8f),
    ("gradient", "auto-punch", 0x984d94f23b7b030e),
    ("gradient", "photo-stitch-1", 0x0ed118d1455a407e),
    ("gradient", "photo-stitch-2", 0x0985faaa6fbfb4fc),
];

#[test]
fn openstitch_mode_output_is_identical_on_every_platform() {
    let got = actual();
    let table: String = got
        .iter()
        .map(|(i, t, h)| format!("    (\"{i}\", \"{t}\", 0x{h:016x}),\n"))
        .collect();
    let same = got.len() == EXPECTED.len()
        && got
            .iter()
            .zip(EXPECTED)
            .all(|((i, t, h), (ei, et, eh))| i == ei && t == et && h == eh);
    assert!(
        same,
        "OpenStitch-mode output differs from the pinned hashes on {}-{}.\n\
         If this happens on one system only, the output is not deterministic there.\n\
         If it is an intended change on every system, set EXPECTED to:\n{table}",
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
}
