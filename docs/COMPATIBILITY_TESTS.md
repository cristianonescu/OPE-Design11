# Compatibility tests

## The compatibility harness

Compatibility is tested by `stitchtool compat` (crate `crates/openstitch-compat`). It reads
declarative specs from `compat/specs/`, runs them headlessly, measures the files they export, and
writes a report. The current tier of every FEATURE_MATRIX row and audit item is in the generated
[COMPATIBILITY_STATUS.md](COMPATIBILITY_STATUS.md).

```bash
cargo run -p stitchtool -- compat
```

The command writes `target/compat/report.json` (every check, value and difference) and
`target/compat/summary.md`. It exits non-zero when a spec fails, errors or is invalid.

| Flag | Default | Use |
|---|---|---|
| `--specs DIR` | `compat/specs` | Spec directory (`<area>/<id>.json`). |
| `--reference DIR` | `compat/reference` | PE-DESIGN reference files and their `MANIFEST.json`. |
| `--machine DIR` | `compat/machine` | Sew-out records. |
| `--work DIR` | `target/compat/work` | Scratch; one fresh directory per spec, holding its exports. |
| `--report FILE`, `--summary FILE` | `target/compat/…` | Outputs. |
| `--matrix FILE` | `docs/FEATURE_MATRIX.md` | Every row a spec names must exist exactly once. |
| `--status FILE` | — | Writes the generated tier table. |
| `--check-status FILE` | — | Fails if FILE differs from the table this run generates (CI). |
| `--only TEXT` | — | Runs only specs whose `area/id` contains TEXT: an id, part of one, `area/id` as the summary shows it, or `area/`. Matching no spec is an error that lists the closest spec names. |

CI runs `compat --check-status docs/COMPATIBILITY_STATUS.md` on the ubuntu-latest job. It adds
the summary to the job page and uploads the report as the `compat-report` artifact.
`openstitch-compat/tests/repo_specs.rs` runs the same specs on all three operating systems.

### Tiers

Each check has one tier, and the tiers are kept apart.

| Tier | Evidence | Where it comes from |
|---|---|---|
| internal-regression | OpenStitch's own earlier output, or the guide's description of the behaviour | `expected` values in the spec |
| oracle | An independent implementation (pyembroidery 1.5.1, libembroidery) | `expected` values marked `"tier": "oracle"`, or `against` a fixture file |
| reference | PE-DESIGN 11 output supplied by the owner | `compat/reference/` plus a provenance entry in `MANIFEST.json` |
| machine | A sew-out on a Brother machine | a record in `compat/machine/` for the current export |

These rules decide what a result can claim:

- A spec reaches a tier only when every check of that tier passes, and so does every lower tier
  that has checks. A failing internal check therefore blocks a passing reference check.
- A reference check whose file does not exist yet is **pending**: reported, never passed and
  never failed. A spec with only pending checks has the status "pending reference".
- A reference file with no manifest entry, an incomplete entry, or a SHA-256 that does not match
  is an **error**. Nothing reaches the reference tier without a provenance record.
- A sew-out record counts only while its SHA-256 matches the current export.

[compat/reference/README.md](../compat/reference/README.md) is the owner's guide to adding a
reference file: what to make in PE-DESIGN, how to name it, and what to record.
[compat/machine/README.md](../compat/machine/README.md) does the same for sew-outs.

### Spec format (`openstitch-compat/1`)

