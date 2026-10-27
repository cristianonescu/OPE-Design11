# How to add a reference file

A reference file is a design saved by PE-DESIGN 11 for one compatibility spec. With it, the
spec's reference checks run and the spec can reach the **reference** tier. Without it, those
checks are reported as "pending reference", never as passed or failed.

Only the owner adds these files, from their own licensed PE-DESIGN 11. Black-box use is
allowed (Art. 5(3)); see [LEGAL_AND_LICENSING.md](../../docs/LEGAL_AND_LICENSING.md). The
harness compares only documented data: stitches, colour blocks, the thread table, the documented
header fields, and the names of the object classes. Nothing here studies undocumented fields.

## 1. Find what to make

1. Run `cargo run -p stitchtool -- compat`.
2. Open `target/compat/summary.md`. Under "Waiting for files", each line is one file to make, with
   its spec.
3. Open the spec (`compat/specs/<area>/<id>.json`) and read `references[].make`. It says exactly
   what to draw or open in PE-DESIGN, and which settings to use.

Some specs say to open OpenStitch's own export, for example `pes-default-v6`. These files are in
`target/compat/work/<id>/` after a run. Copy them into the VM.

## 2. Make it in PE-DESIGN 11

- Start from a new design. Use the Design Page size given in `make`.
- Set only the settings `make` names. Leave every other setting at its default.
- If you had to change anything else, write it down.
- Save with **Save As** in PE-DESIGN's default PES version, unless `make` says otherwise.
- Do not use Brother's built-in designs, fonts, motifs or programmable-fill patterns unless
  `make` asks for them.

## 3. Name it and put it in place

- Save it under the path in `references[].file`, inside this folder. For example, spec
  `satin-zigzag-rect` uses `compat/reference/stitches/satin-zigzag-rect.pes`.
- Do not rename it, and do not open it again in OpenStitch before saving it here.

## 4. Record where it came from

Add an entry to [MANIFEST.json](MANIFEST.json). A file without an entry, or whose SHA-256 does
not match its entry, is an error. It never counts as a pass.

```json
{
  "file": "stitches/satin-zigzag-rect.pes",
  "spec": "satin-zigzag-rect",
  "reference": "pe-design",
  "madeBy": "Your name",
  "peDesignVersion": "11.30",
  "date": "2026-10-20",
  "os": "Windows 11 23H2 (VM)",
  "settings": "Satin Stitch, Direction 90°, Density 3.0 lines/mm, under sewing off; Save As PES (default version)",
  "sha256": "<output of: shasum -a 256 compat/reference/stitches/satin-zigzag-rect.pes>",
  "notes": "anything unexpected you saw"
}
```

`settings` and `notes` are public. Write what was done in PE-DESIGN, not where it was done:
no local paths (`C:\Users\...`, `D:\...`), user names, machine names or licence details. Name an
input file by its repository path or its SHA-256.

`reference` is the `references[].id` in the spec. On Windows, get the SHA-256 with
`certutil -hashfile <file> SHA256`, and write it in lower case.

## 5. Check and commit

1. Run `cargo run -p stitchtool -- compat --only <spec-id>`. The spec's reference checks now pass
   or fail, and each difference is listed in `target/compat/summary.md`.
2. Run `cargo run -p stitchtool -- compat --status docs/COMPATIBILITY_STATUS.md` to update the
   generated tier table.
3. Commit the file, the manifest and the table together.

This repository is public. Commit only simple test shapes you drew yourself. Keep any file
that holds Brother content outside the repository, and run the harness with
`--reference /path/to/your/folder` (the same layout plus its own `MANIFEST.json`).

If a spec asks you to open an OpenStitch file and PE-DESIGN cannot open it, or opens it blank, do
not add a file. Write what you saw in the "Reference tier" section of
[COMPATIBILITY_TESTS.md](../../docs/COMPATIBILITY_TESTS.md).
