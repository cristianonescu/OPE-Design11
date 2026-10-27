# Distribution

`.github/workflows/build-executables.yml` builds the editor and the command-line tools in release
mode on each OS and uploads one **workflow artifact** per OS (private to the repository, kept 30
days). It runs after the tests on every push to `main` (called from `rust.yml`), and on pull
requests that change `packaging/`, `assets/icons/` or the workflow. Releases are separate:
`release.yml` runs only for a tag the owner pushes (`vX.Y.Z` for a stable version, or
`vX.Y.Z-alpha.N`, `-beta.N`, `-rc.N` for a pre-release, marked as such) and creates a draft
GitHub release with the same packages; `release-smoke.yml` then installs the published Windows
and Linux packages and runs them once. The notes for each release are in `docs/releases/`.

| OS | Artifact | Built by | File associations |
|---|---|---|---|
| macOS (universal: arm64 + x86_64) | `OpenStitch-<label>-macos-universal.zip` holding `OpenStitch.app` | `packaging/macos/bundle.sh` (Info.plist, `.icns`, ad-hoc signature) | `.osd` (owner, exported UTI `io.github.cristianonescu.openstitch.design`), `.pes` (alternate editor) |
| Windows (x64) | `OpenStitch-<label>-windows-x86_64.zip` (portable) and `.msi` | WiX Toolset 3 from `packaging/windows/openstitch.wxs` | `.osd` default (ProgId `OpenStitch.Design`); `.pes` in "Open with" only (`OpenWithProgids`), so an existing PES handler stays the default |
| Linux (x86_64) | `OpenStitch-<label>-linux-x86_64.tar.gz` with `install.sh` | `packaging/linux/package.sh` | `.desktop` entry plus shared-mime-info types `application/x-openstitch-design` (`*.osd`) and `application/x-pes-embroidery` (`*.pes`, magic `#PES`) |
| Linux (x86_64) | `OpenStitch-<label>-linux-x86_64.AppImage` (single file, no install) | `packaging/linux/appimage.sh` (AppDir + `AppRun`, appimagetool) | Same desktop entry, MIME types, icons and thumbnailer, added per user by `--install-integration` |
| Linux (x86_64) | `OpenStitch-x86_64.flatpak` bundle (workflow `Flatpak`, separate artifact) | `packaging/flatpak/io.github.cristianonescu.OpenStitch.yml` (flatpak-builder) | Exported desktop entry and MIME types under the app id |

