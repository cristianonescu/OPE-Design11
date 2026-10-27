# Legal and licensing

This is not legal advice. It records the project's working assumptions and the points that need
a lawyer's review.

## Working position

OpenStitch is an independent, clean-room implementation. Behaviour is specified from Brother's
public documentation, published or open-source format descriptions, files the contributors are
entitled to use, and black-box observation of a licensed copy of PE-DESIGN 11. No Brother code,
binaries, fonts, built-in designs, artwork, icons or UI assets are copied into the product; thread
charts contain colour facts only.

The EU Software Directive (2009/24/EC, Art. 5(3) and 6) permits observing and testing a program
one is licensed to use. It also permits limited decompilation strictly for interoperability of
an independently created program, when the information is not otherwise available. It does not
permit using the results to make a substantially similar program or for other purposes. Art. 8
makes contract terms void where they conflict with Art. 5(3) and 6.

## User interface

The editor arranges its commands like PE-DESIGN 11's Layout & Editing window (Application
button, ribbon tabs and groups, Quick Access Toolbar, Sewing Order and attribute panes, status
bar) so existing users can migrate; [UI_LAYOUT.md](UI_LAYOUT.md) maps each command. The line
the project keeps:

- **Reproduced:** the arrangement of commands, their names, grouping and workflow, as described
  in Brother's public online guide. These are functional: they are how a user finds a command.
- **Our own:** every icon, the Application button mark, colours, spacing and the rest of the
  visual style. The command icons are the SVGs in `assets/icons/commands`, generated for
  OpenStitch with Higgsfield (Recraft V4.1, vector output) under the project owner's account
  and cleaned by `tools/clean_icons.py`. Every prompt is in `assets/icons/commands/prompts.tsv`:
  each describes a generic object (a folder, scissors, a needle) in OpenStitch's own palette,
  and none names or describes Brother, PE-DESIGN or their icons; no reference images were
  used. The vector shapes in `crates/openstitch-app/src/icons.rs` are the fallback when an SVG
  cannot be rendered. The application icon (`assets/icons/openstitch*`) is drawn by
  `tools/gen_icons.py` and the Application button mark in code.
- **Illustrations and textures:** the spot illustrations in `assets/illustrations` (empty
  states, thumbnail placeholders, wizard headers) and the Realistic view's cloth
  (`assets/textures/fabric_cotton.png`) were generated for OpenStitch with Higgsfield (Recraft
  V4.1, vector output for the illustrations, raster for the cloth) under the project owner's
  account, in the command icons' style. Every prompt is in `assets/illustrations/prompts.tsv`
  and `assets/illustrations/README.md`: each describes a generic object (a hoop, a folder, a
  spool, a woven cotton surface) and none names or describes Brother, PE-DESIGN or their
  artwork; no reference images were used. Without them the editor shows the text alone.
- **Never in the repository:** Brother artwork, icons, logos, fonts or screenshots of
  PE-DESIGN. Documentation screenshots show OpenStitch only.
- **Naming:** "PE-DESIGN" is not used as product branding in the user interface. Compatibility
  statements in the documentation ("opens PE-DESIGN 11 files") are fine.

## Technique register

| Technique | Status | Reason |
|---|---|---|
| Reading Brother's public manuals, help pages and spec sheets | Allowed | Public documentation. |
| Running a licensed PE-DESIGN in an isolated VM and recording inputs and outputs | Allowed | Black-box observation (Art. 5(3)). |
| Parsing files the contributors created or are authorised to use | Allowed | Interoperability. |
| Using open-source format implementations as references or test oracles | Allowed | Within their licences; see Dependencies. |
| Decompiling stitch-generation, Photo Stitch, Auto Punch or font code | **Prohibited** | Goes beyond interoperability; would produce a substantially similar program. Use black-box experiments instead. |
| Decompiling file-format readers or writers | **Prohibited pending legal review** | Possible under Art. 6 only if the information is not otherwise available. Public specs and black-box round trips cover most needs. |
| Studying PE-DESIGN output files to discover undocumented fields | **Needs legal review** | Art. 5(3)/6 arguments apply but are unsettled for data formats. Verifying against known specs is lower risk. |
| Anything that circumvents licensing, activation or other technical protection | **Prohibited** | Never used, never analysed. |
| Extracting Brother embroidery fonts, built-in designs, or thread charts beyond colour facts | **Prohibited for redistribution** | Copyrighted assets. Use independently licensed fonts. |
| Reverse-engineering undocumented machine protocols (wireless transfer, Link, original cards) | **Prohibited** | Marked Blocked in FEATURE_MATRIX.md until a public specification exists. |

## Dependencies

| Component | Licence | Use |
|---|---|---|
| pyembroidery 1.5.1 | MIT | Format reference, generated data tables, test oracle. See `THIRD_PARTY_NOTICES.md`. |
| libembroidery (Embroidermodder) | zlib | Read as a secondary format reference where pyembroidery has no writer (PCS version byte, SEW control codes, XXX offsets). No code or data copied. |

Runtime crates and bundled fonts are listed with their licences in `THIRD_PARTY_NOTICES.md`
and docs/ARCHITECTURE.md.
