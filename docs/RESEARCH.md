# Research log

OpenStitch specifies PE-DESIGN 11's behaviour from public sources only: Brother's public online
user's guide, published and open-source descriptions of embroidery file formats, and
independent tests. See [LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md) for the boundaries.

## 2026-10-18: Machine formats (public sources)

Sources: pyembroidery 1.5.1 readers and writers (MIT); the format behaviour they implement is
widely corroborated by other open implementations. Not yet corroborated against files written
by PE-DESIGN 11 itself. Details in FILE_FORMATS.md.

Findings confirmed by tests against pyembroidery output:

- The PEC section is fixed-layout: a 512-byte header, the stitch block, then (colours + 1)
  48×38 icons.
- PEC stitch deltas use a 7-bit short form for −64 < d < 63, otherwise a 12-bit long form with
  flag bits 0x20 (trim) / 0x10 (jump).
- The DST header's `ST:` counts every move record, including the three jumps that encode a trim.

## 2026-10-18: Official online user's guide

Source: Brother's public PE-DESIGN 11 online user's guide (Version C,
support.brother.com). Headings with links are in `research/pedesign11-manual-toc.md`.

- Seven applications: Layout & Editing, Design Center, Design Database, Programmable Stitch
  Creator, Font Creator, Stitch Design Factory, Link.
- Three data types: stitch, text and shape patterns, convertible between them.
- File formats: the authoritative list is in FILE_FORMATS.md.
- 8 line sew types and 15 region sew types, each with its attribute set.
- Wizards: Auto Punch, Cross Stitch, Photo Stitch 1/2, appliqué, patch, cutwork, background
  fill, split / multi-position / jumbo frame, Print and Stitch, Name Drop, templates.
- Machine transfer: USB media, direct to machine memory, wireless, original card, USB/wireless
  Link.

## 2026-10-18: PhotoCraft (architecture reference)

Studied PhotoCraft's command-engine, architecture, control-protocol and testing docs
(github.com/storytold/photocraft). The principles adopted are in ARCHITECTURE.md
("Engine-first layering").

## Next research steps

1. Reference-tier work (VM_PLAN.md) needs a licensed PE-DESIGN installation; until then, the
   guide plus public specs are the sources. Blocked tasks are listed in ROADMAP.md.
2. Read the guide's attribute pages in detail to turn running, satin and fill defaults into
   parameter specs.
3. Produce reference fixtures: the same simple design saved by PE-DESIGN as PES (each
   version), PEC, DST, EXP and JEF, recording the parameters used.
4. Verify the OpenStitch readers on those fixtures, and diff writer output against them.
