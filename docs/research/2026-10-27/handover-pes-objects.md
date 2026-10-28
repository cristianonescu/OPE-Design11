# Handover: pes-objects (pes-io session), 2026-10-27

Goal 2: editable PES round trip with PE-DESIGN 11.50. Method: black-box diffs of the owner's PE-DESIGN
output files only (policy #60); never PE-DESIGN's program files.

## Landed
- #39 Design Property Windows-1252 + CR LF; no zero-length landing stitch.
- #53 CSewSeg trims (not a difference); no lead-in jump segment; MFC Unicode thread strings read.
- #60 LEGAL_AND_LICENSING: output-file study allowed as owner policy.
- #62 PES/PEC with no PEC section (`LA:` missing) is an error.
- #68 `formats/src/pes11.rs` reads `CEmbLine` shapes; FILE_FORMATS.md "PES 0110 object list";
  fixtures `formats/tests/fixtures/pe11` (r01–r16 + PROVENANCE.md); tests `tests/pes11.rs`.
- PR #71 (pushed, not merged; branch `feat/pes-0110-colours`): thread table,
  line colour (+46), region colour (+170), zigzag width/density (+48/+50), run pitch (+52), line
  under sewing (+60), fixtures r17–r22, MFC map numbering fix.

## Mapping (facts; details and evidence in FILE_FORMATS.md)
- Editable objects only in `#PES0110` (Ver.6.0 saves stitch data). No stitches in objects.
- Object list = MFC CArchive: u16 count; new class `FFFF`, schema, name; repeat `0x8000|n`, n = entry
  in ONE map counting classes and objects from 1 (r14: CEmbNText 1, text 2, CTTFLetter 3, A 4, B = `03 80`).
- Coordinates: 0.1 mm, page top-left at (500,500), y down (all files on a 100 mm page; untested for
  other page sizes).
- Thread table right before the list: catalogue, RGB+pad, u32 kind, PEC index string, brand, chart;
  sewing order; may hold unused colours (body +162 always points at 800 RED: unknown attribute).
- `CEmbLine` body: 4432 bytes, then 84-byte geometry header (3 marks: likely entry/direction/exit
  handles, unmapped), u32 n, n × 28-byte vertices (6 f32 + u32 kind 0/0x11). Fields mapped: see table.
- Classes seen, unmapped: `CEmbNText` + `CTTFLetter` (text), `CEmbSpecialShape` (gallery shapes),
  Manual Punch (`CEmbLine` variant; reader refuses), the 52+-byte trailer, E vs V, run times.

## Text so far (r12–r14, r23–r26, files in <reference-notes>\pes-objects)
- `CEmbNText` body: bounds/box/transform as CEmbLine; u16 letter count at +41 (o-relative +51).
- `CTTFLetter` body: +0 u16 char code; f32 100.0 then f32 scale (1.0 size 10, 2.0 size 20); +28
  region sew type (Satin 0 → Fill 1, r23); f32 x,y origin = the click (baseline-left), body +270
  (r26); font name MFC string (`Lato`); same sewing-attribute block layout as CEmbLine. No glyph
  outlines: PE re-renders from the font (OpenStitch bundles Lato Bold).
- r23–r26 not yet committed as fixtures.

## Next steps
1. Text reader: walk CEmbNText + CTTFLetter (map numbering now right), letter fields above; fixtures
   r12–r14, r23–r26; tests.
2. Engine import: `stitches.import`/open of a #PES0110 file → editable OpenStitch shapes (contours from
   the Bézier vertices, LineSewing/RegionSewing from the fields, colours from the table) and text
   (Lato, size, origin); compat spec "OpenStitch read of PE's file gives the same design".
3. Writer for #PES0110: needs the rest of the 4432-byte body (mostly PE defaults; diff more variants,
   e.g. satin/fill parameter sweeps) and the trailer; spec: PE re-save of OpenStitch's editable PES keeps
   its objects (pe-gui makes the re-save).

## Tools
In the notes folder, `pes-objects-tools/`: `pesdiff.py` (aligned pairwise diff), `pesobj.py` (prototype walker),
`fields.py` (run with `python -I`).
