//! The semantic comparer on synthetic patterns that differ in one known way, at the edges of
//! each tolerance, and on the pyembroidery oracle fixtures.

use openstitch_compat::compare::{
    CompareOptions, ThreadFields, compare_files, compare_patterns, first_byte_difference,
};
use openstitch_compat::tolerance::Tolerance;
use openstitch_compat::trace::Anchor;
use openstitch_core::{Command, StitchPattern, Thread};
use openstitch_formats::FileType;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../openstitch-formats/tests/fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Two colour blocks: a 20-stitch zigzag, a trim and jump, then a 10-stitch column.
fn pattern() -> StitchPattern {
    let mut p = StitchPattern::new();
    p.threads = vec![Thread::rgb(0xe0_10_10), Thread::rgb(0x10_40_c0)];
    p.set_name("synthetic");
    for i in 0..=20 {
        p.stitch_to(f64::from(i) * 10.0, if i % 2 == 0 { 0.0 } else { 100.0 });
    }
    p.push(Command::Trim);
    p.jump_to(260.0, -50.0);
    p.stitch_to(260.0, -50.0);
    p.push(Command::ColorChange);
    for i in 0..10 {
        p.stitch_to(
            300.0 + if i % 2 == 1 { 40.0 } else { 0.0 },
            -50.0 + f64::from(i) * 25.0,
        );
    }
    p.push(Command::End);
    p
}

fn exact() -> CompareOptions {
    CompareOptions {
        origin: None,
        ..CompareOptions::default()
    }
}

fn whats(c: &openstitch_compat::Comparison) -> Vec<&str> {
    c.differences.iter().map(|d| d.what.as_str()).collect()
}

#[test]
fn identical_patterns_match() {
    let c = compare_patterns(&pattern(), &pattern(), &exact());
    assert!(c.is_match(), "{:?}", c.differences);
    assert_eq!(c.expected.block_stitches, vec![22, 10]);
    assert_eq!(c.expected, c.actual);
}

#[test]
fn origins_are_normalised() {
    let mut moved = pattern();
    for s in &mut moved.stitches {
        s.x += 1234.0;
        s.y -= 77.0;
    }
    // Not moved: only the bounds and every needle position differ.
    let c = compare_patterns(&pattern(), &moved, &exact());
    assert!(!c.is_match());
    assert!(whats(&c).contains(&"bounds.minX"));
    assert!(
        whats(&c)
            .iter()
            .all(|w| w.starts_with("bounds.") || *w == "entry.position")
    );
    for anchor in [Anchor::BoundsCenter, Anchor::BoundsMin, Anchor::FirstStitch] {
        let options = CompareOptions {
            origin: Some(anchor),
            ..CompareOptions::default()
        };
        let c = compare_patterns(&pattern(), &moved, &options);
        assert!(c.is_match(), "{anchor:?}: {:?}", c.differences);
        let [dx, dy] = c.origin_shift_mm;
        assert!(
            (dx + 123.4).abs() < 1e-9 && (dy - 7.7).abs() < 1e-9,
            "{dx}, {dy}"
        );
    }
}

#[test]
fn a_moved_needle_point_is_reported_where_it_is() {
    let mut moved = pattern();
    // Entry 5 of block 0 moves 0.3 mm (3 units) along x.
    moved.stitches[5].x += 3.0;
    let at = |position_mm: f64| {
        let o = CompareOptions {
            position_mm,
            bounds_mm: 1.0,
            ..exact()
        };
        compare_patterns(&pattern(), &moved, &o)
    };
    let c = at(0.2);
    assert_eq!(whats(&c), ["entry.position"]);
    let d = &c.differences[0];
    assert_eq!(
        (d.block, d.expected_index, d.actual_index),
        (Some(0), Some(5), Some(5))
    );
    assert!((d.delta.expect("delta") - 0.3).abs() < 1e-9);
    // The tolerance edge is inclusive.
    assert!(at(0.3).is_match());
    assert_eq!(at(0.299).total(), 1);
}

#[test]
fn an_inserted_stitch_resynchronises() {
    let mut extra = pattern();
    extra
        .stitches
        .insert(8, openstitch_core::Stitch::new(75.0, 50.0, Command::Stitch));
    let c = compare_patterns(&pattern(), &extra, &exact());
    let w = whats(&c);
    // One extra entry, and the counts it changes; no cascade of position differences.
    assert_eq!(
        w,
        ["total.stitches", "block.stitches", "entry.extra"],
        "{:?}",
        c.differences
    );
    assert_eq!(c.differences[2].actual_index, Some(8));
    // Allowing one stitch more per count leaves only the entry.
    let o = CompareOptions {
        counts: Tolerance::count(1.0),
        ..exact()
    };
    assert_eq!(
        whats(&compare_patterns(&pattern(), &extra, &o)),
        ["entry.extra"]
    );

    // A missing stitch, seen from the other side.
    let c = compare_patterns(&extra, &pattern(), &o);
    assert_eq!(whats(&c), ["entry.missing"]);
    assert_eq!(c.differences[0].expected_index, Some(8));
}

