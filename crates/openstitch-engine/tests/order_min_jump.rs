//! Optimize Sewing Order in its two modes (issue #137). `peDesign` reproduces PE-DESIGN 11.50
//! (references `optimize-entry-exit`, `machine-rbr`, `machine-rbr-far`, `machine-rbgb`; audit
//! item 16): colours grouped in order of first use, drawing order within a colour, entry and
//! exit points linked between same-colour neighbours only. `minJump` is OpenStitch's own
//! option: the same colour blocks, each arranged for the shortest total travel between
//! outlines (greedy nearest-free, then 2-opt), every neighbouring pair linked.

use openstitch_core::{Command, Stitch};
use openstitch_design::{Compiler, OptimizeOrder};
use openstitch_engine::Session;
use openstitch_engine::order::{min_jump, optimized_with, total_travel};
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn order(s: &Session) -> Vec<u64> {
    s.design().objects.iter().map(|o| o.id).collect()
}

/// Needle travel across every jump or trim, mm, in sewing order.
fn jumps(s: &Session) -> Vec<f64> {
    let stitches = Compiler::default().compile(s.design()).stitches;
    let mut out = Vec::new();
    let (mut last, mut jumped): (Option<&Stitch>, bool) = (None, false);
    for st in &stitches {
        match st.command {
            Command::Stitch => {
                if let (Some(l), true) = (last, jumped) {
                    out.push(((st.x - l.x).hypot(st.y - l.y)) / 10.0);
                }
                (last, jumped) = (Some(st), false);
            }
            Command::Jump | Command::Trim => jumped = true,
            _ => {}
        }
    }
    out
}

/// 10 mm squares without under sewing (so the first needle point is the entry point), one
/// per `(x, y, colour)`, drawn in that order (ids 1, 2, …).
fn squares(s: &mut Session, at: &[(f64, f64, &str)]) {
    for &(x, y, c) in at {
        run(
            s,
            "shape.rectangle",
            json!({"x": x, "y": y, "width": 10, "height": 10, "fill": c}),
        );
    }
    let ids: Vec<u64> = (1..=at.len() as u64).collect();
    run(
        s,
        "object.setRegion",
        json!({"ids": ids, "underlay": false}),
    );
}

/// A bad drawing order: five red squares along a line drawn 0, 60, 20, 80, 40 mm.
const ZIGZAG: [(f64, f64, &str); 5] = [
    (0.0, 0.0, "Red"),
    (60.0, 0.0, "Red"),
    (20.0, 0.0, "Red"),
    (80.0, 0.0, "Red"),
    (40.0, 0.0, "Red"),
];

#[test]
fn pe_design_mode_is_the_default_and_keeps_drawing_order() {
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s, &ZIGZAG);
    let r = run(&mut s, "order.optimize", json!({}));
    assert_eq!(r["mode"], "peDesign");
    assert_eq!(
        order(&s),
        [1, 2, 3, 4, 5],
        "PE-DESIGN keeps the drawing order"
    );
    // Linked at the facing sides: the jumps are the gaps between the outlines.
    let j = jumps(&s);
    assert_eq!(j.len(), 4, "{j:?}");
    for (d, gap) in j.iter().zip([50.0, 30.0, 50.0, 30.0]) {
        assert!(
            (d - gap).abs() <= 0.5,
            "jump {d} mm, expected {gap} ({j:?})"
        );
    }
}

#[test]
fn min_jump_walks_the_line_and_is_never_worse_than_greedy() {
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s, &ZIGZAG);
    let pe = total_travel(
        s.design(),
        &optimized_with(s.design(), OptimizeOrder::PeDesign),
    );
    let greedy = total_travel(s.design(), &min_jump(s.design(), false));
    let two_opt = total_travel(s.design(), &min_jump(s.design(), true));
    assert!(
        two_opt <= greedy + 1e-9 && greedy < pe,
        "{two_opt} {greedy} {pe}"
    );
    let r = run(&mut s, "order.optimize", json!({"mode": "minJump"}));
    assert_eq!(r["mode"], "minJump");
    assert_eq!(order(&s), [1, 3, 5, 2, 4], "left to right along the line");
    let j = jumps(&s);
    assert_eq!(j.len(), 4, "{j:?}");
    for d in &j {
        assert!(
            (d - 10.0).abs() <= 0.5,
            "every jump is the 10 mm gap: {j:?}"
        );
    }
    // The design is unchanged otherwise: five squares, one colour.
    let p = Compiler::default().compile(s.design());
    assert_eq!(p.count(Command::ColorChange), 0);
}

/// Deterministic pseudo-random layouts (SplitMix64).
fn layouts(seed: u64, n: usize) -> Vec<(f64, f64, &'static str)> {
    let mut x = seed;
    let mut next = move || {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) as f64 / u64::MAX as f64
    };
    (0..n)
        .map(|_| {
            let colour = if next() < 0.6 { "Red" } else { "Blue" };
            ((next() * 180.0).round(), (next() * 180.0).round(), colour)
        })
        .collect()
}

