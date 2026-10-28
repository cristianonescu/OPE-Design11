# Self-update: research, decision and plan

Goal (owner, 2026-10-28): OpenStitch updates itself. At startup the editor checks GitHub for a
newer release. If there is one it shows the new version's release notes with three choices:
Dismiss, "Don't remind me about this version" (suppresses the prompt for that version only,
until a newer one appears) and Update, which downloads the release, applies it and restarts the
program on the new version. Nobody should have to install an update by hand again.

This document records the survey of how Rust desktop applications do this today, what the
OpenStitch release pipeline already provides, the decision, and the step plan (one pull request
per step). The end-to-end verification result is appended at the bottom as it happens.

## What the pipeline provides today

`release.yml` runs for a tag `vX.Y.Z[-alpha.N|-beta.N|-rc.N]` on `main`, runs the tests, calls
`build-executables.yml` and creates a **draft** GitHub release (pre-release when the tag has a
suffix) with these assets, `<tag>` being the git tag:

| Asset | Install | Running binary |
|---|---|---|
| `OpenStitch-<tag>-windows-x86_64.zip` | Portable: unzip anywhere, run `openstitch.exe` | `<dir>\openstitch.exe` |
| `OpenStitch-<tag>-windows-x86_64.msi` | Installer, per machine (`InstallScope="perMachine"`, `MajorUpgrade`) | `C:\Program Files\OpenStitch\openstitch.exe` |
| `OpenStitch-<tag>-macos-universal.zip` | Unzip, move `OpenStitch.app` to Applications | `OpenStitch.app/Contents/MacOS/openstitch` |
| `OpenStitch-<tag>-linux-x86_64.AppImage` | `chmod +x`, run; `--install-integration` optional | the AppImage runtime mounts it and runs `usr/bin/openstitch`; `$APPIMAGE` holds the image path |
| `OpenStitch-<tag>-linux-x86_64.tar.gz` | `install.sh` into a prefix (`bin/openstitch`, `share/`) | `<prefix>/bin/openstitch` |
| `openstitch-cli-<tag>-<os>.{zip,tar.gz}` | CLI tools | not updated by the editor |
| Flatpak (separate workflow artifact, not a release asset) | `flatpak install` | sandboxed; updates come from the Flatpak remote |

There is no checksum file and no signature today. The macOS bundle is ad-hoc signed; the Windows
builds are unsigned (SmartScreen prompts on first run). None of the editor's crates has an HTTP
client, a zip reader, a hash or a semver parser: the editor is `std` plus eframe, rfd, arboard,
resvg and serde_json. `openstitch-remote` speaks a line protocol over `std::net`.

Settings: `user.json` (engine, `%APPDATA%\OpenStitch`, `~/Library/Application Support/OpenStitch`,
`$XDG_CONFIG_HOME/openstitch`) and `ui.json` beside it (`editor/prefs.rs`: theme, Quick Access
Toolbar, panes, recent files, `showSplash`). `UiPrefs` is hand-mapped JSON that tolerates unknown
and malformed entries, which is the right place for update preferences.

Startup: `main.rs` parses the options into `Launch`; `EditorApp::launch` applies them. The splash
logic already distinguishes a plain interactive start from `--screenshot`, `--script`, `--listen`
and the other automation options (`Launch::splash_allowed`); the update check follows the same
rule.

## Candidates

