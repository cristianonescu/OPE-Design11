# Handover: machine-equiv track (goal 3, "same on the machine"), 2026-10-27

## What landed
- PR #69 (branch `claude/machine-equiv-41a470`, worktree `<worktree>`),
  OPEN, rebased on ffcade9; local fmt / clippy -D warnings / `cargo test --workspace --locked` / compat 49/49 green. Main moved
  two commits after the push, so it conflicts on docs/COMPATIBILITY_STATUS.md (regenerate) and possibly the badge goldens
  (re-record: engine `badge-golden.pes` label `badge-1t`; design `golden_badge.rs` stitch count 2937). Rebase when pinged.
  Contents: `Compiler::trim_distance` 3 mm -> 0 (every interior jump trimmed on single-needle; multi-needle
  `MachineSettings::DEFAULT_MIN_JUMP_TRIM` 3 mm when unset); `Writer::push` drops a stitch repeating the needle point just
  sewn (kept right after a jump: the entry penetration); `design.export` sets metadata `pec.label` = file stem, the PEC writer
  prefers it; spec `compat/specs/machine/machine-formats.json`; `machine` blocks on machine-formats and shape-start-points;
  docs: COMPATIBILITY_AUDIT item 4, FILE_FORMATS label note, STATUS.

## Findings (machine-equiv.md, ranked by sewing effect)
Measured with `pes-compare/peccmp.py` (PEC decoder + record diff; `--batch target/compat/work compat/reference`) on 40 exports
vs 31 PE references and 177 PE-DESIGN 11.50 PES files (repo + PE-reference-notes round2).
1. PE trims EVERY interior frame move (0 plain interior jumps in 177 files; shortest trim 3.0 mm; plain jumps only first
   record and after a colour change). OpenStitch left <= 3 mm moves plain. FIXED (#69).
2. Zero-length stitch records: PE 0, OpenStitch 1-27 per file. FIXED for repeats of a sewn point; the entry-point one after a
   jump stays until fill/satin tie in like PE (0.3 mm forward and back; lines already do since #55). Owner: stitch-sweeps
   (fill-row-structure) / satin.
3. PEC label = file name in PE. FIXED.
4. Extra trims from region routing: hole-sewing-pentagon-star 2 vs 0; text-fit-halign-spaced and -otherside 7 vs 5;
   text-size-hg 2 vs 4. Generator-level (fill travel, text letter joins). Owner: fill / text.
5. PEC stitch-block width/height: PE native 2900 x 1900 (hoop section), OpenStitch design extent (PE preserves it on re-save).
   Cosmetic; NOT changed; revisit only if a sew-out shows a hoop warning.
6. Longest stitch: fills 5.0 vs PE 4.2 mm, text 9.2 vs ~4 mm. Generator-level (stitch-sweeps, text).
Everything else matches record for record (first jump, FE B0 02/01 alternation, jump after a colour change, trim flag on the
move, no closing trim, icon count, 0x1E0/0x1B0, 7/12-bit encoding). Chart codes match for Brother chart colours.

## Other-format comparison (DST, EXP, JEF, PCS, SEW, XXX, HUS, VIP, + SHV, CSD)
- pe-gui DONE (2026-10-27 23:23): all 10 formats PE offers (App menu > Export; order DST, EXP, PCS, HUS, VIP, SHV, JEF, SEW,
  CSD, XXX; no PEC/PHC; no options dialog, no warnings) are in `<reference-notes>\machine-formats  machine-formats.<EXT>` (upper-case extensions) with MANIFEST-machine.json (paths machine/machine-formats.<ext>,
  reference pe-design-<ext>; SHAs in TRACKS.md). Source a.pes f0ee99e7..., opened as stitches, nothing changed.
- NOT yet compared at record level (wrap-up). A first READ-ONLY pass through `stitches.import` + `design.info` (which
  RE-SEWS the file, so its stitch counts include the compiler's 12 mm split and its trims come from jump inference; treat as
  hints only) showed:
  - DST, JEF, SEW, VIP, XXX: 248 stitches and 183 jumps read back, vs 248 stitches / 3 jumps in a.pes: PE seems to write each
    15 mm satin stitch as a jump plus a stitch (one penetration, two frame moves) rather than two stitches. Verify on the raw
    records first (write a DST/EXP/JEF record dumper like peccmp.py); OpenStitch splits into two stitches (`interpolating_long_moves`).
  - DST and EXP read with 0 colour changes (PE's colour-change record not recognised, or absent in EXP: check bytes 0xC3 in
    DST and `80 01` in EXP). PCS reads 2 colours as #000080/#0000ff (PE's PCS colour table or our reader).
  - HUS: OpenStitch's reader rejects PE's file: magic 0x00C8FC5D, ours expects 0x00C8AF5B (libembroidery's); the VIP magic
    matched and read fine. Fix the HUS reader/writer magic after checking the stream layout.
  - CSD reads as 0 stitches (reader gap or a different CSD variant). SHV reads 63 stitches / 10 jumps (SHV is read-only).
  - XXX reads 245 stitches (3 fewer): check the end records.
- Next: copy the files to `compat/reference/machine/machine-formats.<ext>` (lower-case names as the spec expects), add the
  MANIFEST entries, run `compat --only machine-formats`, then fix writers per confirmed rule (DST jumps-for-trim,
  JEF/SEW trims, long-stitch split as jump+stitch if confirmed, HUS magic, colour stops), one PR, oracle fixtures kept green.

## Sew-out package (owner)
`<reference-notes>\sewout\`: README.md (order, what to check), pes-default-v6.pes, machine-formats.pes,
satin-zigzag-rect.pes, shape-start-points.pes, sha256.txt, records/<spec>.json (filled SHA-256, placeholders for machine,
result, observations). Owner: sew the four files as README says, photograph front and back, fill the records, commit each at
the path the README lists under compat/machine/. Hashes are valid for #69's tree; re-export (`compat`) and re-hash after
any stitch change, or the record will not count. No machine-tier pass has been claimed.

## Next steps, in order
1. Rebase #69 when pinged (STATUS, goldens); merge when CI is green.
2. Other-format record-level comparison (exports exist) -> writer fixes PR (one per coherent group), specs at reference tier.
3. Owner sew-outs -> records -> machine tier for the four specs.
4. Entry tie-in for fill/satin (removes the last zero-length records): coordinate with stitch-sweeps / satin.

## Files and references
- Notes folder: machine-equiv.md (findings), pes-compare/peccmp.py, pes-compare/peccmp-results.json (last run).
- Repo (branch): compile.rs, machine.rs, pec.rs, commands.rs (`design.export`), compat/specs/machine/machine-formats.json.
- Evidence: compat/reference/**/*.pes (31), <reference-notes>\round2\{lines,text,sweeps}\*.pes,
  pes-csewseg-trims.pes (PE keeps OpenStitch's trim on re-save).
