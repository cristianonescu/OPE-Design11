# Machine equivalence: PEC streams, OpenStitch vs PE-DESIGN 11.50 (machine-equiv, 2026-10-27)

Goal 3: an OpenStitch export sews on a Brother machine as PE-DESIGN's export of the same design. The machine reads the
PEC section (and the other machine formats), not the editable objects, so this compares those streams.

Tool: `pes-compare/peccmp.py` (this folder). Decodes the PEC section from documented fields only (label, colour count,
chart codes, stitch block width/height, every stitch record with its jump/trim flag, colour-change records and their
bytes, icon presence) and diffs two files: counts, first/last records, longest stitch and jump, what surrounds each
colour change, the first divergent record. `peccmp.py --batch target/compat/work compat/reference --json out.json`
runs every spec pair; `--dump file.pes N` prints one file. Results of the run on main 031b963:
`pes-compare/peccmp-results.json`.

Sample: 40 OpenStitch exports vs their 31 PE references, plus the 177 PE-DESIGN 11.50 PES files available (compat/reference
and <reference-notes> round2: lines, text, sweeps, satin) for PE's own rules.

## PE-DESIGN 11.50's machine stream, as observed (177 files)

1. First record: always a plain jump (flag 0x10) from (0,0) to the first needle point, then stitches. Never a zero-length
   stitch after it.
2. Interior frame moves: ALL carry the trim flag (0x20). Zero plain interior jumps in 177 files. Shortest trimmed move
   3.02 mm (text-size-hg). Travel shorter than that is sewn across (e.g. Feed-type motif lines have no jump records at
   all: line-motif-default.pes, 188 records, 0 jumps, 0 trims). Jump Stitch Trimming was ON (owner's default, notes.md).
3. Colour change: `FE B0 02`, `FE B0 01`, alternating from 02, written after the last stitch of the block, followed by a
   plain jump to the next block's first needle point. No trim record before it (the machine cuts at a colour change).
4. End: the last record is a stitch; no trailing trim or jump. `FF` then the icons.
5. Zero-length stitch records: none (the only file with them, text-fit-halign-stretch.pes, is the off-page file whose
   PEC is clipped at -2047; excluded).
6. Chart codes: one PEC chart index per colour block, the thread PE shows (800 RED -> 5, 900 BLACK -> 20, ...).
7. Stitch block header width/height: 2900 x 1900 (0.1 mm) in every native file = the hoop section size (Design Settings
   "290 x 190 mm"), not the design's extent. In PE's re-saves of OpenStitch files it keeps OpenStitch's value (400 x 300 for
   pes-default-v6), so PE reads this field as the hoop size and preserves it. The next two fields are 0x1E0 / 0x1B0 as
   everywhere.
