# Handover: Line Start/End Type (Feed/Run), lines-startend track

**Status (2026-10-27):** [#70](https://github.com/cristianonescu/OpenStitch/pull/70) is a draft. It is on branch `lines/startend-feed-run`, in worktree `.claude/worktrees/lines-feedrun`, at commit 50c5eee. It is stacked on #63's commit 088a3bb and queued after #69. Local gate is green: fmt, clippy -D warnings, cargo test --workspace --locked, compat --status.

## Rule as implemented
Closed stem, candlewick and triangle-chain lines with a moved entry point follow it; other line types, open lines and chain diamonds don't. Evidence: 10 PE-DESIGN 11.50 files; all match PE's needle points in order (chain 0.00 mm, stem and candlewick within 0.10 mm).
- **Elements stay in place.** Every link, station and knot is where it would be with no moved points; only where the sewing starts and ends changes. No jumps or trims.
- **Which element a point belongs to:** the last element that begins at or before it, in sewing order.
  - Chain link: begins at its point on the line.
  - Stem station: begins at its centre.
  - Candlewick knot: begins one radius before its centre.
- **Feed:** the pattern starts with L2's element (the exit point) and goes round to the element before it.
  - A stem sews that station again at the end.
  - A candlewick sews that knot last (it starts with the knot's running stitch).
- **Run:** starts at L1's element and sews only the leads to L2's element, the shorter way round (backwards if need be), then continues as Feed.
  - Chain leads: the link points, one straight stitch each.
  - Stem leads: the station centres.
  - Candlewick leads: its running stitch.
- **Tie-in:** stem and candlewick Run (and candlewick Feed) start with running stitch and use the one-step tie-in; all others keep the long 0.3/0.6/0.9/0.5 tie-in. The lock is unchanged.
- **Closing corner** (the outline's first vertex):
  - Stem: sews the corner point, then the last leg's end-station bottom point.
  - Candlewick: sews one knot there, facing along the last leg. The next running stitch starts at the corner, and a short first stitch is split in two equal halves (1.6 + 1.6 mm).
- **Labels:** the first 8 reference files' L1/L2 screen labels were swapped. The specs use the corrected points; see lines-analysis.md, "CORRECTION".

## Files
- `crates/openstitch-design/src/lines.rs`
  - `stem_pieces`, `candlewick_pieces`, `chain_pieces`; the in-place output of the existing generators is unchanged.
  - `start_loop`, `LoopLine`, `arc_of`.
- `crates/openstitch-design/src/compile.rs`, `line_elements`: the `in_place` check, the call to `start_loop`, and the `lead_in` flag that picks the tie-in.
- `crates/openstitch-design/src/model.rs`: `LineSewing.exit` and `start_end`, plus the `StartEnd` enum (Feed is the default).
- Engine:
  - `commands.rs`: `object.setLine` `startEnd`. Switching the line type keeps L1/L2 and resets Start/End to Feed.
  - `drawing.rs`: `object.setEntryExit` `lineExit`.
- Tests: `stitch_type_tests.rs` `moved_points_*` (2 tests).
- Specs: `compat/specs/stitches/line-{chain,stem,candlewick}-startend-{feed,run}.json` and `line-chain-startend-{mid,l2side}-{feed,run}.json`.
  - Reference files: `compat/reference/stitches/`; their 10 MANIFEST entries are copied verbatim.
  - Sources: `<reference-notes>\round2\lines` (MANIFEST-lines-startend.json, log.txt).
- Docs: COMPATIBILITY_AUDIT.md "Next 11" (Start/End Type item with a "Still open" list), FEATURE_MATRIX.md, COMPATIBILITY_STATUS.md.
- Notes: lines-analysis.md (both Start/End sections, including the before/after table).

## Open items
1. **Motif lines.** PE's default motif001 is PE's own artwork, so no stitch comparison is possible. The first-look notes say Feed and Run differ there only in the tie-in. Needs a pe-gui reference with a simple motif drawn in PE.
2. **Open lines and chain diamonds.** Not measured; they still go through `Contour::starting_at`.
3. **The editor.** No L2 marker drag and no Feed/Run radio in the Sewing Attributes pane; the engine supports both.

## Next steps
1. When the coordinator says #63 has merged (squash), rebase #70 onto main from the worktree:
   ```
   git fetch && git rebase --onto origin/main 088a3bb lines/startend-feed-run
   ```
2. Regenerate the compat status: `cargo run -p stitchtool -- compat --status docs/COMPATIBILITY_STATUS.md`.
3. Rerun fmt, clippy and `cargo test --workspace --locked`.
4. Push with `--force-with-lease`, mark #70 ready, and merge once every check is green:
   ```
   gh pr merge 70 --squash --admin --delete-branch --repo cristianonescu/OpenStitch
   ```
5. Delete the worktree `lines-feedrun`.
6. For the open items: open the analysis with pe-compare/pec_dump.py and align PE's file on OpenStitch's output by bounding box, not median (a median slips by one element). Compare needle points in order.
