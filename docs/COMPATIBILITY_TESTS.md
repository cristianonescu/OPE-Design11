# Compatibility tests

## Method

1. Define a controlled input (a design or a parameter set).
2. Produce the reference output in PE-DESIGN 11 (licensed copy, isolated VM); store it under
   `reference/` (git-ignored; owner's files) with a sidecar JSON of the parameters used.
3. Produce the same output with OpenStitch.
4. Compare structurally: stitch count, coordinates, sequence, colour changes, jumps, trims,
   bounds, thread assignment; then bytes. Tolerances only where justified and written here.
5. Record the result in the table below and in FEATURE_MATRIX.md.

Binary differences caused by non-functional metadata (labels, padding, timestamps) are listed
separately from functional differences.

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

None yet. Needs a licensed PE-DESIGN installation (ROADMAP.md, "Blocked").
