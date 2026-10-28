# Patent check: facts to verify before the related backlog items ship

Research date: 2026-10-28. This is not legal advice and draws no legal conclusions. It lists,
for each patent named in [competitive-features.md](../competitive-features.md) section 5 (and
the ones found while checking them), what Google Patents shows: holder, dates, status, what
the first claim covers in plain words, which OpenStitch item it touches, and a design-around
in plain words. Statuses and expiry dates are what Google Patents displays and labels as
assumptions; the owner's lawyer should confirm each against the USPTO, EPO and WIPO
registers before relying on it. Claim gists are paraphrases, not claim text.

## Summary

| Patent | Holder | Priority | Status shown | Touches | Reading |
|---|---|---|---|---|---|
| US 8,532,810 B2 | Vistaprint (Cimpress) | 1998-08-17 | Expired (fee related), term ended 2018 | auto-digitizing (Auto Punch), B4 routing | expired; the survey had mis-described it as a skeleton-satin patent (corrected below) |
| US 7,996,103 B2 | Brother Industries | 2007-11-26 | **Active** until 2030-03-27 | F16 direction-aware photo stitch (deferred); B4's "travel over sewn lines" rule needs a look | the one live patent on the list |
| US 5,541,847 A | Pulse Microsystems | 1994-10-14 | Expired (lifetime) 2014 | B1 contour fill | expired |
| US 6,934,599 B2 | VSM Group (now Singer Sourcing) | 2002-04-22 | Expired (lifetime) 2023 | D12 TrueType to satin, Font Creator | expired |
| US 6,587,745 B1 | Wilcom | 1998-09-07 | Expired (fee related), lapsed 2007 | B2 guided fills, today's direction curve | expired |
| WO 2005/061774 A1 | Wilcom International | 2003-12-22 | Ceased; no US or EP entry | B2 stitch-angle maps | never granted anywhere shown |
| US 6,600,966 B1 | Brian D. Bailie (Embrilliance) | 2002-02-26 | Expired (lifetime) 2022 | W1 multi-position split, today's splitter | expired |
| US 9,492,937 B2 and US 10,023,982 B2 | BriTon Leap (Embrilliance) | 2015-07-29 | **Active** | O7 appliqué cut files from stitch data | live; see the design-around |
| US 10,358,753 B2 and US 11,060,220 B2 | BriTon Leap (Embrilliance) | 2014-07-30 | **Active** until about 2035-07-29 | B14 segmentation, `object.toShapes` (bitmap tracing of stitch data) | live; see the design-around |

So three live families matter: Brother's US 7,996,103 and the two BriTon Leap families.
Everything else on the list is shown as expired or ceased.

## Per patent

### US 8,532,810 B2 — "Automatically generating embroidery designs"

- Holder: Vistaprint Technologies, now Cimpress. Inventor David A. Goldman. Priority
  1998-08-17; filed 2012-06-06 as a continuation; granted 2013-09-10.
- Status shown: Expired (fee related); anticipated expiration 2018-08-17 (the term runs from
  the 1998 priority). The whole continuation chain (US 6,836,695; 6,804,573; 7,587,256;
  8,219,238; 9,200,397) is shown expired; no later continuation is listed.
- Claim 1 in plain words: segment an image into objects; recursively split an object into
  fragments, recording for each fragment how it was created (its link to the parent at a
  start edge or scan line); order the fragments to reduce thread cuts or improve path
  connectivity; write an embroidery file from the fragments in that order. Claims 5 and 9
  are the same as an apparatus and a storage medium.
- Touches: the survey's section 5 listed it as "skeleton-based satin from strokes", which
  is wrong: it is about fragmenting auto-digitized regions and ordering them. It would have
  concerned Auto Punch and the B4 fragment ordering, and it is expired.
- Design-around: none needed on the status shown. OpenStitch's Auto Punch and `order.rs`
  order whole objects, not recursively generated fragments with parent links.
- URL: https://patents.google.com/patent/US8532810B2/en

### US 7,996,103 B2 — "Embroidery data generating apparatus …"

- Holder: Brother Industries. Inventor Kenji Yamada. Priority 2007-11-26 (JP 2007-304116);
  filed 2008-11-21; granted 2011-08-09.
- Status shown: **Active**, adjusted expiration 2030-03-27. No US continuations listed;
  JP 2009-125337 in the family.
- Claim 1 in plain words: an apparatus that computes, per pixel of an image, an angle
  feature (direction and strength), generates line-segment data and colours from it, decides
  the sewing order of the segments, and then decides whether the connecting move from one
  segment's end to the next segment's start is sewn as a running stitch or left as a jump,
  based on three tests: how much of that connector is covered by later segments, whether the
  summed colour difference with the segments under it is at or below a threshold, and the
  angle differences where it crosses other segments. Claim 5 is the medium.
