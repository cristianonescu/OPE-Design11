# Reference environment (historical note)

This document planned a Windows virtual machine for a licensed PE-DESIGN 11, with snapshots
and an experiment protocol, before any reference file existed. It was not used: since
2026-10-27 the reference files are made on the owner's own Windows machine, in a licensed
PE-DESIGN 11.50, through its normal interface. The protocol that replaced this plan is
[compat/reference/README.md](../compat/reference/README.md): a spec names what to draw, the
file goes under `compat/reference/`, and `MANIFEST.json` records its provenance and SHA-256.

What still holds from the plan:

- Only black-box use of PE-DESIGN: no debuggers, API hooking, memory inspection, or changes to
  its files, licence data or network traffic ([LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md)).
- Only simple test shapes the owner drew, saved at PE-DESIGN's defaults unless the spec says
  otherwise; no Brother built-in designs, fonts, motifs or patterns in the repository.
- Public records say what was done in PE-DESIGN, never where: no local paths, user names or
  machine names.
