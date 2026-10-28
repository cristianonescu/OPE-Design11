# Distribution

`.github/workflows/build-executables.yml` builds the editor and the command-line tools in release
mode on each OS and uploads one **workflow artifact** per OS (private to the repository, kept 30
days). It runs after the tests on every push to `main` (called from `rust.yml`), and on pull
requests that change `packaging/`, `assets/icons/` or the workflow. Releases are separate:
`release.yml` runs only for a tag the owner pushes (`vX.Y.Z` for a stable version, or
`vX.Y.Z-alpha.N`, `-beta.N`, `-rc.N` for a pre-release, marked as such) and creates a draft
GitHub release with the same packages and a `SHA256SUMS` file (sha256sum format, one line per
asset) that the editor's self-updater checks downloads against; `release-smoke.yml` then installs
the published Windows and Linux packages and runs them once. The build receives the label as
`OPENSTITCH_RELEASE_TAG`, which the editor embeds so a release build knows its tag (see
[research/updater.md](research/updater.md)). The notes for each release are in `docs/releases/`.

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
signature, the document types and the icon in the bundle. Every job checks that its packages
carry the app icon (see [Icons and shortcuts](#icons-and-shortcuts)).

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

## Icons and shortcuts

The OpenStitch logo (a needle and its thread on a navy tile), generated with Higgsfield; see
`assets/brand/README.md`. `assets/icons/openstitch.svg` is the vector master and the Linux
scalable icon. `tools/gen_icons.py` (Pillow, plus resvg or Inkscape to rasterise) writes from it
the PNGs 16–1024, `openstitch.ico` (16/24/32/48/64/128/256), `openstitch.icns` and
`openstitch-64.rgba`, which the app embeds as its window icon; at 32 px and below the needle and
thread are drawn bolder so they stay visible. Re-run the script after changing the SVG and
commit every file it writes; the packages below take them from `assets/icons/`.

What each package carries, and what the workflow checks before it uploads the package:

- **Windows exe (zip and MSI).** `openstitch.exe` embeds `openstitch.ico` as its icon resource
  (`crates/openstitch-app/build.rs` writes a `.res` and links it with the MSVC toolchain), so
  Explorer, the taskbar and Alt+Tab show the logo for the exe itself, and the self-updater's
  in-place swap of the exe keeps it. The zip also holds `openstitch.ico`.
- **Windows MSI.** The Icon table holds `openstitch.ico`; the advertised Start-menu shortcut,
  the `OpenStitch.Design` ProgId (`.osd` files) and `ARPPRODUCTICON` (Programs and Features)
  refer to it, and `.pes` files get the installed `openstitch.ico` as their `DefaultIcon`.
  Windows Installer copies the Icon-table entry to `%WINDIR%\Installer\{ProductCode}\` under
  its table name and the shell loads it by that name's extension, so the name must end in
  `.ico` or `.exe`: v0.1.0-alpha.4 named it `OpenStitchIcon` and the Start menu, the taskbar
  (for a window started from the shortcut) and Programs and Features all showed the generic
  icon. The workflow fails if any icon name in the MSI lacks the extension, or if the exe has
  no icon resource.
- **macOS.** `OpenStitch.app/Contents/Resources/openstitch.icns` (16–1024 px, `ic07`–`ic14`),
  named by `CFBundleIconFile` in `Info.plist`; the `.osd` document type and its UTI use the same
  file. `ditto` zips the bundle with its structure. The workflow checks the plist key, that the
  file is an icon and that the zip lists it.
- **Linux tarball.** `share/icons/hicolor/<size>x<size>/apps/openstitch.png` for 16–512 and
  `scalable/apps/openstitch.svg`, with `share/applications/openstitch.desktop` (`Icon=openstitch`);
  `install.sh` copies them under the prefix and refreshes the icon cache. The workflow installs
  into a temporary prefix and checks that every size landed.
- **AppImage.** `openstitch.desktop`, `openstitch.png` (256 px) and `.DirIcon` at the AppDir
  root, which file managers and AppImage launchers read, plus the same hicolor set under `usr/
  share`; `--install-integration` installs the hicolor icons for the user. The workflow extracts
  the image (`--appimage-extract`) and checks the root files and the hicolor set.
- **Flatpak.** The hicolor icons renamed to the app id (see [Flatpak](#flatpak)).

The editor shows the logo as a splash for about a second on a plain start (no options, or only
a file to open). Click or press a key to skip it. It never shows with `--screenshot`,
`--script`, `--listen` or any option that sets up a view; `--no-splash` and Options ▸ "Show
splash at startup" turn it off.

## Opening files

The editor opens a file given on the command line (`openstitch design.osd`, or any machine
file, which is imported into a new design). Windows and Linux (tarball, AppImage integration,
Flatpak) pass the double-clicked file that way. macOS Finder does not; see *macOS double-click*
under Known limits.

## Updates

The editor updates itself (design and decision: [research/updater.md](research/updater.md);
code: `crates/openstitch-update`, `crates/openstitch-app/src/update_ui.rs`).

- **When.** A plain interactive start (at most a file to open; `--no-splash` is fine) of a
  release build starts a check on a background thread; `--screenshot`, `--script`, `--listen`
  and the other options that set up a view do not, nor does a development build (debug, or a
  binary under a cargo `target/` directory), a Flatpak, or a start with
  `OPENSTITCH_NO_UPDATE_CHECK` set. Options ▸ *Check for Updates Now* and Help ▸ *Check for
  Updates* ask on demand and also report "You have the latest version" or the error.
- **What is newer.** The build's tag (`OPENSTITCH_RELEASE_TAG` at build time; About shows it)
  is compared with GitHub's release list using semantic versioning with pre-release precedence
  (`0.1.0-alpha.3 < 0.1.0-alpha.4 < 0.1.0-beta.1 < 0.1.0`). A pre-release build sees newer
  pre-releases and stable releases; a stable build sees stable releases only unless Options ▸
  *Include pre-releases* is on. Drafts are never offered. The check is one anonymous request
  (GitHub allows 60 an hour per address); offline or on any error the start is silent.
- **The dialog.** Shows the notes of every release newer than the running one (the release
  body: `docs/releases/<tag>.md` plus GitHub's generated list), newest first. *Dismiss* closes
  it; with *Don't remind me about this version* ticked the version is remembered in `ui.json`
  (`updates.skipVersion`) and offered again only when something newer appears. *Update* is
  disabled while the design has unsaved changes.
- **Update.** Downloads `SHA256SUMS`, then the package for this install to a temporary folder,
  refusing it unless its SHA-256 matches, over HTTPS only (rustls) and only from the
  repository's release URLs. Then, per install:

  | Install | Package | How it is applied |
  |---|---|---|
  | Windows portable (exe outside Program Files) | `OpenStitch-<tag>-windows-x86_64.zip` | `openstitch.exe` is unpacked beside the running one as `.new`, the running exe is renamed `.old`, the new one takes its place, and the editor restarts; `.old` is removed at the next start |
  | Windows MSI (`%ProgramFiles%\OpenStitch`) | `OpenStitch-<tag>-windows-x86_64.msi` | The editor exits and a detached `cmd` runs `msiexec /i … /passive /norestart` (Windows asks for permission, the usual publisher-unknown prompt for an unsigned package), then starts the installed editor |
  | macOS `OpenStitch.app` | `OpenStitch-<tag>-macos-universal.zip` | The zip is unpacked beside the bundle, the bundle is renamed `OpenStitch.app.old`, the new one takes its place and is started with `open -n`; the download sets no quarantine attribute, so Gatekeeper does not ask again |
  | Linux AppImage | `OpenStitch-<tag>-linux-x86_64.AppImage` | The new image replaces `$APPIMAGE` (desktop integration keeps pointing at the same path) and is started |
  | Linux tarball | `OpenStitch-<tag>-linux-x86_64.tar.gz` | `bin/openstitch` is replaced; the thumbnailer is not (reinstall the tarball for that) |
  | Flatpak | — | `flatpak update`; the editor never checks |

  A location the user cannot write (an app bundle installed by an administrator, a system
  prefix) makes the update fail with the reason and a link to the release page. The restarted
  editor gets the same command-line arguments and shows "Updated from X to Y" in the status
  bar.
- **Settings** in `ui.json` under `updates`: `check` (default true), `includePrereleases`
  (default false), `skipVersion`.
- **Testing a build without a release:** with `OPENSTITCH_UPDATE_API` and
  `OPENSTITCH_UPDATE_DOWNLOADS` set, the client uses that API base and download prefix (plain
  http allowed) instead of GitHub. The release pipeline publishes `SHA256SUMS` since
  v0.1.0-alpha.4, the first version that checks; a release without it is never applied.

## Known limits

- **MSI validation.** `light` runs with `-sice:ICE50`: the advertised Start-menu shortcut's
  icon is `openstitch.ico` from the Icon table while its target is `openstitch.exe`, and ICE50
  wants the two extensions to match. Making the Icon entry from the exe would store the whole
  binary (about 30 MB) in the MSI's Icon table, so the `.ico` stays and ICE50 stays suppressed.
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
- **macOS application menu.** `OpenStitch.app` installs no application menu (the bold menu
  next to the Apple menu is empty): `main.rs` builds the event loop with winit's
  `with_default_menu(false)`. winit's default menu named its items after the process
  ("About openstitch", "Quit openstitch"; `CFBundleName` is not consulted) and its Quit item
  ended the process through `NSApplication terminate:`, which never reaches the window's close
  request, so Cmd+Q lost unsaved changes without the prompt (winit 0.30.13 / eframe 0.37, which
  has no hook into `applicationShouldTerminate:`; a delegate needs `unsafe` objc2 code, see
  *macOS double-click*). Without the menu the keys reach the editor: Cmd+Q and Cmd+W close
  through the unsaved-changes prompt, Cmd+M minimises (`EditorApp::macos_app_shortcuts`,
  listed in Help ▸ Keyboard Shortcuts). Not available: Cmd+H / Hide Others, Services and the
  menu's About (Help ▸ About OpenStitch remains). Quitting from the Dock icon's menu or at
  logout still terminates without the prompt. A native menu with the right names and a Quit
  item routed through the prompt needs a menu crate such as `muda` (safe API over objc2);
  not done yet.
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