#[test]
fn command_types_jumps_trims_and_blocks_are_compared() {
    let mut p = pattern();
    p.stitches[3].command = Command::Jump;
    let c = compare_patterns(&pattern(), &p, &exact());
    assert!(whats(&c).contains(&"entry.command"), "{:?}", c.differences);
    assert!(whats(&c).contains(&"total.jumps"));

    // No trim: the trim count and the entry differ.
    let mut p = pattern();
    p.stitches.retain(|s| s.command != Command::Trim);
    let c = compare_patterns(&pattern(), &p, &exact());
    assert!(whats(&c).contains(&"total.trims"));
    assert!(whats(&c).contains(&"entry.missing"));

    // One colour block fewer.
    let mut p = pattern();
    p.stitches.retain(|s| s.command != Command::ColorChange);
    p.threads.truncate(1);
    let c = compare_patterns(&pattern(), &p, &exact());
    for w in ["total.colorChanges", "blocks", "threads.count"] {
        assert!(whats(&c).contains(&w), "{w}: {:?}", c.differences);
    }
}

#[test]
fn split_jumps_collapse_unless_asked_not_to() {
    // DST splits a long move into several jumps.
    let mut p = pattern();
    let at = p
        .stitches
        .iter()
        .position(|s| s.command == Command::Jump)
        .expect("jump");
    p.stitches
        .insert(at, openstitch_core::Stitch::new(230.0, 20.0, Command::Jump));
    let c = compare_patterns(&pattern(), &p, &exact());
    assert_eq!(whats(&c), ["total.jumps"]);
    let o = CompareOptions {
        collapse_jumps: false,
        ..exact()
    };
    let c = compare_patterns(&pattern(), &p, &o);
    assert_eq!(whats(&c), ["total.jumps", "entry.extra"]);
}

#[test]
fn count_tolerance_in_percent_has_inclusive_edges() {
    let block = |n: usize| {
        let mut p = StitchPattern::new();
        for i in 0..n {
            p.stitch_to(i as f64 * 10.0, 0.0);
        }
        p
    };
    let o = CompareOptions {
        counts: Tolerance::percent(2.0),
        sequence: false,
        bounds_mm: 1e9,
        ..exact()
    };
    assert!(compare_patterns(&block(100), &block(102), &o).is_match());
    let c = compare_patterns(&block(100), &block(103), &o);
    assert_eq!(whats(&c), ["total.stitches", "block.stitches"]);
    assert_eq!(c.differences[1].block, Some(0));
}

#[test]
fn threads_and_metadata_are_compared_field_by_field() {
    let mut p = pattern();
    p.threads[1].chart = Some("Chart X".into());
    p.threads[1].name = Some("Blue".into());
    p.threads[0] = Thread::rgb(0xe0_10_11);
    p.metadata.insert("author".into(), "Ana".into());
    let c = compare_patterns(&pattern(), &p, &exact());
    let w = whats(&c);
    assert_eq!(
        w,
        [
            "thread.rgb",
            "thread.name",
            "thread.chart",
            "metadata.author"
        ],
        "{:?}",
        c.differences
    );
    assert_eq!(c.differences[0].block, Some(0));
    assert_eq!(c.differences[1].block, Some(1));
    // Fields can be left out.
    let o = CompareOptions {
        threads: ThreadFields {
            name: false,
            chart: false,
            ..ThreadFields::default()
        },
        metadata: false,
        ..exact()
    };
    assert_eq!(whats(&compare_patterns(&pattern(), &p, &o)), ["thread.rgb"]);
    let o = CompareOptions {
        threads: ThreadFields::NONE,
        metadata: false,
        ..exact()
    };
    assert!(compare_patterns(&pattern(), &p, &o).is_match());
}

#[test]
fn the_difference_list_is_capped_and_the_rest_counted() {
    let mut moved = pattern();
    for s in &mut moved.stitches {
        s.x += 1.0;
    }
    let o = CompareOptions {
        max_differences: 3,
        bounds_mm: 1.0,
        ..exact()
    };
    let c = compare_patterns(&pattern(), &moved, &o);
    assert_eq!(c.differences.len(), 3);
    assert!(c.unlisted > 0);
    assert_eq!(c.total(), 3 + c.unlisted);
}

