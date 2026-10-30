//! Semantic comparison of two embroidery designs: the expected one (a reference or oracle file)
//! and the actual one (OpenStitch's output).
//!
//! Compared, each with its own tolerance: needle positions and command types entry by entry
//! within each colour block (so stitch order counts), colour blocks, stitch counts per block,
//! totals of stitches, jumps, trims, colour changes and stops, design bounds, the thread table
//! (RGB, name, catalogue number, brand, chart, colour type), metadata and, for two PES files,
//! the object section summary (version, class list, object and segment counts, thread table
//! size). Byte identity is checked only when asked for, since most formats carry labels, dates
//! or padding that do not change what is sewn.
//!
//! Formats place the origin differently (PES/PEC relative to the design centre, DST to the
//! first stitch, OpenStitch to the Design Page), so the actual design is first moved onto the
//! expected one by an [`Anchor`] (bounds centre by default). Runs of jumps are collapsed to
//! their last position (DST splits long moves), and end markers are ignored.
//!
//! Every difference is reported with its location (colour block, entry index on each side)
//! and both values; nothing is summarised away except past `maxDifferences`, where the
//! remaining count is given.

use crate::tolerance::Tolerance;
use crate::trace::{Anchor, Entry, Trace, command_name, distance};
use openstitch_core::{Command, StitchPattern, Thread};
use openstitch_formats::FileType;
use openstitch_formats::pes::ObjectSection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct CompareOptions {
    /// How the actual design is moved onto the expected one (`null`: not moved).
    pub origin: Option<Anchor>,
    /// Allowed distance between corresponding needle points, mm.
    pub position_mm: f64,
    /// Allowed difference of each bound, mm.
    pub bounds_mm: f64,
    /// Allowed difference of each count (stitches per block, totals).
    pub counts: Tolerance,
    /// Compare the streams entry by entry (order, positions, command types).
    pub sequence: bool,
    /// Collapse each run of jumps into its last jump.
    pub collapse_jumps: bool,
    pub threads: ThreadFields,
    pub metadata: bool,
    /// For two PES files: compare the object section summary.
    pub pes_objects: bool,
    /// Require byte-identical files (only meaningful for deterministic writers).
    pub bytes: bool,
    /// Stop recording differences after this many (the rest are counted).
    pub max_differences: usize,
    /// How far ahead the sequence comparison looks to resynchronise after an inserted or
    /// missing entry.
    pub resync_window: usize,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            origin: Some(Anchor::BoundsCenter),
            position_mm: 0.0,
            bounds_mm: 0.0,
            counts: Tolerance::EXACT,
            sequence: true,
            collapse_jumps: true,
            threads: ThreadFields::default(),
            metadata: true,
            pes_objects: true,
            bytes: false,
            max_differences: 1000,
            resync_window: 16,
        }
    }
}

/// Thread fields to compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ThreadFields {
    pub rgb: bool,
    pub name: bool,
    pub catalog: bool,
    pub brand: bool,
    pub chart: bool,
    pub kind: bool,
}

impl ThreadFields {
    /// Threads not compared at all (not even their number), e.g. against a DST file.
    pub const NONE: ThreadFields = ThreadFields {
        rgb: false,
        name: false,
        catalog: false,
        brand: false,
        chart: false,
        kind: false,
    };
}

impl Default for ThreadFields {
    fn default() -> Self {
        Self {
            rgb: true,
            name: true,
            catalog: true,
            brand: true,
            chart: true,
            kind: true,
        }
    }
}

/// One difference between the expected and the actual design.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    /// What differs, e.g. `stitch.position`, `block.stitches`, `thread.chart`, `pes.classes`.
    pub what: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<usize>,
    /// Entry index in the expected stream (after normalisation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_index: Option<usize>,
    /// Entry index in the actual stream (after normalisation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_index: Option<usize>,
    pub expected: Value,
    pub actual: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

impl Difference {
    fn new(what: &str, expected: Value, actual: Value) -> Self {
        Self {
            what: what.into(),
            block: None,
            expected_index: None,
            actual_index: None,
            expected,
            actual,
            delta: None,
        }
    }
}

