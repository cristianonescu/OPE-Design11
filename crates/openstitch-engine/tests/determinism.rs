//! The same design sews the same stitches, to the bit, on every platform. Stitch generation
//! uses only IEEE-exact arithmetic and the `libm` crate's transcendental functions
//! (`openstitch_design::fmath`), never the platform's C math library, whose last bits differ
//! between glibc, the MSVC runtime and Apple's libm. Before that change an arched text
//! measured 25.8 mm high on Linux and 26.0 mm on Windows and macOS (compat spec
//! pes-0110-editable-text-transform, 2026-10-29).

use openstitch_design::Compiler;
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// FNV-1a over the bits of every needle point's coordinates and command.
fn fingerprint(s: &Session) -> (usize, u64) {
    let p = Compiler::default().compile(s.design());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for st in &p.stitches {
        eat(st.x.to_bits());
        eat(st.y.to_bits());
        eat(st.command as u64);
    }
    (p.stitches.len(), h)
}

fn session() -> Session {
    let mut s = Session::new(std::env::temp_dir());
    run(&mut s, "design.new", json!({"width": 100, "height": 100}));
    s
}

#[test]
fn arched_text_sews_the_same_bits_on_every_platform() {
    // The design of compat spec pes-0110-editable-text-transform: HHHHHHH in Lato Bold, 10 mm,
    // over PE-DESIGN's first arch, filled without under sewing at density 5.
    let mut s = session();
    run(
        &mut s,
        "text.add",
        json!({"text": "HHHHHHH", "font": "lato-bold", "size": 10, "x": 50, "y": 50,
               "fill": "Red", "envelope": 1}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "fill", "underlay": false, "density": 5}),
    );
    assert_eq!(fingerprint(&s), EXPECTED_ARCH);
}

#[test]
fn rotated_shapes_and_curves_sew_the_same_bits_on_every_platform() {
    // Rotation (sin, cos), curve sampling, distances (hypot) and a satin at an angle.
    let mut s = session();
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 40, "cy": 40, "rx": 17.3, "ry": 9.1}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "angle": 37, "density": 4.5}),
    );
    run(&mut s, "object.rotate", json!({"ids": [1], "degrees": 23}));
    assert_eq!(fingerprint(&s), EXPECTED_ROTATED);
}

// Recorded on Windows (MSVC) with the libm-backed functions; Linux and macOS must match.
const EXPECTED_ARCH: (usize, u64) = (1731, 4783615289628419459);
const EXPECTED_ROTATED: (usize, u64) = (353, 5700264818730648043);
