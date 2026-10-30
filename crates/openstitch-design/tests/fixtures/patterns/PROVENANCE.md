# Owner-made Programmable Stitch Creator files

Made by the project owner, Cristian Onescu, on 2026-10-29 in his licensed PE-DESIGN 11.50
(Programmable Stitch Creator, Windows 11 Pro), drawn by hand on the editor's grid and saved
through the normal Save As dialog (each mode offers exactly one type). They contain no
Brother pattern, motif or font content: Brother's bundled files were never opened, read or
copied. Kept byte for byte (CRLF, `.gitattributes`). Recorded in the reference round 4 log
("PATTERN FILES", 13:37–13:41; psc-deco-2lines: "DECORATIVE FILL follow-ups", 2026-10-30 09:53–09:59).

| File | Mode, Save type (Default Size) | What was drawn (cell units, 0…1024, grid snap) | SHA-256 |
|---|---|---|---|
| psc-line-2pt.pas | Fill/Stamp, `*.pas` (7.5 mm) | one line, cell top-left (0,0) to bottom-right (1024,1024) | d6a291cb42d8e50162be96368a4994b4a74e655776d423b222073c91f141b105 |
| psc-square-4pt.pas | Fill/Stamp, `*.pas` (7.5 mm) | clicks (128,128) (896,128) (896,896) (128,896), double-click back on (128,128): written with its closing point, 5 points | f50dae1bfc5b90767601cd0256639978b34ced8a3335f9601ce3917d09d97ad3 |
| psc-two-lines.pas | Fill/Stamp, `*.pas` (7.5 mm) | two separate 2-point lines at y 256 and 768: two `@TILELINE` records, nothing marks a jump | 438960972e42d1d34117c37b79c605168a4ded7b2592acc332a9ab0595477bfd |
| psc-motif-2pt.pmf | Motif, `*.pmf` (5.0 mm) | the editor's fixed base line (0,512)–(1024,512), saved untouched | faea0ef7cb5ea8ce0b5e4e4d86f210b013a13e99b0806b07c11cf440e5ce8914 |
| psc-motif-zigzag-3pt.pmf | Motif, `*.pmf` (5.0 mm) | a point added on the base line and dragged to (512,768) | 2008a77bbe9e98138815e7c81796af04dfdbbc296914266ce16594075c5af7e2 |
| psc-deco-1line.plf | Decorative Fill, `*.plf` (100.0 mm) | one line (128,128)–(896,896) on the 16 × 16 grid | 9f2d039a64d30e3c7ad0498858a6a4bfe34ce73e046404498225ee58b4161416 |
| psc-deco-2lines.plf | Decorative Fill, `*.plf` (100.0 mm); drawn 2026-10-30 | two separate lines (128,256)–(896,256) and (128,768)–(896,768): `@SYSENV 10000 10000 2`, the third number 2 here and 1 for one line | f3295abab0cf056940e7b2ed61ec2b0f0aea506d4d1bc2e71001a11264626f9e |

The PES files PE-DESIGN sewed from them (a 40 × 40 mm square with a programmable fill of
psc-square-4pt and of psc-line-2pt, a motif fill of psc-motif-2pt, a motif line of
psc-motif-zigzag-3pt) are compat references `compat/reference/stitches/prog-fill-square-4pt.pes`,
`prog-fill-line-2pt.pes`, `motif-fill-2pt.pes` and `line-motif-zigzag-3pt.pes` (specs of the
same names), with their settings in `compat/reference/MANIFEST.json`; so are
`deco-fill-1line-100mm.pes` and `deco-fill-1line-offcentre.pes` (psc-deco-1line as a decorative
fill on a centred square and on an off-centre rectangle).
