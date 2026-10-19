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
    `colorBlock` and an `align` anchor;
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
| `stitchedArea` | Area covered by 0° fill rows (row length × median spacing), mm² |
| `rowShift {stitchLength, interior}` | Row-to-row needle shift of a 0° fill (Fill Frequency) |
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

### Specs (2026-10-19)

The specs convert the audit's tests. The Rust tests (`satin_compat.rs`, `region_compat.rs`,
`pes_compat.rs`, `order_holes.rs`, the formats oracle tests) stay as they are.

| Audit item | Specs |
|---|---|
| 1 Satin zigzag | `satin-zigzag-rect`, `satin-zigzag-column`, `satin-zigzag-skeleton` |
| 2 Default PES v6 | `pes-default-v6`, `pes-library-convert-v6` |
| 3 Running Stitch Path | `region-travel-u-fill`, `-along-outline`, `-on-outline`, `-satin`, `-gradation`, `running-stitch-path-exclusions` |
| 7 Auto direction | `auto-satin-bent-band` |
| 14 Programmable fill 45° | `programmable-fill-45` |
| 15 Frequency and ranges | `fill-frequency-0`, `-25`, `-50`, `-default`, `attribute-ranges` |
| 16 Optimize entry/exit | `optimize-entry-exit` |
| 17 Hole sewing | `hole-sewing-pentagon-star`, `hole-sewing-limits` |
| 18 PES thread chart | `pes-thread-chart-roundtrip` (oracle) |
| 19 Design Property | `pes-design-properties` |

Local run, 2026-10-19: 23 specs, all passing. 21 are at the internal-regression tier and 2 at
the oracle tier. 42 checks are waiting for PE-DESIGN reference files or sew-outs.

Binary differences caused by non-functional metadata (labels, padding, timestamps) are listed
separately from functional differences: `compare` reports what is sewn, and `bytesIdentical` is
used only where the writer is deterministic.

## Oracle tier (independent implementation, not PE-DESIGN)

Fixtures produced by `tools/genfixtures.py` with pyembroidery 1.5.1: 41 stitches in two colour
blocks with a trim + jump and one stitch longer than 121 units.

| Test | Expected | Actual | Result |
|---|---|---|---|
| DST read matches reference decoder | 44 entries identical | identical | pass |
| DST write(read(f)) == f | byte-identical | byte-identical (after fixing `ST:` to count all move records) | pass |
| PES/PEC read matches reference decoder | identical stitches, threads `#ed171f`, `#0a55a3` | identical | pass |
| PES/PEC write(read(f)) == f | byte-identical incl. icons | byte-identical | pass |
| DST every delta −121…121 encodes/decodes | lossless | lossless | pass |
| PEC long-form deltas and > 2047 interpolation | positions preserved, trim kept | preserved | pass |
| EXP read / write(read(f)) == f | identical / byte-identical | identical | pass |
| JEF read / write(read(f)) == f (date fixed) | identical, threads `#ff0000` `#0b2f84` / byte-identical | identical | pass |
| JEF stop encoded as colour 0 and read back | 1 stop, 0 colour changes | as expected | pass |
| DST trim as 2, 3, 5 jumps; read with same count | 1 trim + 1 jump | as expected | pass |
| XXX read / write(read(f)) == f | identical, threads `#e01010` `#1040c0` / byte-identical | identical | pass |
| XXX end offset at 0xFC points at `7F 7F 02 14` | yes | yes | pass |
| PCS read matches reference decoder (fixture assembled, see FILE_FORMATS.md) | identical, exact RGB threads | identical | pass |
| PCS write(read(f)) == f; 1/6 mm grid survives a round trip | byte-identical | byte-identical | pass |
| SEW read matches reference decoder (fixture assembled) | identical, chart threads `#ff0000` `#386cae` | identical | pass |
| SEW write(read(f)) == f | byte-identical | byte-identical | pass |
| PES v1, v6, v6 truncated (with `CEmbOne`/`CSewSeg`, thread table) read matches reference | identical; v6 threads `#e01010` `#1040c0` (exact RGB) | identical | pass |
| PES v1, v6, v6t write(read(f)) == f | byte-identical | byte-identical | pass |
| PES v6 with thread chart, brand, catalogue and description (`oracle_v6_chart.pes`): write(read(f)) == f | byte-identical (chart strings and colour type kept) | byte-identical | pass |
| Default PES (`FileType::Pes.write`) == pyembroidery `6` | byte-identical to `oracle_v6.pes` | byte-identical | pass |
| HUS read matches reference decoder (fixture assembled, literal-only streams) | identical, chart threads `#ff0000` `#0000e7` | identical | pass |
| HUS written by OpenStitch (LZ77 + Huffman) read by the reference decoder | same stitches and threads as the original | identical (`openstitch_hus_read.json`) | pass |
| SHV read matches reference decoder (fixture assembled) | identical, chart threads `#ff0000` `#0000ff`, name `oracle` | identical | pass |
| PHC read matches reference decoder (fixture assembled around `oracle.pec`) | identical, PEC chart threads | identical | pass |
| CSD (spec-derived): synthetic type-0 and type-1 files | 303 stitches, 1 colour change, 1 trim, palette colours | as expected | pass (self-consistency only) |
| VIP written by OpenStitch read by libembroidery `sew` (built locally, commit `1138e04`) | 44 records, colours `224,16,16` and `16,64,192` | as expected; stitch coordinates not checkable (stub decompressor) | partial |

PCS, SEW and HUS have no reference writer in pyembroidery 1.5.1. For them the read is the oracle
check; the byte-identity test only confirms the writer matches the layout the reader expects.

Implementation under test: the crate `crates/openstitch-formats`.

Run: `cargo test -p openstitch-formats` (39 format tests + 10 malformed-input tests, all
passing on 2026-10-18; every fixture round-trips byte-identically). Golden engine regressions:
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

None yet. It needs a licensed PE-DESIGN installation (ROADMAP.md, "Blocked").

16 specs are ready for reference files, and 2 of them also for a sew-out. Each has a
`references[].make` instruction, and `target/compat/summary.md` lists the files they wait for.
Add them as [compat/reference/README.md](../compat/reference/README.md) describes. Observations
that are not files go here. Example: a PES that PE-DESIGN opens blank.