| Candidate | What it is | Licence | Fit |
|---|---|---|---|
| [`self_update`](https://docs.rs/self_update) 1.3.x | Builder that queries GitHub Releases, picks an asset by name, downloads, extracts and replaces the running executable (via `self-replace`). | MIT | Does the Windows single-exe case well. Pulls `reqwest` (tokio, hyper, rustls) by default; `ureq` backend is opt-in. Asset selection is by substring on target triple, not our names; no app-bundle or AppImage logic; its restart is `exec`-style, left to the caller. Checksums only through its `checksums` feature with a per-asset `.sha256`, signatures only with zipsign. Big dependency surface for the part we would actually use. |
| [`self-replace`](https://docs.rs/self-replace) 1.5 (mitsuhiko, used by rye and uv) | Replace or delete the running executable. Unix: write beside, atomic rename. Windows: rename the running exe aside, copy a helper with `FILE_FLAG_DELETE_ON_CLOSE`, spawn it to delete the old file after the parent exits. | Apache-2.0 | Small (tempfile, fastrand, windows-sys) and proven. Its Windows trick exists to leave *no* stale file behind; we can get the same result without a helper process by deleting the old file at the next start. Kept as the fallback if that proves flaky. |
| [`velopack`](https://docs.velopack.io/getting-started/rust) | Full installer + updater framework (Squirrel successor): `Update.exe`, delta packages, `vpk` packer. | MIT | Needs the .NET `vpk` tool in the release pipeline, its own install layout and installer, replacing our MSI, app bundle and AppImage packaging. Too invasive for what we need. |
| `cargo-dist` + [`axoupdater`](https://docs.rs/axoupdater) 0.10 | Updater for binaries installed by cargo-dist's shell/PowerShell installers, driven by an "install receipt" the installer writes. | MIT OR Apache-2.0 | Built for CLI tools installed by cargo-dist's installers; requires adopting cargo-dist for the release. Deps: axoasset, axoprocess, miette, homedir, self-replace. Not a fit for a GUI app shipped as MSI, `.app` and AppImage. |
| [Tauri updater](https://v2.tauri.app/plugin/updater/) | Design reference: static `latest.json` keyed `OS-ARCH` with `url` + minisign `signature`, or a dynamic endpoint returning 204/200; signing mandatory; Windows runs the installer and the app exits, macOS replaces the `.app` from `.app.tar.gz`, Linux replaces the AppImage. | MIT/Apache-2.0 | The platform mechanics match ours exactly (MSI for installed, bundle swap on macOS, AppImage overwrite on Linux). We adopt the shape of its flow and its mandatory-TLS stance; a machine-readable manifest is not needed because the GitHub release API already is one. |
| Hand-rolled GitHub Releases client | `GET /repos/{owner}/{repo}/releases` with `ureq`, semver compare, select asset by our own names, download, verify `SHA256SUMS`, apply per platform. | n/a | About 600 lines plus tests; every path is ours to test against a local mock server; the dependency set is four small, widely audited crates. |

### Platform mechanics

**Windows.** A running `.exe` cannot be overwritten or deleted, but it can be *renamed* within
its volume. Portable install: rename `openstitch.exe` to `openstitch.exe.old`, move the new exe
into place (same directory, so the move is a rename), spawn it with the same arguments, exit; the
new process deletes `*.exe.old` at start (retry at later starts if the old process is still
exiting). This is what `self-replace` does minus the helper process. A rename can also fail when
the directory is not writable (an exe copied into `Program Files` by hand): then the update is
not applied automatically and the dialog opens the release page instead.
MSI install (`C:\Program Files\OpenStitch`): the per-machine install needs elevation, which
`msiexec` requests itself through UAC. The app downloads and verifies the new `.msi`, spawns
`msiexec /i <msi> /passive /norestart` followed by a relaunch of the installed exe (one detached
`cmd /c "msiexec ... && start "" "<exe>""`; no script file is written), and exits so the installer
can replace the file. `MajorUpgrade` in `openstitch.wxs` removes the old version. SmartScreen
does not prompt for `msiexec`; the unsigned MSI shows the usual publisher-unknown UAC prompt. The
install kind needs no registry lookup: an exe under `%ProgramFiles%\OpenStitch` means MSI,
anywhere else means portable.

**macOS.** The asset is a zip holding `OpenStitch.app`. The bundle is replaced as a whole:
extract into a temporary directory on the same volume as the bundle, rename the running
`OpenStitch.app` to `OpenStitch.app.old` (allowed while running), rename the new bundle into
place, `open -n <bundle>` (so Launch Services starts the new one even while the old process is
still alive), exit; the new process removes the `.old` bundle at start. Gatekeeper only
examines files with the `com.apple.quarantine` attribute, which browsers set through Launch
Services; a plain socket download by our own process does not set it (nor does `curl`), so the
replaced bundle starts without the right-click-Open dance, and App Translocation does not apply.
The bundle stays ad-hoc signed, as today. If `/Applications/OpenStitch.app` is not writable by
the user (installed by an admin), the dialog falls back to the release page.

**Linux.** AppImage: the runtime exports `$APPIMAGE` (the image path) and `$APPDIR`; write the
new image beside it, `chmod 755`, rename over the old path (atomic replace; the running process
keeps its mapped image), exec the new path, exit. The desktop integration written by
`--install-integration` points at the same path, so it stays valid. Tarball install: replace
`<prefix>/bin/openstitch` the same way when the prefix is writable (a user prefix such as
`~/.local`); a system prefix (`/usr/local`) is read-only for the user and falls back to the
release page. Flatpak (`/.flatpak-info` exists): no self-update, no check; the dialog is never
shown and Options says updates come from Flatpak.

**Development builds, `cargo run`, CI.** No check when `cfg!(debug_assertions)`, when the
executable lives under a `target/` directory, when `OPENSTITCH_NO_UPDATE_CHECK` is set, or on a
non-interactive start (`--screenshot`, `--script`, `--listen`, `--no-splash` and the other
options that already suppress the splash). Tests never touch the network: the client takes a
base URL, and the tests run a local `std::net` HTTP server serving canned release JSON, assets
and `SHA256SUMS`.

### Asset mapping

| Running as | Asset name pattern | Apply |
|---|---|---|
| Windows x86_64, exe not under `%ProgramFiles%` | `OpenStitch-<tag>-windows-x86_64.zip` | rename-aside and swap the exe |
| Windows x86_64, exe under `%ProgramFiles%\OpenStitch` | `OpenStitch-<tag>-windows-x86_64.msi` | `msiexec /passive`, relaunch |
| macOS (any arch; universal binary) inside `*.app` | `OpenStitch-<tag>-macos-universal.zip` | bundle swap |
| Linux x86_64 with `$APPIMAGE` | `OpenStitch-<tag>-linux-x86_64.AppImage` | replace the image |
| Linux x86_64 plain binary | `OpenStitch-<tag>-linux-x86_64.tar.gz` | replace `bin/openstitch` |
| Anything else (Windows/Linux arm64, Flatpak, macOS outside a bundle) | none | dialog shows the notes and a link only |

### Integrity

- HTTPS only, through `ureq` with `rustls`; a URL that is not `https://` is refused by the
  client, and downloads are only accepted from `github.com` / `objects.githubusercontent.com`
  (the redirect target of release assets).
- `release.yml` publishes `SHA256SUMS` (the `sha256sum` format, one line per asset) with every
  release. The updater downloads it first, then the asset, and refuses an asset whose digest does
  not match or is not listed. An older release without `SHA256SUMS` is never auto-applied.
- Signatures: GitHub serves `SHA256SUMS` over the same TLS channel as the assets, so the checksum
  guards against a corrupt or truncated download and a swapped asset, not against a compromised
  GitHub account. Minisign signatures (`minisign-verify`, MIT, no dependencies) with the public
  key compiled into the editor would close that gap; it needs a private key stored as a
  repository secret and is listed as a follow-up, not part of this feature.

### Version comparison

The workspace version is `CARGO_PKG_VERSION` (`0.1.0`); the tag carries the pre-release
(`v0.1.0-alpha.3`). The running version therefore has to be the *tag*, embedded at build time:
`build-executables.yml` already receives the label (`v0.1.0-alpha.3` or `main-<sha>`), so the
build exports `OPENSTITCH_RELEASE_TAG` and the editor reads it with `option_env!`; without it
(a local `cargo build`) the version is `CARGO_PKG_VERSION` and the check is off. Comparison is
semver with pre-release precedence: `0.1.0-alpha.3 < 0.1.0-alpha.4 < 0.1.0-beta.1 < 0.1.0 <
0.1.1`. The `semver` crate (MIT OR Apache-2.0, no dependencies; the crate cargo itself uses)
does this correctly, including numeric versus alphanumeric identifiers. Policy: a pre-release
build is offered newer pre-releases and stable releases; a stable build is offered stable
releases only, unless the user turns on "Include pre-releases" in Options.

### GitHub API, offline and failure behaviour

`GET https://api.github.com/repos/cristianonescu/OpenStitch/releases?per_page=20` lists drafts
only to authenticated owners, so an anonymous call sees published releases and pre-releases
only; `/releases/latest` is useless while every release is a pre-release (it returns 404).
Unauthenticated calls are limited to 60 per hour per IP; one call per start is far below that,
and a conditional request (`If-None-Match` with the stored `ETag`) that answers 304 does not
count. The check runs on a `std::thread` started from `EditorApp::launch`, with a 10 s global
timeout, a `User-Agent: OpenStitch/<version>` header (GitHub requires one) and
`Accept: application/vnd.github+json`. Its result is sent back over a `std::sync::mpsc` channel
that the UI polls once per frame; nothing on the UI thread waits for it. Any failure (offline,
DNS, TLS, 4xx/5xx, bad JSON) is logged at debug level and otherwise silent; the dialog only ever
appears for a successful answer that names a newer version. The result is cached in `ui.json`
(`updates.lastCheck`, `updates.etag`, `updates.latest`) so a start that happens within an hour
of the previous check reuses the answer.

### Rendering the release notes

The release body is GitHub-flavoured markdown (our `docs/releases/<tag>.md` followed by the
generated change list). `egui_commonmark` 0.26 (MIT OR Apache-2.0) targets egui 0.37, which the
editor uses, and renders headings, lists, emphasis, links, tables and code blocks in a
`ScrollArea`; images and SVG stay off (`default-features = false`, no `fetch`), so nothing in the
notes can trigger a network request. The dialog shows the notes for every release between the
running version and the latest, newest first, each under its own heading.

### Settings

In `ui.json` beside `user.json`, under `updates`:

| Key | Meaning |
|---|---|
| `check` (bool, default true) | Options ▸ "Check for updates at startup" |
| `includePrereleases` (bool, default false) | Offer alpha/beta/rc to a stable build; a pre-release build always sees them |
| `skipVersion` (string or null) | The version the user chose not to be reminded of; cleared automatically when the latest release is newer than it |
| `lastCheck`, `etag`, `latest` | Cache of the last successful check |

Options also gets a "Check for updates now" button that runs the same check and shows the dialog
with "You have the latest version" when nothing is newer.

### Restart

After applying, the new executable is started as a detached child process with the original
command line (the file that was open is reopened by passing its path), then the current process
asks eframe to close (`ctx.send_viewport_cmd(ViewportCommand::Close)`) so `on_exit` saves the
preferences normally. An unsaved design is saved to the autosave slot first, and the dialog
refuses to start the update while a design has unsaved changes until the user saves or discards.

## Decision

Hand-rolled client in a new crate `openstitch-update`, with four small dependencies:

| Crate | Why | Licence |
|---|---|---|
| `ureq` 3 (default features: `rustls`, `gzip`) | Blocking HTTPS client; no async runtime; rustls with the bundled Mozilla roots | MIT OR Apache-2.0 |
| `semver` 1 | Pre-release-correct version ordering | MIT OR Apache-2.0 |
| `sha2` 0.11 | SHA-256 of the download | MIT OR Apache-2.0 |
| `zip` 9 (deflate only) | Windows portable zip and the macOS bundle zip; the tarball through `flate2` (already in the tree) plus a minimal tar reader, or `tar` 0.4 (MIT OR Apache-2.0) | MIT |

plus `egui_commonmark` 0.26 in the editor for the notes. `self_update` is not used because the
part we would use (GitHub listing and exe swap) is the easy part, while its defaults bring
reqwest/tokio/hyper into a binary that otherwise has no async runtime, and it has no notion of
our MSI, bundle and AppImage layouts. `self-replace` stays on the shelf as a drop-in if the
Windows rename-aside approach misbehaves on some filesystem. Velopack and cargo-dist/axoupdater
would replace packaging that already works and is documented. Everything new is MIT or
Apache-2.0, compatible with the repository's `MIT OR Apache-2.0`; `THIRD_PARTY_NOTICES.md` lists
the runtime crates.

## Plan: one pull request per step

1. **Release pipeline: checksums and the build tag.** `release.yml` writes `SHA256SUMS` over
   the assets before `gh release create`; `build-executables.yml` passes the label to cargo as
   `OPENSTITCH_RELEASE_TAG` so the editor knows the tag it was built from. This document.
   Verified by a `workflow_dispatch` build and by the next release.
2. **`openstitch-update` crate: check, select, download, verify.** `Version` parsing from the
   tag, `Install` detection (portable/MSI/bundle/AppImage/tarball/flatpak/dev), release listing
   and filtering, asset selection, streaming download with progress and size limit, `SHA256SUMS`
   parsing and verification. Unit tests against a local `std::net` mock HTTP server with canned
   GitHub JSON; no network in CI. Nothing in the editor yet.
3. **Apply and restart per platform.** `apply::stage` (extract the asset into a temp dir beside
   the install), `apply::swap` per install kind, `restart`, and `cleanup_previous` for the
   `.old` leftovers at the next start, with tests on the file operations (fake exe files) on all
   three OSes. Still no UI; a hidden `--update-self` CLI path for manual testing.
4. **Editor dialog and settings.** Background check at an interactive start, the dialog with the
   notes (egui_commonmark), Dismiss, the skip-version checkbox, Update with a progress bar and
   error text, Options entries (check at startup, include pre-releases, check now), `ui.json`
   keys, dev-build and `--no-splash`-style suppression, `--show update` for screenshots.
5. **Docs.** README and user-guide section, `FEATURE_MATRIX.md`, `DISTRIBUTION.md`,
   `THIRD_PARTY_NOTICES.md`, release notes paragraph, this document's verification record.
6. **End-to-end verification.** Build at version N, publish a draft pre-release N+1 (or a
   throwaway test tag), confirm on Windows that the dialog appears with the notes, that the skip
   checkbox persists across restarts, and that Update downloads, verifies, swaps and restarts
   (portable zip, then the MSI); record the result below. macOS and Linux paths are covered by
   the unit tests and by the release smoke workflow extended with an `--update-self --dry-run`
   scene.

Each PR is rebased on `main` as the coordinator merges, keeps `cargo fmt`, `clippy -D warnings`
and the tests green on ubuntu, macos and windows, and ends with the coordinator being told it is
ready to merge.

## Verification record

**2026-10-28, Windows 11, portable install, local stand-in for GitHub.** Two release builds of
the same source were made with `OPENSTITCH_RELEASE_TAG=v0.1.0-alpha.3` and `v0.1.0-alpha.4`.
The alpha.3 exe was placed in a folder as the portable install (with an old `README.md`); a
Python `http.server` on `127.0.0.1:8765` served a release list naming alpha.4 (notes in
markdown), the alpha.4 build zipped under the real asset name and layout, and `SHA256SUMS`.
The editor was started with `OPENSTITCH_UPDATE_API` / `OPENSTITCH_UPDATE_DOWNLOADS` pointing
at it and driven with `--script` (window-point clicks on Help ▸ Check for Updates and the
dialog's buttons), because screen control was not available to the session.

| Step | Result |
|---|---|
| Startup check | The server logged `GET /repos/cristianonescu/OpenStitch/releases?per_page=20` with `User-Agent: OpenStitch/v0.1.0-alpha.3 (+https://github.com/cristianonescu/OpenStitch)` within a second of the start; no UI stall. |
| Dialog | "OpenStitch v0.1.0-alpha.4 is available", "You have v0.1.0-alpha.3", the notes rendered with heading, paragraph, bullets, bold, italic and code; checkbox, Update, Dismiss and Release page present. |
| Update | `GET …/SHA256SUMS` then `GET …/OpenStitch-v0.1.0-alpha.4-windows-x86_64.zip`; the folder then held `openstitch.exe` (SHA-256 equal to the alpha.4 build), `openstitch.exe.old` (the alpha.3 build) and the new `README.md`; the old process exited with code 0. |
| Restart | A new `openstitch.exe` process appeared and, re-running the same script, asked the server for the release list as `OpenStitch/v0.1.0-alpha.4`: the restarted editor is the new build with the original arguments. |
| Cleanup | After the old process had ended, the next start removed `openstitch.exe.old`. |
| Real GitHub, HTTPS | The alpha.4 build started without the test variables (`api.github.com` over rustls) and Help ▸ Check for Updates answered "You have the latest version (v0.1.0-alpha.4)" (the newest published release being alpha.3). |
| Skip | With "Don't remind me about this version" ticked and Dismiss clicked, `ui.json` held `"updates": {"check": true, "includePrereleases": false, "skipVersion": "v0.1.0-alpha.4"}`; a plain interactive start of the alpha.3 build then performed the check (server log) and the preference stayed as written. That the dialog stays closed for a skipped version, and reopens for a newer one, is `skipped_version_stays_quiet_until_a_newer_one` (`editor/updates.rs`). |

Not covered on a real machine: the MSI path (`msiexec /passive` with the UAC prompt; its
command line is tested as a string), macOS and Linux (stage/swap/cleanup are tested on all three
OSes in CI with fake programs and real zip/tar archives). First real-world run: the release
after the one that ships the updater, when it offers itself to that version.