#[test]
fn two_opt_never_loses_to_greedy_and_keeps_colour_blocks() {
    for (seed, n) in [
        (1u64, 3usize),
        (2, 4),
        (3, 6),
        (4, 8),
        (5, 10),
        (6, 12),
        (7, 16),
    ] {
        let mut s = Session::new(std::env::temp_dir());
        let at = layouts(seed, n);
        squares(&mut s, &at);
        let d = s.design();
        let pe = optimized_with(d, OptimizeOrder::PeDesign);
        let greedy = min_jump(d, false);
        let best = min_jump(d, true);
        let (tp, tg, tb) = (
            total_travel(d, &pe),
            total_travel(d, &greedy),
            total_travel(d, &best),
        );
        assert!(
            tb <= tg + 1e-9,
            "seed {seed}: 2-opt {tb} worse than greedy {tg}"
        );
        assert!(
            tb <= tp + 1e-9,
            "seed {seed}: 2-opt {tb} worse than PE-DESIGN {tp}"
        );
        // Same colour blocks as PE-DESIGN: the sequence of colours is identical.
        let colours = |o: &[usize]| -> Vec<String> {
            o.iter()
                .map(|&i| d.objects[i].region.as_ref().expect("region").thread.hex())
                .collect()
        };
        assert_eq!(colours(&best), colours(&pe), "seed {seed}");
        let mut sorted = best.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            (0..n).collect::<Vec<_>>(),
            "seed {seed}: a permutation"
        );
    }
}

#[test]
fn min_jump_keeps_overlapping_objects_in_drawing_order() {
    let mut s = Session::new(std::env::temp_dir());
    // Three red squares far apart, drawn right to left, plus a red square drawn last that
    // overlaps the first (leftmost in the walk) one: it must stay after it, however the
    // walk would like to start there.
    squares(
        &mut s,
        &[
            (80.0, 0.0, "Red"),
            (40.0, 0.0, "Red"),
            (0.0, 0.0, "Red"),
            (5.0, 5.0, "Red"),
        ],
    );
    for _ in 0..2 {
        run(&mut s, "order.optimize", json!({"mode": "minJump"}));
        let o = order(&s);
        let at = |id| o.iter().position(|&x| x == id).expect("kept");
        assert!(at(4) > at(3), "the overlapping square stays on top: {o:?}");
        // The two overlapping squares are sewn together (travel 0 between them).
        assert_eq!(at(4), at(3) + 1, "{o:?}");
    }
}

#[test]
fn min_jump_links_across_colour_changes_pe_design_does_not() {
    let mut s = Session::new(std::env::temp_dir());
    squares(&mut s, &[(0.0, 0.0, "Red"), (50.0, 0.0, "Blue")]);
    run(&mut s, "order.optimize", json!({}));
    let r = |s: &Session, i: usize| s.design().objects[i].region.clone().expect("region");
    assert_eq!(
        r(&s, 0).exit,
        Some(openstitch_design::v2(50.0, 100.0)),
        "PE-DESIGN leaves the default exit before a colour change"
    );
    run(&mut s, "order.optimize", json!({"mode": "minJump"}));
    assert_eq!(
        r(&s, 0).exit.map(|p| p.x),
        Some(100.0),
        "linked to the facing side"
    );
    assert_eq!(r(&s, 1).entry.map(|p| p.x), Some(500.0));
}

#[test]
fn the_design_setting_is_saved_used_by_default_and_validated() {
    let dir = std::env::temp_dir().join(format!("os-min-jump-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let mut s = Session::new(&dir);
    squares(&mut s, &ZIGZAG);
    assert_eq!(
        run(&mut s, "design.info", json!({}))["optimizeOrder"],
        "peDesign"
    );
    let r = run(
        &mut s,
        "design.setProperties",
        json!({"optimizeOrder": "minJump"}),
    );
    assert_eq!(r["optimizeOrder"], "minJump");
    assert_eq!(s.design().optimize_order, OptimizeOrder::MinJump);
    run(&mut s, "project.save", json!({"path": "min-jump.osd"}));
    let text = std::fs::read_to_string(dir.join("min-jump.osd")).expect("saved");
    assert!(text.contains("\"optimizeOrder\": \"minJump\""), "{text}");
    run(&mut s, "design.new", json!({}));
    run(&mut s, "project.open", json!({"path": "min-jump.osd"}));
    assert_eq!(s.design().optimize_order, OptimizeOrder::MinJump);
    // Optimise without a mode follows the setting.
    let r = run(&mut s, "order.optimize", json!({}));
    assert_eq!(r["mode"], "minJump");
    assert_eq!(order(&s), [1, 3, 5, 2, 4]);
    // Undo restores the drawing order; the setting itself undoes too.
    run(&mut s, "history.undo", json!({}));
    assert_eq!(order(&s), [1, 2, 3, 4, 5]);
    // A design without the field stays at PE-DESIGN's mode.
    run(&mut s, "design.new", json!({}));
    assert_eq!(s.design().optimize_order, OptimizeOrder::PeDesign);
    assert!(
        s.execute(
            "design.setProperties",
            Some(&json!({"optimizeOrder": "shortest"}))
        )
        .is_err()
    );
    assert!(
        s.execute("order.optimize", Some(&json!({"mode": "nearest"})))
            .is_err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