#[test]
fn oracle_files_compare_across_formats() {
    // The same PEC stitch data inside a PES and a bare PEC file.
    let (pes, pec) = (fixture("oracle.pes"), fixture("oracle.pec"));
    let o = CompareOptions {
        metadata: false,
        ..CompareOptions::default()
    };
    let c = compare_files((&pes, FileType::Pes), (&pec, FileType::Pec), &o).expect("compare");
    assert!(c.is_match(), "{:?}", c.differences);

    // DST against PES, origins normalised and no thread table: the stitches agree; the only
    // difference is one trim + jump at the start of the second colour block that the PEC data
    // has and the DST file does not.
    let dst = fixture("oracle.dst");
    let o = CompareOptions {
        threads: ThreadFields::NONE,
        metadata: false,
        ..CompareOptions::default()
    };
    let c = compare_files((&pes, FileType::Pes), (&dst, FileType::Dst), &o).expect("compare");
    assert_eq!(
        whats(&c),
        [
            "total.jumps",
            "total.trims",
            "entry.missing",
            "entry.missing"
        ],
        "{:?}",
        c.differences
    );
    for (d, command) in c.differences[2..].iter().zip(["trim", "jump"]) {
        assert_eq!(d.block, Some(1));
        assert_eq!(d.expected["command"], command);
    }
}

#[test]
fn pes_object_sections_and_bytes() {
    let (v1, v6) = (fixture("oracle_v1.pes"), fixture("oracle_v6.pes"));
    let o = CompareOptions {
        threads: ThreadFields::NONE,
        metadata: false,
        ..CompareOptions::default()
    };
    let c = compare_files((&v6, FileType::Pes), (&v1, FileType::Pes), &o).expect("compare");
    let w = whats(&c);
    assert!(w.contains(&"pes.version"), "{w:?}");
    assert!(w.contains(&"pes.threads"), "{w:?}");
    assert!(
        !w.iter().any(|w| w.starts_with("entry.")),
        "same stitches: {w:?}"
    );

    // Byte identity only when asked.
    let same = compare_files(
        (&v6, FileType::Pes),
        (&v6, FileType::Pes),
        &CompareOptions {
            bytes: true,
            ..CompareOptions::default()
        },
    )
    .expect("compare");
    assert!(same.is_match());
    let mut changed = v6.clone();
    let last = changed.len() - 1;
    changed[last] ^= 0xff;
    let d = first_byte_difference(&v6, &changed).expect("differs");
    assert_eq!(d.expected_index, Some(last));
    let d = first_byte_difference(&v6, &v6[..100]).expect("shorter");
    assert_eq!(d.expected_index, Some(100));
    assert!(first_byte_difference(&v6, &v6).is_none());

    // A file that is not what its type says is an error, not a match.
    assert!(compare_files((&v6, FileType::Pes), (b"junk", FileType::Pes), &o).is_err());
}

/// `dropRepeats` leaves zero-length stitches out on both sides and says how many per side;
/// without it they are a count difference. A repeat after a jump (the landing) counts too.
#[test]
fn drop_repeats_counts_what_it_leaves_out() {
    use openstitch_compat::compare::DroppedRepeats;
    let base = pattern();
    // Two repeats in the expected design: one after the jump (a landing), one mid-run.
    let mut expected = base.clone();
    let landing = expected
        .stitches
        .iter()
        .position(|s| s.command == Command::Jump)
        .expect("jump");
    let j = expected.stitches[landing];
    expected.stitches.insert(
        landing + 1,
        openstitch_core::Stitch::new(j.x, j.y, Command::Stitch),
    );
    let s5 = expected.stitches[5];
    expected
        .stitches
        .insert(6, openstitch_core::Stitch::new(s5.x, s5.y, Command::Stitch));
    let plain = compare_patterns(&expected, &base, &exact());
    assert!(
        whats(&plain).contains(&"total.stitches"),
        "{:?}",
        whats(&plain)
    );
    assert_eq!(plain.dropped_repeats, None);
    let dropping = CompareOptions {
        drop_repeats: true,
        ..exact()
    };
    let c = compare_patterns(&expected, &base, &dropping);
    assert!(c.is_match(), "{:?}", whats(&c));
    // `base` already has a zero-length stitch on its own landing (jump then a stitch there).
    assert_eq!(
        c.dropped_repeats,
        Some(DroppedRepeats {
            expected: 3,
            actual: 1
        })
    );
}
