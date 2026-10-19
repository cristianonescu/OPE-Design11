# Distribution

`.github/workflows/build-executables.yml` builds the editor and the command-line tools in release
mode on each OS and uploads one **workflow artifact** per OS (private to the repository, kept 30
days). It runs after the tests on every push to `main` (called from `rust.yml`), and on pull
requests that change `packaging/`, `assets/icons/` or the workflow. Releases are separate:
`release.yml` runs only for a `vX.Y.Z` tag the owner pushes for a stable version and creates a
draft GitHub release with the same packages.

| OS | Artifact | Built by | File associations |
|---|---|---|---|
| macOS (universal: arm64 + x86_64) | `OpenStitch-<label>-macos-universal.zip` holding `OpenStitch.app` | `packaging/macos/bundle.sh` (Info.plist, `.icns`, ad-hoc signature) | `.osd` (owner, exported UTI `io.github.cristianonescu.openstitch.design`), `.pes` (alternate editor) |
| Windows (x64) | `OpenStitch-<label>-windows-x86_64.zip` (portable) and `.msi` | WiX Toolset 3 from `packaging/windows/openstitch.wxs` | `.osd` default (ProgId `OpenStitch.Design`); `.pes` in "Open with" only (`OpenWithProgids`), so an existing PES handler stays the default |
| Linux (x86_64) | `OpenStitch-<label>-linux-x86_64.tar.gz` with `install.sh` | `packaging/linux/package.sh` | `.desktop` entry plus shared-mime-info types `application/x-openstitch-design` (`*.osd`) and `application/x-pes-embroidery` (`*.pes`, magic `#PES`) |

The Linux job validates the desktop entry (`desktop-file-validate`) and MIME XML, installs into a
temporary prefix and checks the MIME database picked up both globs. The macOS job checks the
signature and the document types in the bundle.

Every artifact also holds `openstitch-cli-<label>-<os>` (`.tar.gz`, or `.zip` on Windows)
with `stitchtool`, `openstitch-mcp`, the README and `examples/`. `<label>` is `main-<sha>` for
builds from `main` and the tag (e.g. `v0.2.0`) for releases; the MSI's internal version is the
workspace version. Before packaging, each job smoke-tests the binaries (`stitchtool exec
examples/badge.json`, `openstitch-mcp --help`).

## Icons

Original artwork (an embroidery hoop with a satin-stitched star and a threaded needle), drawn in
code by `tools/gen_icons.py` (Pillow): `assets/icons/openstitch.svg` (same geometry, Linux
scalable icon), PNGs 16–1024, `openstitch.ico`, `openstitch.icns` and `openstitch-64.rgba`, which
the app embeds as its window icon. Re-run the script after changing the artwork; it uses no
third-party images or fonts.

## Opening files

The editor opens a file given on the command line (`openstitch design.osd`, or any machine
file, which is imported into a new design). Windows and Linux pass the double-clicked file that
way.

## Known limits

- **MSI validation.** `light` runs with `-sice:ICE50` because the Start-menu shortcut uses
  `openstitch.ico` while `openstitch.exe` has no embedded icon resource. Embedding the icon in
  the exe (a Windows resource at build time) removes the need for the suppression.
- **Not signed for distribution.** macOS builds are ad-hoc signed (Gatekeeper asks on first
  launch); Windows builds carry no Authenticode signature. Signing needs certificates from the
  owner (Apple Developer ID + notarisation; a code-signing certificate for Windows).
- **macOS double-click.** Finder hands a document to a running app through an Apple Event, which
  winit (under eframe) does not deliver. Double-clicking a `.osd` or `.pes` starts OpenStitch but
  does not load the file yet; File ▸ Open works. Handling `application:openURLs:` needs an
  AppKit delegate hook, which requires `unsafe` (forbidden in this workspace) or upstream
  support in winit.
- **Windows executable icon.** The MSI shows the icon on the Start-menu shortcut, in Add/Remove
  Programs and on `.osd`/`.pes` files; `openstitch.exe` itself has no embedded icon resource yet
  (needs a resource compiler step in the build).
- **macOS architecture.** The bundle is arm64 only (the `macos-latest` runner); an x86_64 or
  universal build needs a second target and `lipo`.
- **No AppImage/Flatpak yet**; the tarball installs per user or system-wide with `install.sh`.
- File-manager thumbnails exist on Linux only; see below.

## Thumbnails

`openstitch-thumbnailer` (crate `openstitch-render`) writes a PNG of the stitches of an `.osd`
project or any machine file OpenStitch reads, cropped to the design on a transparent square:
`openstitch-thumbnailer -s SIZE INPUT OUTPUT.png`. Inputs over 64 MiB are refused; the readers
are the fuzzed `openstitch-formats` codecs.

| OS | State |
|---|---|
| Linux | Done: `packaging/linux/openstitch.thumbnailer` (`Exec=openstitch-thumbnailer -s %s %i %o`) for `application/x-openstitch-design` and `application/x-pes-embroidery`; `build-executables.yml` builds it next to the editor, `package.sh` ships it, `install.sh` installs it into `share/thumbnailers`, and the Linux job renders a thumbnail from the installed copy. |
| macOS | Blocked. Quick Look thumbnails come from an app extension (`.appex`) implementing `QLThumbnailProvider`, loaded by the system into its own process; the old `.qlgenerator` plug-ins are no longer loaded by current macOS. Writing either from Rust means Objective-C runtime FFI, which needs `unsafe` (forbidden workspace-wide); the alternative is a small Swift extension, which the Rust-only rule excludes. |
| Windows | Blocked. Explorer thumbnails come from an in-process COM server DLL implementing `IThumbnailProvider` and `IInitializeWithStream`, registered under the file type. COM vtables from Rust need `unsafe`. |
| PEN | Blocked: the format is undocumented (see FILE_FORMATS.md). |

Lifting the `unsafe` ban for one small, separately audited crate per OS (objc2 for Quick Look,
`windows` crate COM for Explorer) would unblock both; that is the owner's decision.