```json
{
  "schema": "openstitch-compat/1",
  "id": "satin-zigzag-rect",
  "area": "stitches",
  "title": "Region satin is a zigzag across the column",
  "description": "…",
  "covers": {"audit": [1], "featureMatrix": [{"area": "Region sew types", "feature": "Satin: a zigzag"}]},
  "evidence": [{"kind": "guide", "url": "https://support.brother.com/…", "note": "…"}],
  "inputs": [{"from": "crates/openstitch-formats/tests/fixtures/oracle.dst", "name": "oracle.dst"}],
  "script": [{"command": "shape.rectangle", "params": {"x": 10, "y": 10, "width": 5, "height": 2}},
             {"id": "pts", "command": "stitches.points", "params": {"id": 1}}],
  "exports": [{"path": "satin.pes", "pesVersion": "6"}],
  "references": [{"id": "pe-design", "file": "stitches/satin-zigzag-rect.pes", "make": "what to draw and save"}],
  "machine": {"record": "stitches/satin-zigzag-rect.json", "make": "what to sew and look at"},
  "measurements": [
    {"id": "stitches", "measure": {"kind": "stitchSegments"},
     "source": {"export": "satin.pes", "align": {"anchor": "boundsMin", "to": [10, 10]}},
     "expected": 15, "tolerance": {"count": 1}, "evidence": "audit item 1",
     "reference": {"tolerance": {"percent": 10}}}
  ]
}
```

The fields:

- **`covers`** names audit items and FEATURE_MATRIX rows. A row is named by its Area cell and
  the start of its Feature cell.
- **`evidence`** has kinds `guide`, `spec`, `audit`, `measured` and `oracle`. URLs must be https.
- **`script`** is what `stitchtool exec` takes. An optional `id` labels a step, so a
  measurement can read its reply.
  - The script runs in a session sandboxed to the spec's work directory.
  - `inputs` are repository files copied into that directory first.
  - Paths must be relative and must not contain `..`.
- **`exports`** are written after the script, with `design.export`.
  - `fromInput` writes an input file again through the codecs instead.
  - `fromScript` reads a file the script wrote itself.
  - Each export is read back with OpenStitch's reader before it is measured.
- **`measurements`** each have:
  - a `measure` (below) and a `source`: an `export` or a labelled `step`, optionally one
    `colorBlock`, an `align` anchor and an `exclude` (`{"first": n, "last": m}`: the first
    and last stitch segments are left out, e.g. a tie-in and an end lock; the measurement's
    evidence says why);
  - an `expected` value with a `tolerance`, a `reference` check, or both;
  - an `evidence` string saying where the expected value comes from.
- **Expected values** are compared structurally:
  - numbers within the tolerance (`mm`, `count`, `deg` (circular), `percent`, `modulo`);
  - a number against an array checks every element;
  - `{"min": a, "max": b}` is a range.
- **A reference check** takes the same measurement on the reference file, with the same
  `colorBlock` and `align`, and compares the two values. For `compare` and `bytesIdentical` it
  compares the two files directly.

### Measurements