/// Counts and size of one design, for the report.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub stitches: usize,
    pub jumps: usize,
    pub trims: usize,
    pub color_changes: usize,
    pub stops: usize,
    pub block_stitches: Vec<usize>,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl Summary {
    pub fn of(p: &StitchPattern) -> Self {
        let b = p.bounds();
        Self {
            stitches: p.count(Command::Stitch),
            jumps: p.count(Command::Jump),
            trims: p.count(Command::Trim),
            color_changes: p.count(Command::ColorChange),
            stops: p.count(Command::Stop),
            block_stitches: p
                .color_blocks()
                .iter()
                .map(|b| b.1.iter().filter(|s| s.command == Command::Stitch).count())
                .collect(),
            width_mm: b.width() / 10.0,
            height_mm: b.height() / 10.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub expected: Summary,
    pub actual: Summary,
    /// How far the actual design was moved to line up with the expected one, mm.
    pub origin_shift_mm: [f64; 2],
    pub differences: Vec<Difference>,
    /// Differences found past `maxDifferences` and not listed.
    pub unlisted: usize,
}

impl Comparison {
    pub fn total(&self) -> usize {
        self.differences.len() + self.unlisted
    }

    pub fn is_match(&self) -> bool {
        self.total() == 0
    }
}

struct Out<'a> {
    options: &'a CompareOptions,
    list: Vec<Difference>,
    unlisted: usize,
}

impl Out<'_> {
    fn push(&mut self, d: Difference) {
        if self.list.len() < self.options.max_differences {
            self.list.push(d);
        } else {
            self.unlisted += 1;
        }
    }

    fn count(&mut self, what: &str, block: Option<usize>, expected: usize, actual: usize) {
        let t = &self.options.counts;
        if !t.accepts(expected as f64, actual as f64) {
            self.push(Difference {
                block,
                delta: Some(actual as f64 - expected as f64),
                ..Difference::new(what, json!(expected), json!(actual))
            });
        }
    }
}

/// Entries the sequence comparison works on: no end markers, jump runs collapsed if asked.
fn normalised(trace: &Trace, collapse_jumps: bool) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::with_capacity(trace.entries.len());
    for e in &trace.entries {
        if e.command == Command::End {
            continue;
        }
        if collapse_jumps
            && e.command == Command::Jump
            && let Some(last) = out.last_mut()
            && last.command == Command::Jump
            && last.block == e.block
        {
            *last = *e;
            continue;
        }
        out.push(*e);
    }
    out
}

pub fn compare_patterns(
    expected: &StitchPattern,
    actual: &StitchPattern,
    options: &CompareOptions,
) -> Comparison {
    let mut out = Out {
        options,
        list: Vec::new(),
        unlisted: 0,
    };
    let (se, sa) = (Summary::of(expected), Summary::of(actual));
    let te = Trace::from_pattern(expected);
    let mut ta = Trace::from_pattern(actual);
    let mut shift = [0.0, 0.0];
    if let Some(anchor) = options.origin
        && let (Some(pe), Some(pa)) = (te.anchor(anchor), ta.anchor(anchor))
    {
        shift = [pe.0 - pa.0, pe.1 - pa.1];
        ta.translate(shift[0], shift[1]);
    }

    for (what, e, a) in [
        ("total.stitches", se.stitches, sa.stitches),
        ("total.jumps", se.jumps, sa.jumps),
        ("total.trims", se.trims, sa.trims),
        ("total.colorChanges", se.color_changes, sa.color_changes),
        ("total.stops", se.stops, sa.stops),
    ] {
        out.count(what, None, e, a);
    }
    if se.block_stitches.len() != sa.block_stitches.len() {
        out.push(Difference {
            delta: Some(sa.block_stitches.len() as f64 - se.block_stitches.len() as f64),
            ..Difference::new(
                "blocks",
                json!(se.block_stitches.len()),
                json!(sa.block_stitches.len()),
            )
        });
    }
    for (i, (e, a)) in se.block_stitches.iter().zip(&sa.block_stitches).enumerate() {
        out.count("block.stitches", Some(i), *e, *a);
    }

    if let (Some(be), Some(ba)) = (te.bounds(), ta.bounds()) {
        let names = ["minX", "minY", "maxX", "maxY"];
        let (ve, va) = ([be.0, be.1, be.2, be.3], [ba.0, ba.1, ba.2, ba.3]);
        for k in 0..4 {
            let d = va[k] - ve[k];
            if d.abs() > options.bounds_mm + 1e-9 {
                out.push(Difference {
                    delta: Some(d),
                    ..Difference::new(&format!("bounds.{}", names[k]), json!(ve[k]), json!(va[k]))
                });
            }
        }
    }

    if options.sequence {
        let blocks = te.block_count().max(ta.block_count());
        let (ne, na) = (
            normalised(&te, options.collapse_jumps),
            normalised(&ta, options.collapse_jumps),
        );
        for block in 0..blocks {
            let ie: Vec<usize> = (0..ne.len()).filter(|&i| ne[i].block == block).collect();
            let ia: Vec<usize> = (0..na.len()).filter(|&i| na[i].block == block).collect();
            compare_sequence(&ne, &ie, &na, &ia, block, &mut out);
        }
    }

    compare_threads(
        &expected.threads,
        &actual.threads,
        &options.threads,
        &mut out,
    );
    if options.metadata {
        let keys: std::collections::BTreeSet<&String> = expected
            .metadata
            .keys()
            .chain(actual.metadata.keys())
            .collect();
        for k in keys {
            let (e, a) = (expected.metadata.get(k), actual.metadata.get(k));
            if e != a {
                out.push(Difference::new(
                    &format!("metadata.{k}"),
                    json!(e),
                    json!(a),
                ));
            }
        }
    }

    Comparison {
        expected: se,
        actual: sa,
        origin_shift_mm: shift,
        differences: out.list,
        unlisted: out.unlisted,
    }
}