- Touches: F16 (photo thread-painting), deferred, and specifically its direction-aware
  variant (per-pixel angle field with connectors decided by coverage and colour tests).
  B4's auto-routing decides connectors by graph structure (route inspection) and by a length
  threshold, not by per-pixel angle features or colour-difference tests, and does not start
  from an image; this is the fact for the lawyer to check against claim 1's combination.
- Design-around for F16 (facts to check): the claim combines (1) per-pixel angle features
  from an image, (2) segments from them, (3) the three-test connector decision. The Sfumato
  family of F16 uses density-modulated meanders per shade with no angle field and no
  per-connector colour test, which is a different construction; the colour-stipple family
  likewise. Avoid the direction-aware (Sobel field) variant until 2030 or until the lawyer
  reads the claim.
- URL: https://patents.google.com/patent/US7996103B2/en

### US 5,541,847 A — "Method for automatically generating a chenille filled embroidery stitch pattern"

- Holder: Pulse Microsystems. Priority 1994-10-14; granted 1996-07-30.
- Status shown: Expired (lifetime) 2014-10-14. Companion US 5,668,730 and US 5,771,173 also
  past term.
- Claim 1 in plain words: for a polygon, find the medial-axis spine from its Voronoi diagram,
  build concentric inward offsets of the polygon down to the spine at the fill density, order
  them to reduce jumps, then place spirals of varying size along them for chenille loops.
- Touches: B1 contour fill (concentric offsets). Expired; and B1 uses polygon offsetting
  without a Voronoi spine or chenille spirals.
- URL: https://patents.google.com/patent/US5541847A/en

### US 6,934,599 B2 — "Providing character data for use by an embroidery machine"

- Holder: VSM Group, now Singer Sourcing. Priority 2002-04-22 (GB 0209171.8); granted
  2005-08-23. Family GB 2387854 B.
- Status shown: Expired (lifetime), adjusted expiration 2023-06-09.
- Claim 1 in plain words: from font outline data, group the contours of each character,
  construct cut lines across the groups, divide them further, define embroidery objects from
  the pieces, and output each character's stitch data as a group a machine can select.
- Touches: D12 TrueType to satin, Font Creator. Expired.
- URL: https://patents.google.com/patent/US6934599B2/en

### US 6,587,745 B1 — "Curved line fill stitching in embroidery designs"

- Holder: Wilcom. Priority 1998-09-07 (AU PP5766); granted 2003-07-01. WO 2000/014319 ceased,
  EP 1112402 withdrawn, JP withdrawn.
- Status shown: Expired (fee related), lapsed 2007-07-01; the term would have ended
  2019-09-07.
- Claim 1 in plain words: the user gives an area, a fill type and a "stitch definition
  curve"; the fill lines run edge to edge and the cross-pattern lines both curve according to
  that curve rather than the area's shape.
- Touches: today's direction curve (`direction::Warp`) and B2's guided fills. Expired.
- URL: https://patents.google.com/patent/US6587745B1/en

### WO 2005/061774 A1 — "Turning complex fill stitching in embroidery designs"

- Applicant: Wilcom International. Priority 2003-12-22 (AU 2003907052); PCT filed
  2004-12-22; published 2005-07-07.
- Status shown: Ceased; no US or EP national or regional entry listed. A PCT application
  that enters no national phase grants no rights.
- Claim 1 in plain words: a complex fill shape and two or more stitch definition lines that
  divide it; the lines (and added cut lines) split the shape into simple sub-shapes; stitch
  points in each sub-shape follow the definition lines with offsets across sub-shapes for a
  smooth transition.
- Touches: B2 stitch-angle maps (strips between direction lines). Never granted on the
  record shown; the lawyer should confirm that no AU national patent exists (Google shows
  none).
- URL: https://patents.google.com/patent/WO2005061774A1/en

### US 6,600,966 B1 — "… dividing an embroidery machine design into multiple regional designs"

- Holder: Brian D. Bailie as an individual (Embrilliance's founder; the Embrilliance pages
  say "patented design splitting technology" without a number). Filed 2002-02-26; granted
  2003-07-29.
- Status shown: Expired (lifetime), anticipated expiration 2022-02-26. No continuations
  listed; cited by US 7,457,683 (adjustable embroidery design), not a continuation.
- Claim 1 in plain words: define two or more regions from the hoop size and/or its
  attachment points, overlay them on the design's stitches, and write a stitch file for a
  regional design; other independent claims cover per-stitch region assignment, region-first
  or colour-first ordering, media and PC-system forms.
- Touches: W1 (intelligent multi-hoop split) and today's split, multi-position and jumbo
  splitters. Expired.
- URL: https://patents.google.com/patent/US6600966B1/en

### US 9,492,937 B2 and US 10,023,982 B2 — "Automatic creation of applique cutting data from machine embroidery data"

