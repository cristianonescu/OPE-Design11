# Handover: pe-gui (PE-DESIGN 11.50 reference files), 2026-10-27

Landed: PR #37 (optimize-entry-exit: PE colour order, same-colour linking), #41 (pes-default-v6 decoupled from fill),
#51 (shape-start-points: Shape tools start/end at bottom centre). In flight: nothing. All posted reference requests served.

## Where the references are (all under <reference-notes>, outside the repo)
| Set | Folder | MANIFEST |
|---|---|---|
| round 2 misc (optimize exp1-7, satin, shape-start, v6 re-saves) | round2\ | entries were put in the requesting tracks' worktrees / PRs |
| fill/satin sweeps S1-S8 (+h33) | round2\sweeps\ (+ log.txt) | MANIFEST-sweeps.json (38; copy in stitch-sweeps worktree, PR #59) |
| text T1-T6 (T4 presets 1-26 on the page) | round2\text\ | MANIFEST-text.json (43) |
| lines defaults, STARTEND, mid-link chain | round2\lines\ | MANIFEST-lines.json, MANIFEST-lines-startend.json (12), -pes-default-v6, -pes-csewseg-trims |
| pes-objects r01-r26 (+ r01-v6) | pes-objects\ | MANIFEST-pes-objects.json (27) |
| machine formats (10 exports) | machine-formats\ | MANIFEST-machine.json (10) |
| CSewSeg trims re-saves | pes-io-trims\ | in pes-io's PR |
Every folder has a log.txt or notes with each step. Screenshots never go in the repo.

## Procedure as actually done
- PE-DESIGN closes by itself every hour or so: reopen it (computer-use `open_application`, embedit.exe). After a restart the
  Shape and Text tools are back at PE defaults; otherwise tools REMEMBER the last attributes (type, colour, values) ->
  reset with house buttons / Color pane before each file and check the panes.
- File > New keeps the last page (100 x 100 mm, grid 1 mm, snap on). Page (0,0) = screen (420,171), 5 px/mm at 107 %.
- Save As: the type list keeps the last choice; reselect "Layout & Editing Ver.11.0 (*.PES)" (#PES0110) every time.
  Ver.6.0 asks "save as stitch data?" -> OK. Typed file names: no newline; press Return as a separate key.
- Anything off the page: Save As warns "Please change the object size or position" and, if saved anyway, the file has
  NO PEC block (old T4 files). Answer No and move the object.
- Text: the click is the BASELINE-left of the text. Text defaults: line Not Sewn, region Satin 800 RED, size 10, Satin and
  Fill density 5.0, under sewing OFF. Font: repo's Lato-Bold.ttf installed per-user (owner-approved); PE lists "Lato".
- Rectangle defaults: line Zigzag 800 RED (w 2.0, d 4.5, under sewing on), region Fill 005 SILVER (4.5, 45 deg, under
  sewing Medium, step 4.0, freq 30). Fill default direction is 45 deg.
- Drop-down lists ignore plain clicks: hover the item, then click; the Color list needs scrolling for codes > 330.
  Mouse wheel over the attribute pane changes spinner values: scroll only lists.
- Entry/exit points (Select > Select Entry/Exit Points): they move only along the outline; drag in small steps;
  L2 will not move onto the edge segment between a corner and L1, nor up the next edge: it stops at the corner.
  The labels shown can be swapped vs behaviour (lines-analysis.md "CORRECTION").
- Design Property OK is disabled on an empty design (draw one shape first).
- Export (App menu > Export...) is the only route to non-PES formats; no options dialogs; it starts in PE's program
  folder (never save there; never touch PE's folders, registry or licence).
- Count check: PEC record count = PE stitch count - 1 (scratchpad pec.py decoder, PEC offset at header bytes 8..11).

## Outstanding
- T5 second part (user-mapped text) not done: needs Brother font tools / built-in content (not allowed).
- text-fit-halign-stretch has no PEC (stretched text cannot fit the page); not redone.
- Text-track worktree was never found; text files stay in round2\text for the next owner to copy.