8. Label: the first 8 characters of the FILE name at save time, whatever the Design Property name (pes-design-properties.pes:
   name "Rose", label "pes-desi"; the re-save of OpenStitch's a.pes: label "a-resave").
9. Stitch length: PES keeps satin stitches whole up to at least 15 mm (sweep-satin-width-15: no split); fill stitches
   4.0-4.5 mm. Longest in the references 7.4 mm (auto-satin-bent-band).
10. Trim encoding: the move itself carries the 0x20 flag in both bytes (one record); PE never writes a zero-length trim
    record followed by a jump.

## Systematic differences found, ranked by sewing effect

| # | Difference | Effect on the machine | Status |
|---|---|---|---|
| 1 | OpenStitch left frame moves of <= 3.0 mm between elements as plain jumps (text-fit-halign-left: a 1.49 mm jump between letters; text-fit-otherside: 1.71 mm). PE trims every interior move. | A loose thread span on top of the fabric where PE cuts. | FIXED: `Compiler::trim_distance` default 30 -> 0 (every interior jump is trimmed; multi-needle keeps PE's minimum-length setting). |
| 2 | Zero-length stitch records (ours 1-27 per file, PE 0): generators emit the same point twice (run end = next run start, duplicate row points). | A second needle penetration in the same hole; extra stitch count. | FIXED for repeats of a needle point just sewn (compile.rs `Writer::push`). A zero-length stitch right after a jump is KEPT on purpose: it is the block's first penetration (the entry point); PE ties in there with a 0.3 mm stitch forward and back instead. That tie-in belongs to the generators (lines already have it since #55; fill/satin: stitch-sweeps' fill-row-structure, satin's lock follow-up). |
| 3 | PEC label: OpenStitch wrote the design name ("Untitled" for every compat export); PE writes the file name. | What the machine lists/displays for the design. | FIXED: `design.export` sets `pec.label` from the output file stem; the PEC writer prefers it over the name (PES Design Property name unchanged). |
| 4 | Trim counts in hole-sewing-pentagon-star: ours 2 interior trims, PE 0 (PE sews the star and the pentagon's hole travel without leaving the fabric: 3 jump runs vs our 5). | Two cuts PE does not make: thread tails on the back, time. | Generator-level (fill's region travel / hole sewing path), not the writer. -> fill track. |
| 5 | Stitch block width/height: ours = design extent (pyembroidery's rule, what PE preserves on re-save); PE native = hoop section size 2900 x 1900. | None observed on a machine (the extent is read from the stitches; Ink/Stitch files use the same rule). Possibly what a machine shows as the design's hoop. | NOT changed: OpenStitch's pattern carries no hoop; recorded as a known cosmetic difference. Revisit if a sew-out shows a hoop warning. |
| 6 | Chart codes differ in shape-start-points ([5,2,56,...] vs PE [5,1,61,...]) | Different thread names shown on the machine. | Not a writer difference: the spec script uses hex colours while pe-gui picked PE chart threads "each in a different colour". Our nearest-chart mapping is exact for Brother chart colours (Red 5, Black 20 match PE in every file). |
| 7 | Longest stitch: ours 5.0 mm in fills (PE 4.2), 9.2 mm in text (PE 3.8-4.3), 6.5 in hole sewing (PE 4.7) | Longer stitches than PE in fills and text. | Generator-level: fill row-end/travel stitches (stitch-sweeps), text (text track). The compiler's 12 mm split (max_stitch) never triggers in these files; satin stays whole as PE. |
| 8 | Stitch counts and positions (fill 5-16 % more stitches, text +29 %, etc.) | Covered by the stitch tracks' specs. | Out of scope here; see the coordinator's pes-compare summary in TRACKS.md. |

Everything else in the stream matches PE record for record: the structure around colour changes (stitch, FE B0 xx, jump,
stitch), the alternating 02/01 bytes, the first jump, the stitch-ending tail with no closing trim, the trim encoding (the
move carries the flag), the 7-bit/12-bit encoding, the icon count, the 0x1E0/0x1B0 fields. PE re-saves of OpenStitch files
(pes-csewseg-trims, pes-default-v6) are record-identical apart from the label and PE's re-placement of the design at the
page origin.

## Other machine formats (DST, EXP, JEF, PCS, SEW, XXX, HUS, VIP)

pe-gui delivered PE's exports in all 10 formats on 2026-10-27 23:23 (see handover-machine-equiv.md for the first read-only pass: PE appears to split 15 mm stitches as jump + stitch; HUS magic 0x00C8FC5D; DST/EXP colour change not read). Request was: PE exports of OpenStitch's
`<reference-notes>\machine-formats\a.pes` (two Red running lines with a trim, a Black satin 40 x 15 mm, so
15 mm stitches must be split in DST/EXP/JEF/SEW/XXX). Spec `compat/specs/machine/machine-formats.json` exports every
format and compares each with its reference once the files arrive. Open questions the files answer: DST jumps-for-trim
count (OpenStitch default 3), DST label, EXP trim record, JEF/SEW trim handling (OpenStitch JEF drops trims by default),
colour-stop records, PCS version/hoop bytes, and the per-format split of long stitches.

## After the fixes (same comparer, branch claude/machine-equiv-41a470)

40/40 specs pass (32 at the reference tier). Stream differences left per pair: the PEC width/height field (#5), trims that
come from region routing (#4: hole-sewing 2 vs 0; text-fit-halign-spaced / otherside 7 vs 5; text-size-hg 2 vs 4), the
kept entry-point zero-length stitch (1-3 per file in fills and text, #2), and the stitch geometry itself. No plain
interior jump remains in any OpenStitch export. Labels now equal the export file's stem.

## Sew-out package (for the owner)

<reference-notes>\sewout\: README.md (what to sew, in what order, what to look at), the four PES files,
sha256.txt, and records/<spec>.json templates with the SHA-256 filled in. Specs with a `machine` block: pes-default-v6,
machine-formats, satin-zigzag-rect, shape-start-points (block added). A record counts only while the SHA matches the
current export, so the files are re-exported and re-hashed after each rebase; the README says so. No machine-tier pass is
claimed anywhere.