- Holder: BriTon Leap, Inc. (Embrilliance). Filed 2015-07-29 and 2017-08-04; granted
  2016-11-15 and 2018-07-17. A third application (US 15/290,452) abandoned.
- Status shown: Active.
- Claim gist (from the titles and the family; the full claims were not read in this pass):
  deriving a cutting file for appliqué fabric from the stitch data of an embroidery design
  (the placement line's stitches become the cut path).
- Touches: O7's "cut files" item (SVG/PLT/FCM from the appliqué material outline), the
  SewWhat-Pro-style "Appliqué Cutter" that reads a placement colour from a stitch file.
- Facts for the design-around: OpenStitch's appliqué wizard works from **shapes** (the user's
  outline object), and a cut file written from the shape's outline does not start from
  "machine embroidery data". Writing a cut file from an imported stitch file's placement
  colour (the SewWhat-Pro feature) is the case to have the lawyer read against these
  claims; it is not in B1–B15.
- URLs: https://patents.google.com/patent/US9492937B2/en , https://patents.google.com/patent/US10023982B2/en

### US 10,358,753 B2 and US 11,060,220 B2 — "Sewing data for embroidery designs systems and methods"

- Holder: BriTon Leap, Inc. Priority 2014-07-30; filed 2018-06-21 and 2019-06-11
  (continuation); granted 2019-07-23 and 2021-07-13.
- Status shown: Active, anticipated expiration 2035-07-29.
- Claim 1 of US 10,358,753 in plain words: obtain sewing (stitch) data; draw the design into
  a bitmap; trace, simplify and inflate the bitmap's outline; generate outline stitch data
  that follows the traced outline; add it to the sewing data and store the result. Claim 11:
  similar with a Bézier-fitted, offset outline and fill-stitch generation; claim 17 the
  medium. The claims are about **tracing a bitmap rendering of stitches** to get an outline
  (or fill) and adding stitches along it. Hoop positioning is not claimed.
- Touches: `object.toShapes` today ("shapes traced from its stitch coverage", which draws
  the stitches and traces the covered area with `digitize::coverage::trace`); B12's
  knockdown wizard when applied to stitch blocks (it offsets a traced outline and fills it);
  B14's resize uses segmentation by stitch statistics (rails, rows) rather than bitmap
  tracing, but its fill case may fall back to the coverage trace.
- Facts for the design-around: the claims need the bitmap-draw-and-trace step. B14's
  classifier recovers rails and rows from the stitch geometry directly (no raster), and its
  fill regeneration can take the row spans from the stitches rather than from a traced
  outline, which keeps the raster step out. `object.toShapes`'s coverage trace is the
  existing feature the lawyer should read against claims 1 and 11 (the outline-stitch
  generation step in claim 1 is "add outline stitches following the traced outline"; the
  convert-to-shapes command produces editable shapes, not outline stitches, which is the
  difference to check). The knockdown wizard on stitch blocks could use the block's convex
  hull or the stitch-geometry segmentation instead of a trace.
- URLs: https://patents.google.com/patent/US10358753B2/en , https://patents.google.com/patent/US11060220B2/en

## What this changes in the backlog

- Section 5 of the main document is corrected: US 8,532,810 is not a skeleton-satin patent
  and is expired; the active concerns are Brother's US 7,996,103 (F16's direction-aware
  variant only) and the two BriTon Leap families (O7 cut files from stitch data; tracing a
  bitmap of stitch data in `object.toShapes`, B12 on stitch blocks and B14's fill fallback).
- B14's sketch already prefers geometric segmentation; its fill case should take row spans
  from the stitches, not from a traced outline, and the note's test `segment-generated-fill`
  checks that no raster is involved. B12's knockdown on stitch blocks should use the convex
  hull or the segmentation outline.
- D12 (TrueType to satin) and today's skeleton satin have no live patent on this list.
- Nothing here is a conclusion; it is the list for the owner's lawyer, with the claim
  elements to read in each case.

## Sources

- https://patents.google.com/patent/US8532810B2/en and https://patents.google.com/patent/US9200397B2/en
- https://patents.google.com/patent/US7996103B2/en
- https://patents.google.com/patent/US5541847A/en
- https://patents.google.com/patent/US6934599B2/en
- https://patents.google.com/patent/US6587745B1/en
- https://patents.google.com/patent/WO2005061774A1/en
- https://patents.google.com/patent/US6600966B1/en
- https://patents.google.com/patent/US10358753B2/en , https://patents.google.com/patent/US11060220B2/en
- https://patents.google.com/patent/US9492937B2/en , https://patents.google.com/patent/US10023982B2/en
- https://ipqwery.com/ipowner/en/owner/ip/193055-briton-leap-inc.html (BriTon Leap portfolio listing)