fn entry_value(e: &Entry) -> Value {
    json!({"command": command_name(e.command), "x": e.x, "y": e.y})
}

fn compare_sequence(
    ne: &[Entry],
    ie: &[usize],
    na: &[Entry],
    ia: &[usize],
    block: usize,
    out: &mut Out,
) {
    let tol = out.options.position_mm + 1e-9;
    let same =
        |a: &Entry, b: &Entry| a.command == b.command && distance((a.x, a.y), (b.x, b.y)) <= tol;
    let window = out.options.resync_window;
    let (mut i, mut j) = (0, 0);
    let missing = |out: &mut Out, i: usize| {
        out.push(Difference {
            block: Some(block),
            expected_index: Some(ie[i]),
            ..Difference::new("entry.missing", entry_value(&ne[ie[i]]), Value::Null)
        })
    };
    let extra = |out: &mut Out, j: usize| {
        out.push(Difference {
            block: Some(block),
            actual_index: Some(ia[j]),
            ..Difference::new("entry.extra", Value::Null, entry_value(&na[ia[j]]))
        })
    };
    while i < ie.len() && j < ia.len() {
        let (e, a) = (&ne[ie[i]], &na[ia[j]]);
        if same(e, a) {
            i += 1;
            j += 1;
            continue;
        }
        // Resynchronise on the nearest match after skipping entries on one side.
        let skip_e = (1..=window).find(|&k| i + k < ie.len() && same(&ne[ie[i + k]], a));
        let skip_a = (1..=window).find(|&k| j + k < ia.len() && same(e, &na[ia[j + k]]));
        match (skip_e, skip_a) {
            (Some(k), b) if b.is_none_or(|b| k <= b) => {
                for s in 0..k {
                    missing(out, i + s);
                }
                i += k;
            }
            (_, Some(k)) => {
                for s in 0..k {
                    extra(out, j + s);
                }
                j += k;
            }
            _ => {
                let what = if e.command == a.command {
                    "entry.position"
                } else {
                    "entry.command"
                };
                out.push(Difference {
                    block: Some(block),
                    expected_index: Some(ie[i]),
                    actual_index: Some(ia[j]),
                    delta: (e.command == a.command).then(|| distance((e.x, e.y), (a.x, a.y))),
                    ..Difference::new(what, entry_value(e), entry_value(a))
                });
                i += 1;
                j += 1;
            }
        }
    }
    for k in i..ie.len() {
        missing(out, k);
    }
    for k in j..ia.len() {
        extra(out, k);
    }
}

pub fn thread_value(t: &Thread) -> Value {
    json!({
        "rgb": t.hex(),
        "name": t.name,
        "catalog": t.catalog_number,
        "brand": t.brand,
        "chart": t.chart,
        "kind": t.kind.unwrap_or(Thread::CUSTOM_KIND),
    })
}

fn compare_threads(expected: &[Thread], actual: &[Thread], fields: &ThreadFields, out: &mut Out) {
    if *fields == ThreadFields::NONE {
        return;
    }
    if expected.len() != actual.len() {
        out.push(Difference {
            delta: Some(actual.len() as f64 - expected.len() as f64),
            ..Difference::new("threads.count", json!(expected.len()), json!(actual.len()))
        });
    }
    for (i, (e, a)) in expected.iter().zip(actual).enumerate() {
        let (ve, va) = (thread_value(e), thread_value(a));
        for (key, on) in [
            ("rgb", fields.rgb),
            ("name", fields.name),
            ("catalog", fields.catalog),
            ("brand", fields.brand),
            ("chart", fields.chart),
            ("kind", fields.kind),
        ] {
            if on && ve[key] != va[key] {
                out.push(Difference {
                    block: Some(i),
                    ..Difference::new(&format!("thread.{key}"), ve[key].clone(), va[key].clone())
                });
            }
        }
    }
}