The Linux job validates the desktop entry (`desktop-file-validate`) and MIME XML, installs into a
temporary prefix and checks the MIME database picked up both globs; it then builds the AppImage
and tests its thumbnailer and desktop integration (see [AppImage](#appimage)). The macOS job checks the
signature and the document types in the bundle.

Every artifact also holds `openstitch-cli-<label>-<os>` (`.tar.gz`, or `.zip` on Windows)
with `stitchtool`, `openstitch-mcp`, the README and `examples/`. `<label>` is `main-<sha>` for
builds from `main` and the tag (e.g. `v0.2.0`) for releases; the MSI's internal version is the
workspace version. Before packaging, each job smoke-tests the binaries (`stitchtool exec
examples/badge.json`, `openstitch-mcp --help`).

## AppImage

`packaging/linux/appimage.sh <openstitch> <version> <out-dir>` lays out an AppDir (editor and
thumbnailer under `usr/bin`, the desktop entry, MIME XML, thumbnailer entry and hicolor icons
under `usr/share`, `AppRun` and the 256 px icon at the top) and packs it with
[appimagetool](https://github.com/AppImage/appimagetool) 1.9.1 and the static
[type2 runtime](https://github.com/AppImage/type2-runtime) `20251108`. Both are downloaded from
those versioned releases (never `continuous`) and checked against SHA-256 sums pinned in the
script (x86_64 and aarch64); set `APPIMAGETOOL` / `APPIMAGE_RUNTIME` to use local copies. The
type2 runtime is statically linked, so the AppImage needs no `libfuse2` on the host (it uses
FUSE 3 when present; `--appimage-extract-and-run` works without FUSE).

`AppRun` takes three extra options besides a file to open:

| Option | Effect |
|---|---|
| `--thumbnailer -s SIZE IN OUT.png` | Runs the bundled `openstitch-thumbnailer` |
| `--install-integration` | For the current user (`$XDG_DATA_HOME`, default `~/.local/share`): desktop entry whose `Exec` is this AppImage's path, `.osd`/`.pes` MIME types, icons, the thumbnailer entry, and a copy of the thumbnailer in `openstitch/appimage/` (file managers run thumbnailers outside the AppImage mount, often sandboxed); sets OpenStitch as the default for both types |
| `--remove-integration` | Removes all of the above |

Run `--install-integration` again after moving or updating the AppImage. Tools such as
AppImageLauncher or appimaged also pick up the embedded desktop entry, but not the thumbnailer.

The CI step prints `ldd` of the editor, renders a thumbnail through `AppRun`, installs the
integration into a temporary `XDG_DATA_HOME`, validates the written desktop entry, checks its
`Exec`, the MIME glob, the icon and the copied thumbnailer, then removes it again. The binaries
are built on `ubuntu-latest`, so the AppImage needs a glibc at least as new as that runner's
(Ubuntu 24.04: glibc 2.39). X11, Wayland, OpenGL and Vulkan libraries are loaded at run time
from the host, as with the tarball.

## Flatpak

`packaging/flatpak/io.github.cristianonescu.OpenStitch.yml` builds the editor against the
freedesktop 25.08 runtime with the `rust-stable` SDK extension. It installs the binary, the
desktop entry, MIME types and icons renamed to the app id, and AppStream metadata
(`io.github.cristianonescu.OpenStitch.metainfo.xml`). Sandbox: Wayland with X11 fallback, GPU,
the home folder and removable media (`/media`, `/run/media`); file dialogs go through the
desktop portal (rfd's `xdg-portal` backend).

```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak-builder --user --install-deps-from=flathub --force-clean --install build-flatpak packaging/flatpak/io.github.cristianonescu.OpenStitch.yml
flatpak run io.github.cristianonescu.OpenStitch
```

`.github/workflows/flatpak.yml` runs this build (on pull requests and pushes to `main` that
touch `packaging/flatpak/`, `packaging/linux/`, the icons or the workflow, and on demand),
validates the AppStream file with `appstreamcli`, checks the exported desktop entry, MIME types
and icons, and uploads `OpenStitch-x86_64.flatpak` (install with `flatpak install --user
OpenStitch-x86_64.flatpak`; the runtime comes from Flathub). It is a separate workflow so that
the main build does not wait for the SDK download.

Limits:
- flatpak-builder runs `appstreamcli compose` on the host, which needs the host's SVG image
  loader (Debian/Ubuntu: `librsvg2-common`) and `eu-strip` (`elfutils`); without them the build
  ends with `file-read-error` on the scalable icon or a missing `eu-strip`.
- Cargo downloads crates during the build (`build-args: --share=network`), which local
  `flatpak-builder` allows. Flathub builds offline: a submission needs the crates as sources,
  generated from `Cargo.lock` by `flatpak-cargo-generator.py` (flatpak-builder-tools) into
  `cargo-sources.json`, plus `CARGO_NET_OFFLINE=true`.
- Flatpak does not export thumbnailers, so file managers show no stitch previews for the
  Flatpak (use the tarball or the AppImage integration for that).
- The sandbox has no network, so `openstitch --listen` cannot be reached from `stitchtool` on
  the host; allow it with `flatpak override --user --share=network io.github.cristianonescu.OpenStitch`.

## Icons

The OpenStitch logo (a needle and its thread on a navy tile), generated with Higgsfield; see
`assets/brand/README.md`. `assets/icons/openstitch.svg` is the vector master and the Linux
scalable icon. `tools/gen_icons.py` (Pillow, plus resvg or Inkscape to rasterise) writes from it
the PNGs 16–1024, `openstitch.ico`, `openstitch.icns` and `openstitch-64.rgba`, which the app
embeds as its window icon; at 32 px and below the needle and thread are drawn bolder so they
stay visible. Re-run the script after changing the SVG. On Windows (MSVC) the build also embeds
`openstitch.ico` in `openstitch.exe` (`crates/openstitch-app/build.rs`), which the taskbar,
Alt+Tab and Explorer show.

The editor shows the logo as a splash for about a second on a plain start (no options, or only
a file to open). Click or press a key to skip it. It never shows with `--screenshot`,
`--script`, `--listen` or any option that sets up a view; `--no-splash` and Options ▸ "Show
splash at startup" turn it off.

## Opening files

The editor opens a file given on the command line (`openstitch design.osd`, or any machine
file, which is imported into a new design). Windows and Linux (tarball, AppImage integration,
Flatpak) pass the double-clicked file that way. macOS Finder does not; see *macOS double-click*
under Known limits.

## Known limits

- **MSI validation.** `light` runs with `-sice:ICE50` because the Start-menu shortcut uses
  `openstitch.ico` rather than the icon inside `openstitch.exe`. The exe now embeds the same
  icon (`crates/openstitch-app/build.rs` links it as a resource with the MSVC toolchain), so the
  shortcut can point at the exe and the suppression can go.
- **Not signed for distribution.** macOS builds are ad-hoc signed (Gatekeeper asks on first
  launch); Windows builds carry no Authenticode signature. Signing needs certificates from the
  owner (Apple Developer ID + notarisation; a code-signing certificate for Windows).
- **macOS double-click (Blocked).** Finder passes a document (double-click, *Open With*, a drop
  on the Dock icon) as an `odoc` Apple Event, not on the command line. Double-clicking a `.osd`
  or `.pes` starts OpenStitch but does not load the file; File ▸ Open works. Checked against the
  versions in `Cargo.lock` (eframe 0.37, winit 0.30.13, objc2 0.6 / objc2-app-kit 0.3):
  - winit has no file-open event. Its macOS docs say it will never set an application
    delegate and tell applications to register their own `NSApplicationDelegate` with
    `application:openURLs:` (`winit::platform::macos`, module docs).
  - Registering a delegate means defining an Objective-C class: objc2's `define_class!`
    (`declare_class!` before 0.6) requires `unsafe impl` blocks and `#[unsafe(method(...))]`
    attributes in the calling crate, and `#![forbid(unsafe_code)]` rejects them (they cannot
    be `allow`ed under `forbid`).
  - The other route, `NSAppleEventManager::setEventHandler_andSelector_forEventClass_andEventID`,
    is an `unsafe fn` and also needs a handler object of a defined class.
  - eframe 0.37 exposes no hook for this, and no maintained crate offers a safe wrapper that
    works with winit's event loop (tao, the Tauri fork of winit, has `Event::Opened`, but
    eframe cannot run on it).

  Unblocking needs one of: upstream support in winit/eframe (an `open_urls`-style event), or
  the owner allowing `unsafe` in one small, separately audited crate (a ~40-line objc2
  delegate that forwards the paths to the editor through a channel; the same exception as the
  Quick Look and Explorer thumbnailers below).
- **Flatpak is not on Flathub.** The bundle comes from the `Flatpak` workflow; publishing on
  Flathub needs offline cargo sources (see [Flatpak](#flatpak)) and the owner's submission.
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