| Kind | Value |
|---|---|
| `stitchCount`, `stitchSegments`, `commandCount {command}` | Needle penetrations; stitches between consecutive penetrations; entries of one command |
| `interiorMoves {command}` | Trims or jumps between the first and last stitch of each colour block (inside an object) |
| `colorBlocks`, `blockStitchCounts` | Sewn colour blocks; stitches per block |
| `bounds {property}` | `width`, `height`, `minX`… of stitches and jumps, mm |
| `stitchLength {stat, minLength}`, `stitchLengthHistogram {bin}` | Min / max / mean / median stitch length; counts per length bin |
| `edgeStitches {rails, within, maxLength}`, `offRailPoints`, `sideAlternation` | Stitches lying along an edge; needle points on no edge; share of stitches crossing from one edge to the other |
| `dominantAngle {bin}` | Direction (0–180°, +y down) carrying the most stitch length |
| `angleOffPerpendicular {stations, radius, minLength}` | Largest deviation from perpendicular of stitches crossing stations on a band |
| `stitchesOutside {polygon, within}` | Stitch midpoints outside a region |
| `travelGaps`, `totalJumpLength` | Needle travel across each jump / trim run; total jump length, mm |
| `polar {center, ring?, ringWithin, property}` | Needle points seen from `center` (mm): `minRadius`, `maxRadius` (mm), `midAngle`, `angleSpan` (degrees clockwise on screen from 12 o'clock); points within `ringWithin` mm (default 0.3) of a circle of radius `ring` (an outline sewn with fitted text) are left out |
| `columns {x, halfWidth}` | For each x (mm), the highest and lowest needle point within `halfWidth` mm (default 0.3): `[top0, bottom0, …]`, mm |
| `needleEnds` | First and last needle point `[x0, y0, x1, y1]`, mm from the top-left of the needle points (a jump landing right before the first stitch counts as the start) |
| `stitchedArea` | Area covered by 0° fill rows, mm²: each row's needle extent (with the stitches into and out of it from ≤ 0.5 mm away) × the mean row gap. Single points (under sewing across the fill, travel) are not rows |
| `rowShift {stitchLength, interior}` | Row-to-row needle shift of a 0° fill (Fill Frequency). Only fill rows count: runs with two consecutive interior needle points one stitch length (±0.15 mm) apart. Under sewing, edge runs and travel are skipped |
| `rowSpacing` | Mean distance between the distinct lines of a 0° fill's rows (runs of two or more needle points on one horizontal line), mm: (last − first) / (lines − 1), so the file's 0.1 mm rounding cancels out. Under sewing edge runs would count as lines: use it with under sewing off |
| `rowWidth` | Width of a 0° fill's row needle extent (runs of two or more needle points on one line), mm: the outline's width plus twice the pull compensation. Under sewing and travel lie inside it |
| `pesVersion`, `pesClasses`, `pesSummary`, `metadata {key}`, `threads` | PES object section, header strings, thread table |
| `replyValue {pointer, pluck}` | A JSON pointer into a labelled step's reply |
| `refused {command, params, errorContains}` | Whether a command is refused after the script |
| `compare {against, options}` | Semantic comparison (below); the number of differences, each listed |
| `bytesIdentical {against}` | Byte identity; the first differing offset |

### Semantic comparer

`openstitch_compat::compare` compares an expected design (reference or oracle) with the actual
one (OpenStitch). Each of the following has its own tolerance:

- the colour block count;
- stitches per block;
- totals of stitches, jumps, trims, colour changes and stops;
- bounds;
- the thread table (RGB, name, catalogue, brand, chart, colour type), field by field;
- metadata;
- for two PES files: version, class list, object and segment counts, thread table size;
- needle positions and command types, entry by entry within each colour block, so stitch order
  counts. After an inserted or missing entry, the comparer resynchronises within a window, so one
  extra stitch is one difference, not a cascade.

Normalisation and reporting:

- **Origins.** The actual design is moved onto the expected one by an anchor: bounds centre
  (default), bounds min, first stitch, or none. PES/PEC, DST and the Design Page place the
  origin differently.
- **Jumps.** Runs of jumps are collapsed (DST splits long moves). End markers are ignored.
- **Bytes.** Byte identity is checked only when asked for.
- **Reporting.** Every difference is reported with its location (colour block, entry index on
  each side) and both values. The list stops at `maxDifferences`, and the rest are counted.

Determinism: specs run in path order and numbers in the report are rounded to 4 decimals. No
times or absolute paths are written. The status table holds no measured numbers.

### Specs

The first specs (2026-10-19) converted the audit's tests; since 2026-10-27 each PE-DESIGN
reference file lands with its own spec (fill sweeps, satin, lines, lettering, PES), and the
full list with the tier each has reached is the generated
[COMPATIBILITY_STATUS.md](COMPATIBILITY_STATUS.md). The Rust tests (`satin_compat.rs`,
`region_compat.rs`, `pes_compat.rs`, `order_holes.rs`, the formats oracle tests) stay as they
are. The specs that cover the audit's items:

| Audit item | Specs |
|---|---|
| 1 Satin zigzag | `satin-zigzag-rect`, `satin-zigzag-column`, `satin-zigzag-skeleton` |
| 2 Default PES v6 | `pes-default-v6`, `pes-library-convert-v6` |
| 3 Running Stitch Path | `region-travel-u-fill`, `-along-outline`, `-on-outline`, `-satin`, `-gradation`, `running-stitch-path-exclusions` |
| 7 Auto direction | `auto-satin-bent-band` |
| 14 Programmable fill 45° | `programmable-fill-45` |
| 15 Frequency and ranges | `fill-frequency-0`, `-25`, `-50`, `-default`, `attribute-ranges` |
| 16 Optimize entry/exit | `optimize-entry-exit` |
| Shape-tool start points | `shape-start-points` |
| 17 Hole sewing | `hole-sewing-pentagon-star`, `hole-sewing-limits` |
| 18 PES thread chart | `pes-thread-chart-roundtrip` (oracle) |
| 19 Design Property | `pes-design-properties` |
| 5 Editable PES objects | `pes-0110-editable-shapes`, `pes-0110-editable-text` (reference) |

The counts (specs, passes, pending references) and the tier per spec are in
[COMPATIBILITY_STATUS.md](COMPATIBILITY_STATUS.md), regenerated by every run.

Binary differences caused by non-functional metadata (labels, padding, timestamps) are listed
separately from functional differences: `compare` reports what is sewn, and `bytesIdentical` is
used only where the writer is deterministic.

## Oracle tier (independent implementation, not PE-DESIGN)

Fixtures produced by `tools/genfixtures.py` with pyembroidery 1.5.1: 41 stitches in two colour
blocks with a trim + jump and one stitch longer than 121 units.

| Test | Expected | Actual | Result |
|---|---|---|---|
| DST read matches reference decoder | 44 entries identical | identical | pass |
| DST write(read(f)) read back | same needle points and colour changes (the writer lays records out as PE-DESIGN, see the reference tier) | as expected | pass |
| PES/PEC read matches reference decoder | identical stitches, threads `#ed171f`, `#0a55a3` | identical | pass |
| PES/PEC write(read(f)) == f | byte-identical incl. icons | byte-identical | pass |
| DST every delta −121…121 encodes/decodes | lossless | lossless | pass |
| PEC long-form deltas and > 2047 interpolation | positions preserved, trim kept | preserved | pass |
| EXP read / write(read(f)) read back | identical / same needle points | identical | pass |
| JEF read (no trim inference) / write(read(f)) read back | identical records, threads `#ff0000` `#0b2f84` (pyembroidery's 3 mm trim rule is not PE-DESIGN's) / same needle points | identical | pass |
| JEF stop encoded as colour 0 and read back | 1 stop, 0 colour changes | as expected | pass |
| DST frame moves padded to 2, 3, 5 jump records; read with the same count | the move and the jump home read as trims | as expected | pass |
| XXX read / write(read(f)) read back | identical, threads `#e01010` `#1040c0` / same needle points | identical | pass |
| XXX end offset at 0xFC points at `7F 7F 02 14` | yes | yes | pass |
| PCS read matches reference decoder (fixture assembled, see FILE_FORMATS.md) | identical, exact RGB threads | identical | pass |
| PCS write(read(f)) read back; 1/6 mm grid with a fraction byte survives a round trip | same needle points within 0.01 (threads from PE-DESIGN's fixed table) | as expected | pass |
| SEW read matches reference decoder (fixture assembled) | identical, chart threads `#ff0000` `#386cae` | identical | pass |
| SEW write(read(f)) read back | same needle points (PE-DESIGN's zero-length stitch before a colour change aside) | as expected | pass |
| PES v1, v6, v6 truncated (with `CEmbOne`/`CSewSeg`, thread table) read matches reference | identical; v6 threads `#e01010` `#1040c0` (exact RGB) | identical | pass |
| PES v1, v6, v6t write(read(f)) == f | byte-identical | byte-identical | pass |
| PES v6 with thread chart, brand, catalogue and description (`oracle_v6_chart.pes`): write(read(f)) == f | byte-identical (chart strings and colour type kept) | byte-identical | pass |
| Default PES (`FileType::Pes.write`) == pyembroidery `6` | byte-identical to `oracle_v6.pes` | byte-identical | pass |
| HUS read matches reference decoder (fixture assembled, literal-only streams) | identical, chart threads `#ff0000` `#0000e7` | identical | pass |
| HUS written by OpenStitch (LZ77 + Huffman) read back | same needle points, threads, trims and colour changes as the oracle design; an earlier layout of the same compressor's output was read by the reference decoder (`openstitch_hus_read.json`) | as expected | pass |
| SHV read matches reference decoder (fixture assembled) | identical, chart threads `#ff0000` `#0000ff`, name `oracle` | identical | pass |
| PHC read matches reference decoder (fixture assembled around `oracle.pec`) | identical, PEC chart threads | identical | pass |
| CSD (spec-derived): synthetic type-0 and type-1 files | 303 stitches, 1 colour change, 1 trim, palette colours | as expected | pass (self-consistency only) |
| VIP written by OpenStitch read by libembroidery `sew` (built locally, commit `1138e04`) | 44 records, colours `224,16,16` and `16,64,192` | as expected; stitch coordinates not checkable (stub decompressor) | partial |

PCS, SEW and HUS have no reference writer in pyembroidery 1.5.1. The reads are the oracle checks;
the writers of every machine format follow PE-DESIGN 11.50's record layout instead of
pyembroidery's since 2026-10-28 (reference tier, `machine-formats`), so write(read(f)) is checked
by reading the result back, not byte for byte.

Implementation under test: the crate `crates/openstitch-formats`.

Run: `cargo test -p openstitch-formats` (every fixture reads, writes in PE-DESIGN's layout and
reads back to the same needle points). Golden engine regressions:
`cargo test -p openstitch-engine` (`script_output_matches_golden_pes`) and
`cargo test -p openstitch-design` (`golden_project_compiles_to_recorded_statistics`).

### Imports (Rust)

| Test | Expected | Result |
|---|---|---|
| SVG rect, circle, quadratic path, transformed group with a hole | 4 shapes; sizes in mm exact (f32 tolerance); circle as cubic curves; group contours offset by the transform | pass |
| `import.svg` of `examples/imports/leaf.svg`-like artwork | 3 grouped shapes, fill/stroke colours kept, 40 mm wide at (10, 15) mm, one Undo, rescale to 80 mm | pass |
| Hostile SVG: empty, not XML, truncated, NaN path, entity expansion, 5,001 paths, `<image href="/etc/hosts">`, path outside the sandbox | `Err` or nothing read; no file outside the sandbox opened | pass |
| Background image: place 200×100 px at 50 mm, show/hide, opacity, save/open `.osd`, clear + undo, bad PNG and bad parameters | as expected | pass |

### Split export (Rust)

| Test | Expected | Result |
|---|---|---|
| 300 colour blocks to PES and VIP; 1,500 blocks to DST; 2 × 70,000 records to PCS | parts of 255 / 100 blocks, ≤ 999 changes, ≤ 65,535 records; every stitch and thread in order; later parts start with a jump | pass |
| `design.export` of 260 blocks to PES: without `split` an error and no file; with `split` `big-1.pes`, `big-2.pes` holding all 260 blocks | as expected | pass |

### Hostile input (Rust)

| Test | Expected | Result |
|---|---|---|
| Every reader on every prefix of every fixture | `Ok` or `Err`, no panic | pass |
| Each reader on its own fixture cut inside its header | `Err` | pass |
| 3,040 random inputs (0–11 KB) through all readers; 12,000 corrupted fixtures | no panic | pass |
| PES PEC offset 0, = file length, `u32::MAX` | `Truncated` | pass |
| JEF colour count `u32::MAX`; SEW 65,535 colours; PCS 65,535 colours | `Truncated`, no large allocation | pass |
| HUS/VIP record or colour count `u32::MAX`; stream offset past the end; bad magic | `ValueOutOfRange` / `Truncated` / `BadSignature` | pass |
| Compressed streams: over-subscribed or > 16-bit codes, unassigned codes, distances before the start | `Err`; output capped at the record count | pass |
| cargo-fuzz, one target per reader, 30 s each seeded with the fixture (1.0–7.9 M runs per reader, 32.6 M total) | no crash | pass |
| cargo-fuzz `read_hus` / `read_vip` / `read_shv`, 25 s each (236 k / 977 k / 3.65 M runs); `read_phc` / `read_csd`, 20 s each (1.18 M / 1.36 M runs), 2026-10-19 | no crash | pass |

Fuzz locally (nightly + `cargo install cargo-fuzz`):
`cd crates/openstitch-formats && cargo +nightly fuzz run read_dst -- -max_total_time=30`.

## Reference tier (PE-DESIGN 11)

Reference files are added one spec at a time, each landing together with the OpenStitch change
that makes its reference checks pass, so `main` stays green. They are made by the owner in a
licensed PE-DESIGN 11.50 through its normal interface (black-box), and every file has a
provenance entry in `compat/reference/MANIFEST.json`. Each spec's `references[].make` says
what to draw; `target/compat/summary.md` lists the specs still waiting for a file or a sew-out,
and [COMPATIBILITY_STATUS.md](COMPATIBILITY_STATUS.md) the tier each has reached. Add files as
[compat/reference/README.md](../compat/reference/README.md) describes. Observations that are not
files (for example, a PES that PE-DESIGN opens blank) go here.

### `pes-design-properties` — reference, 2026-10-27

Made in PE-DESIGN 11 Layout & Editing **11.50** on Windows 11 Pro 10.0.26200 by the owner,
black-box: `target/compat/work/pes-design-properties/rose.pes` (an OpenStitch export) was opened
in PE-DESIGN, its Design Property was viewed, and it was re-saved unchanged with Save As. The
default Save As type is "Layout & Editing Ver.11.0 File", so the file carries the header
`#PES0110`. Provenance and SHA-256 are in
[MANIFEST.json](../compat/reference/MANIFEST.json).

Finding this file surfaced: OpenStitch's PES reader had no case for the PE-DESIGN 11 header
version `#PES0110`, so it left the object-section metadata unparsed and reported the PEC label
(`pes-desi`) as the design name. The reader now parses the `0110` metadata (same documented layout
as `0040`+: pec offset, `01 00`, a two-char code, then the five length-prefixed strings), so the
five Design Property fields read back from a PE-DESIGN 11 file. The thread-table and object
offsets after the metadata are not publicly described for this version; they are left unparsed,
and stitches and chart colours still come from the PEC section.

Also observed: PE-DESIGN preserves the string bytes on save but *displays* them through the
Windows code page, so OpenStitch's former UTF-8 `é` showed as `Ã©` in the Design Property dialog,
and a bare LF in the comment showed as no line break. A Design Property typed into PE-DESIGN
itself (`pes/design-property-native.pes`, system locale en-US, ANSI code page 1252) settles the
convention: `é` is the single byte `E9` (Windows-1252) and the comment line break is `0D 0A`
(CR LF). Public descriptions don't cover this (pyembroidery reads and writes UTF-8; edutechwiki
says ASCII). OpenStitch now writes PES header strings (Design Property and thread-table strings)
as Windows-1252 with CR LF, with `?` for characters outside that code page. It reads CR LF back
as LF, and reads valid UTF-8 as UTF-8 so its own earlier files and pyembroidery's still read
as written. A unit test checks the writer's bytes against the native file.

### `pes-default-v6` — reference, 2026-10-27

Made in PE-DESIGN 11.50 like the file above: OpenStitch's `a.pes` (`#PES0060`) opened with its
stitches (not blank) and was re-saved unchanged with Save As (`#PES0110`). PE-DESIGN reported 930
stitches where OpenStitch's file had 932. OpenStitch's design compiler pushed a zero-length stitch
at the point each jump landed on (the start of each colour block), and the PEC writer encoded it
as a `00 00` record. None of the 16 PE-DESIGN reference files has a zero-length stitch record:
each block starts with a jump record followed by real stitches, so the jump's end is the first
needle point. PE-DESIGN dropped the two records on open. The compiler no longer emits that
stitch, and stitch objects re-sewn from a file start each run at the jump's end. The PEC encoder
no longer adds pyembroidery's `00 00` padding before a diagonal first stitch after a jump
(PE-DESIGN's files have none). It still writes any zero-length stitch it is given, so
pyembroidery's files still rewrite byte for byte.

Remade the same day (pe-gui): the spec now sews two open polylines as running stitch (run pitch
2.5 mm, run times 1, no region), so the reference no longer changes when region generation does.
PE-DESIGN opened the first export with 54 stitches where OpenStitch's file had 56. PE-DESIGN reads
the `CSewSeg` runs and sews only the points of stitch runs: OpenStitch put each jump's landing
only in the jump segment, so after the change above that needle point was lost (the zero-length
stitch had carried it before). PE-DESIGN's own re-saved file reopened unchanged. Each stitch run
after a jump now starts at the landing; PE-DESIGN then reported 56 stitches, and that re-save is
the reference.

### Lettering (`text-*`): reference, 2026-10-27

Made by pe-gui in PE-DESIGN 11.50 with Lato Bold (OFL, the font OpenStitch bundles),
installed for the Windows user and used as a TrueType font; no Brother fonts. Fill Stitch
at its defaults, 800 RED, Save As default type.
- `text-size-h`, `text-size-hg`: H and Hxg at size 20. PE's letters are 1.03 × OpenStitch's
  earlier ones in both directions, stems and counters alike, so the size is 0.7 em (Lato's
  capitals are 1446/2000 em); OpenStitch now matches to the 0.1 mm grid.
- `text-line-spacing-50/100/150`: two 10 mm lines of H. The pitch is linear in the % ("% of
  char height", default 100): 7.9 / 15.9 / 23.8 mm, the font's Windows ascent on that em at
  100 %.
- `text-fit-*`: ABCDE, 8 mm, fitted to a 60 mm circle sewn in running stitch, with each
  dialog option changed alone. Measured with `polar` about the circle's centre, leaving out
  the circle's own needle points. The circle's outline starts and ends at its bottom: left
  alignment starts the text there and runs clockwise, centre centres it at 12 o'clock.
  Centred and below drop the text by half the size and the size (not the capital height).
  `text-fit-otherside` widens `ringWithin` to 0.5 mm: its baseline lies on the circle, and
  both files have two points 0.27–0.30 mm outside it that the default window splits.
  Stretched has no spec: PE's file puts the 140 mm text off the page, beyond what the PES
  reader takes in. Spaced evenly centres each character in an equal share of the outline.
- `text-defaults-h`, `text-defaults-fill`: H at 20 mm with PE's text defaults (Satin,
  density 5.0, under sewing off), and with only the region switched to Fill (density 5.0,
  under sewing off, Frequency 30 %). Box exact; stitch counts within 25 % (544 vs 483 Fill,
  221 vs 285 Satin; see the audit). The other text specs switch their region to Fill as their
  make text says, since text now defaults to Satin. `text-fit-halign-spaced` has no mid-angle
  check: evenly spaced letters leave equal gaps, so the polar measure's middle is undefined.
- `text-sewing-order`: AB with a red fill and a black running outline. PE sews both fills,
  then both outlines, in two colour blocks, as OpenStitch does. The outline block's stitch
  count waits for the lines track's PE line defaults (pitch 2.0 mm, tie-in and lock).
- `text-transform-N`: HHHHHHH, 10 mm, centred at (50, 50) mm, with Text Transform preset N
  (the two arches, then PE's grid in reading order; 26 presets). The first set was saved with
  the text off the page and PE then wrote no PEC section, so pe-gui remade them on the page.
  `tools/fit_envelopes.py` reads every H stem's top and bottom needle point and fits each
  preset's top and bottom edge in the straight text's box (constant, slope and cosine terms,
  outliers dropped; mirror pairs and symmetric presets pooled), into
  `crates/openstitch-design/src/envelope_presets.rs`. The specs check the box (`bounds`) and the
  14 stems (`columns`, 1.05 mm half width: OpenStitch's fill rows end on the stem edges) within
  0.5 mm. The arches set the letters rigidly along an arc whose inner line keeps the text's
  width (1.879 rad), 0.55 % of the width left of centre, as PE does. 13 and 14 (fans, since
  2026-10-28) are fitted from the letters' corners: each letter keeps its height and is sheared
  between one edge of the straight box and the other edge scaled by 0.70 about the centre (13
  the bottom, 14 the top); their stems slant, so their specs check the box and the counts and
  the unit test checks every letter's corners.

### `machine-formats`: reference, 2026-10-28

Made by pe-gui in PE-DESIGN 11.50 on 2026-10-27: OpenStitch's `a.pes` (two Red running lines
40 mm apart with a trim between them, then a Black satin of 15 mm stitches; 248 stitches) was
opened as stitches, nothing changed, and exported through App menu > Export to every type the
list offers (DST, EXP, PCS, HUS, VIP, SHV, JEF, SEW, CSD, XXX; no PEC or PHC; no options
dialog and no warning for any of them; Save As offers only PES). The eight formats OpenStitch
writes are compared record by record (`compare`, 0.1 mm, PCS 0.2 mm) with OpenStitch's export
of the same stitches; SHV and CSD stay in the owner's notes (read-only formats, no writer to
compare). Before the writers followed PE-DESIGN's layout the compare failed in every format
(441–619 differing entries, the HUS magic rejected); the differences and the rules now written
are in FILE_FORMATS.md "PE-DESIGN's record layout" and in the audit. The spec imports OpenStitch's saved a.pes
(the file PE-DESIGN opened) and exports it, so the compare does not depend on the stitch
generator; imported stitch blocks keep their needle points whole on export. The SEW compare runs
from the first record: the round-3 exports (`sew-a-moved10.sew`, byte-identical to the export
of the unmoved design; `sew-square-centre.sew`) showed PE-DESIGN's SEW stream is design-relative
and starts 15.9 mm left of and 9.7 mm above the design centre, where OpenStitch now starts it.

### `pes-csewseg-trims`: reference, 2026-10-27

Made by pe-gui in PE-DESIGN 11.50: OpenStitch's `a.pes` (two Red running lines 40 mm apart, one
colour, a trim between them) was opened unchanged and saved with Save As, default type
(`#PES0110`, the reference), and also as Layout & Editing Ver.6.0 (`#PES0060`, kept as
`crates/openstitch-formats/tests/fixtures/pe_design_v6_trims.pes`). The PEC-only `1t` export
opened blank, so it was not re-saved. The audit listed "CSewSeg drops trims" as a gap. The
re-saves show it isn't one:
- `CSewSeg` has no trim type. PE-DESIGN's own Ver.6.0 stitch group stores the move as a jump
  segment (flag 1) between two stitch runs, as OpenStitch does. Both re-saves keep the trim as the
  PEC record's `0x20` flag, and the reference compare (stitches, jumps, trims) has no differences.
  PE-DESIGN greys out View Thread Trimming and Jump Stitch Trimming for this stitch data.
- PE-DESIGN's stitch group has no lead-in jump segment from the origin. OpenStitch no longer
  writes one, so its `CSewSeg` now matches PE-DESIGN's run for run (unit test
  `pes_v6_stitch_group_matches_pe_design_and_keeps_the_trim`).
- PE-DESIGN writes the v6 thread table's strings (catalogue number, name, brand, chart) as MFC
  Unicode strings: `FF FE FF`, a length in UTF-16 units, then UTF-16LE. The reader took `FF` as
  a 255-byte length and lost the whole object section of PE-DESIGN-written v6 files; it now reads
  both forms. OpenStitch keeps writing single-byte strings, which PE-DESIGN reads.
- Not changed: PE-DESIGN stores `CSewSeg` points in page coordinates (identity transform, bounds
  in the `CEmbOne` header) where OpenStitch stores them relative to the group with a translation,
  as pyembroidery does. It writes colour type 0 where OpenStitch writes `0x0A`, and `01` where
  OpenStitch writes `02` after the hoop field. PE-DESIGN opens OpenStitch's form unchanged.