/// Class names of a PES object section in file order, as far as OpenStitch parses it.
pub fn pes_classes(s: &ObjectSection) -> Vec<String> {
    let mut v: Vec<String> = s
        .objects
        .iter()
        .flat_map(|_| ["CEmbOne".to_string(), "CSewSeg".to_string()])
        .collect();
    v.extend(s.shapes.iter().map(|_| "CEmbLine".to_string()));
    v.extend(s.texts.iter().map(|_| "CEmbNText".to_string()));
    v.extend(s.stitch_groups.iter().map(|_| "CEmbOne".to_string()));
    v.extend(s.groups.iter().map(|_| "CGroupObject".to_string()));
    if let Some(u) = &s.unparsed {
        v.push(u.clone());
    }
    v
}

/// Report-sized summary of a PES object section.
pub fn pes_summary(s: &ObjectSection) -> Value {
    json!({
        "version": s.version,
        "classes": pes_classes(s),
        "objects": s.objects.len(),
        "segments": s.objects.iter().map(|o| o.segments.len()).collect::<Vec<_>>(),
        "threads": s.threads.len(),
        "metadata": s.metadata,
        "unparsed": s.unparsed,
    })
}

pub fn compare_pes_objects(
    expected: &ObjectSection,
    actual: &ObjectSection,
    options: &CompareOptions,
) -> Vec<Difference> {
    let mut out = Out {
        options,
        list: Vec::new(),
        unlisted: 0,
    };
    if expected.version != actual.version {
        out.push(Difference::new(
            "pes.version",
            json!(expected.version),
            json!(actual.version),
        ));
    }
    let (ce, ca) = (pes_classes(expected), pes_classes(actual));
    if ce != ca {
        out.push(Difference::new("pes.classes", json!(ce), json!(ca)));
    }
    out.count(
        "pes.objects",
        None,
        expected.objects.len(),
        actual.objects.len(),
    );
    out.count(
        "pes.threads",
        None,
        expected.threads.len(),
        actual.threads.len(),
    );
    for (i, (e, a)) in expected.objects.iter().zip(&actual.objects).enumerate() {
        out.count(
            "pes.object.segments",
            Some(i),
            e.segments.len(),
            a.segments.len(),
        );
        for (what, ve, va) in [
            ("pes.object.widthMM", e.width, a.width),
            ("pes.object.heightMM", e.height, a.height),
        ] {
            let d = (f64::from(va) - f64::from(ve)) / 10.0;
            if d.abs() > options.bounds_mm + 1e-9 {
                out.push(Difference {
                    block: Some(i),
                    delta: Some(d),
                    ..Difference::new(
                        what,
                        json!(f64::from(ve) / 10.0),
                        json!(f64::from(va) / 10.0),
                    )
                });
            }
        }
    }
    let mut list = out.list;
    list.truncate(options.max_differences);
    list
}

/// The first differing byte offset, or the shorter length when one file is a prefix.
pub fn first_byte_difference(expected: &[u8], actual: &[u8]) -> Option<Difference> {
    let n = expected.len().min(actual.len());
    let at = (0..n)
        .find(|&i| expected[i] != actual[i])
        .or((expected.len() != actual.len()).then_some(n))?;
    let byte = |b: &[u8]| b.get(at).map_or(Value::Null, |v| json!(format!("{v:02x}")));
    Some(Difference {
        expected_index: Some(at),
        actual_index: Some(at),
        ..Difference::new(
            "bytes",
            json!({"length": expected.len(), "byte": byte(expected)}),
            json!({"length": actual.len(), "byte": byte(actual)}),
        )
    })
}

/// Reads both files and compares them; PES object sections and bytes as the options say.
pub fn compare_files(
    expected: (&[u8], FileType),
    actual: (&[u8], FileType),
    options: &CompareOptions,
) -> Result<Comparison, String> {
    let read = |(bytes, ty): (&[u8], FileType), side: &str| {
        ty.read(bytes)
            .map_err(|e| format!("{side} file ({}): {e}", ty.extension()))
    };
    let (pe, pa) = (read(expected, "expected")?, read(actual, "actual")?);
    let mut c = compare_patterns(&pe, &pa, options);
    let mut extra = Vec::new();
    if options.pes_objects && expected.1 == FileType::Pes && actual.1 == FileType::Pes {
        let objects = |b: &[u8], side: &str| {
            openstitch_formats::pes::read_objects(b).map_err(|e| format!("{side} PES objects: {e}"))
        };
        extra.extend(compare_pes_objects(
            &objects(expected.0, "expected")?,
            &objects(actual.0, "actual")?,
            options,
        ));
    }
    if options.bytes {
        extra.extend(first_byte_difference(expected.0, actual.0));
    }
    for d in extra {
        if c.differences.len() < options.max_differences {
            c.differences.push(d);
        } else {
            c.unlisted += 1;
        }
    }
    Ok(c)
}
