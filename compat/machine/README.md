# Sew-out records

A sew-out record says how one of OpenStitch's exports sewed on a real Brother machine. A spec
with a `machine` block reaches the **machine** tier when its record says `pass`, and when its
reference checks (if any) and all lower tiers pass too.

1. Run `cargo run -p stitchtool -- compat --only <spec-id>`. Then take the file named in the
   spec's `machine.make` from `target/compat/work/<spec-id>/`.
2. Sew it as `machine.make` describes.
3. Save a record at the path in `machine.record`, inside this folder:

```json
{
  "schema": "openstitch-compat-machine/1",
  "spec": "satin-zigzag-rect",
  "result": "pass",
  "machine": "Brother model, firmware version",
  "file": "satin.pes",
  "fileSha256": "<shasum -a 256 of the file you sewed>",
  "fabric": "medium-weight woven cotton",
  "stabilizer": "tear-away, one layer",
  "sewnBy": "Your name",
  "date": "2026-10-20",
  "observations": "what you saw: edges, trims, thread breaks, registration"
}
```

The record counts only while `fileSha256` matches the current export. Once OpenStitch's output
changes, the check goes back to pending until the new file is sewn. Photos may sit next to the
record. Do not commit photos that show anything other than the sew-out.
