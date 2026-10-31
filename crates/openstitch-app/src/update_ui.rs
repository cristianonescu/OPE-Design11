//! Self-update: the startup check, the "update available" dialog with the release notes,
//! Dismiss / "Don't remind me about this version" / Update with its progress, and the restart
//! (`docs/research/updater.md`). All network and file work runs on its own thread; the UI only
//! polls a channel once per frame.

use crate::app::EditorApp;
use eframe::egui::{self, RichText};
use openstitch_app::editor::updates::{NO_CHECK_ENV, UPDATED_FROM_ENV, restart_args};
use openstitch_update as up;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};

enum Phase {
    Idle,
    Checking(Receiver<Result<Option<up::Update>, String>>),
    Available(up::Update),
    /// A check the user asked for found nothing newer.
    UpToDate,
    /// This build cannot check (no release tag, Flatpak, development build): the reason.
    Unavailable(String),
    Downloading {
        update: up::Update,
        rx: Receiver<Result<up::Staged, String>>,
        progress: Arc<Mutex<(u64, Option<u64>)>>,
        cancel: Arc<AtomicBool>,
    },
    /// The new version is in place and started; this process is closing.
    Restarting,
    Failed {
        update: Option<up::Update>,
        error: String,
    },
}

pub struct UpdateState {
    /// The release this build is, or `None` for a build without a release tag.
    pub current: Option<up::Current>,
    pub install: up::Install,
    /// Set by `launch` on an interactive start; the first frame starts the check.
    pub check_at_start: bool,
    /// A check requested (`Some(manual)`), started on the next frame.
    wanted: Option<bool>,
    manual: bool,
    phase: Phase,
    pub open: bool,
    /// "Don't remind me about this version".
    skip: bool,
    cache: egui_commonmark::CommonMarkCache,
    /// The tag this process was updated from (status line once).
    updated_from: Option<String>,
}

impl Default for UpdateState {
    fn default() -> Self {
        let install = up::detect();
        up::cleanup_previous(&install);
        UpdateState {
            current: up::current(),
            install,
            check_at_start: false,
            wanted: None,
            manual: false,
            phase: Phase::Idle,
            open: false,
            skip: false,
            cache: egui_commonmark::CommonMarkCache::default(),
            updated_from: std::env::var(UPDATED_FROM_ENV)
                .ok()
                .filter(|t| up::is_release_tag(t)),
        }
    }
}

impl UpdateState {
    /// Asks for a check: `manual` (Options ▸ Check for Updates Now) also reports "up to date"
    /// and errors; the startup check is silent unless something newer exists.
    pub fn check(&mut self, manual: bool) {
        self.wanted = Some(manual);
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.phase,
            Phase::Checking(_) | Phase::Downloading { .. } | Phase::Restarting
        )
    }

    /// `--show update`: the dialog with sample notes, for screenshots; nothing is fetched.
    pub fn show_sample(&mut self) {
        let tag = "v0.1.0-alpha.4";
        let Some(version) = up::parse_tag(tag) else {
            return;
        };
        let download = format!("{}{tag}/", up::DOWNLOAD_PREFIX);
        let asset = |name: &str| up::Asset {
            name: name.into(),
            url: format!("{download}{name}"),
            size: 12_000_000,
        };
        let release = up::Release {
            tag: tag.into(),
            version,
            name: format!("OpenStitch {tag}"),
            prerelease: true,
            body: SAMPLE_NOTES.into(),
            html_url: format!("https://github.com/{}/releases/tag/{tag}", up::REPO),
            published_at: "2026-11-02T10:00:00Z".into(),
            assets: vec![
                asset(&format!("OpenStitch-{tag}-windows-x86_64.zip")),
                asset("SHA256SUMS"),
            ],
        };
        self.phase = Phase::Available(up::Update {
            asset: release.assets.first().cloned(),
            checksums: release.assets.get(1).cloned(),
            notes: vec![release.clone()],
            release,
        });
        self.manual = false;
        self.skip = false;
        self.open = true;
    }

    fn start_check(&mut self, manual: bool, include_prereleases: bool, ctx: &egui::Context) {
        if self.is_busy() {
            return;
        }
        if !manual && std::env::var_os(NO_CHECK_ENV).is_some() {
            return;
        }
        self.manual = manual;
        let Some(current) = self.current.clone() else {
            if manual {
                self.unavailable(
                    "This build has no release tag (a development build), so it cannot check for updates.",
                );
            }
            return;
        };
        if !self.install.checks() {
            if manual {
                let why = match self.install {
                    up::Install::Flatpak => {
                        "This OpenStitch is a Flatpak: updates come from the Flatpak remote (flatpak update)."
                    }
                    _ => "A development build does not check for updates.",
                };
                self.unavailable(why);
            }
            return;
        }
        let (tx, rx) = channel();
        let install = self.install.clone();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                let client = up::Client::github(&current.tag);
                let result = match client.releases(up::REPO, None) {
                    Ok(up::Fetched::Releases { list, .. }) => Ok(up::find_update(
                        &list,
                        &current.version,
                        include_prereleases,
                        &install,
                    )),
                    Ok(up::Fetched::NotModified) => Ok(None),
                    Err(e) => Err(e.to_string()),
                };
                let _ = tx.send(result);
                ctx.request_repaint();
            })
            .map(drop)
            .unwrap_or_else(|e| eprintln!("openstitch: update check thread: {e}"));
        self.phase = Phase::Checking(rx);
    }

    fn start_download(&mut self, update: up::Update, ctx: &egui::Context) {
        let (Some(asset), Some(sums)) = (update.asset.clone(), update.checksums.clone()) else {
            self.fail(Some(update), "No package for this system in the release.");
            return;
        };
        let Some(current) = self.current.clone() else {
            self.fail(Some(update), "This build cannot update itself.");
            return;
        };
        let (tx, rx) = channel();
        let progress = Arc::new(Mutex::new((0u64, None)));
        let cancel = Arc::new(AtomicBool::new(false));
        let (p, c, install, ctx) = (
            progress.clone(),
            cancel.clone(),
            self.install.clone(),
            ctx.clone(),
        );
        let spawned = std::thread::Builder::new()
            .name("update-download".into())
            .spawn(move || {
                let result = download_and_stage(&current.tag, &asset, &sums, &install, &c, &p);
                let _ = tx.send(result.map_err(|e| e.to_string()));
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => {
                self.phase = Phase::Downloading {
                    update,
                    rx,
                    progress,
                    cancel,
                }
            }
            Err(e) => self.fail(Some(update), &format!("cannot start the download: {e}")),
        }
    }

    fn unavailable(&mut self, why: &str) {
        self.phase = Phase::Unavailable(why.to_string());
        self.open = true;
    }

    fn fail(&mut self, update: Option<up::Update>, error: &str) {
        self.phase = Phase::Failed {
            update,
            error: error.to_string(),
        };
        self.open = true;
    }
}

/// Checksum file, asset, verification and staging: the download thread.
fn download_and_stage(
    tag: &str,
    asset: &up::Asset,
    sums: &up::Asset,
    install: &up::Install,
    cancel: &AtomicBool,
    progress: &Mutex<(u64, Option<u64>)>,
) -> Result<up::Staged, Box<dyn std::error::Error + Send + Sync>> {
    let client = up::Client::github(tag);
    let text = client.text(&sums.url)?;
    let checksums = up::Checksums::parse(&text)?;
    let digest = checksums
        .get(&asset.name)
        .ok_or_else(|| format!("{} is not listed in SHA256SUMS", asset.name))?;
    let dir = up::download_dir();
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(&asset.name);
    client.download(
        &asset.url,
        &dest,
        Some(digest),
        up::MAX_ASSET_BYTES,
        cancel,
        &mut |done, total| {
            if let Ok(mut p) = progress.lock() {
                *p = (done, total);
            }
        },
    )?;
    let staged = up::stage(install, &dest)?;
    if !matches!(staged, up::Staged::Msi { .. }) {
        let _ = std::fs::remove_file(&dest);
    }
    Ok(staged)
}

/// Once per frame: starts a requested check, takes the threads' answers, restarts when the
/// new version is staged.
pub fn poll(app: &mut EditorApp, ctx: &egui::Context) {
    let st = &mut app.update;
    if let Some(from) = st.updated_from.take() {
        let to = st
            .current
            .as_ref()
            .map_or_else(|| "this version".to_string(), |c| c.tag.clone());
        app.status = format!("Updated from {from} to {to}");
    }
    if st.check_at_start {
        st.check_at_start = false;
        st.wanted.get_or_insert(false);
    }
    if let Some(manual) = st.wanted.take() {
        st.start_check(manual, app.ui.prefs.updates.include_prereleases, ctx);
    }
    let mut restarting = false;
    match &st.phase {
        Phase::Checking(rx) => match rx.try_recv() {
            Ok(Ok(Some(update))) => {
                let (show, changed) = app.ui.prefs.updates.offer(update.tag(), st.manual);
                if changed {
                    app.ui.save_prefs();
                }
                if show {
                    st.skip = false;
                    st.phase = Phase::Available(update);
                    st.open = true;
                } else {
                    st.phase = Phase::Idle;
                }
            }
            Ok(Ok(None)) => {
                if st.manual {
                    st.phase = Phase::UpToDate;
                    st.open = true;
                } else {
                    st.phase = Phase::Idle;
                }
            }
            Ok(Err(e)) => {
                if st.manual {
                    st.fail(None, &e);
                } else {
                    st.phase = Phase::Idle;
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => st.phase = Phase::Idle,
        },
        Phase::Downloading { rx, update, .. } => {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            match rx.try_recv() {
                Ok(Ok(staged)) => {
                    let from = st
                        .current
                        .as_ref()
                        .map(|c| c.tag.clone())
                        .unwrap_or_default();
                    let result = up::swap(&staged).map_err(|e| e.to_string()).and_then(|()| {
                        let launch = up::launch(&staged, &restart_args());
                        up::restart(&launch, &[(UPDATED_FROM_ENV, from.as_str())])
                            .map_err(|e| format!("the new version did not start: {e}"))
                    });
                    match result {
                        Ok(()) => {
                            st.phase = Phase::Restarting;
                            restarting = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(e) => {
                            let u = update.clone();
                            st.fail(Some(u), &e);
                        }
                    }
                }
                Ok(Err(e)) => {
                    let u = update.clone();
                    let msg = if e == "cancelled" {
                        "The download was cancelled.".to_string()
                    } else {
                        e
                    };
                    st.fail(Some(u), &msg);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    let u = update.clone();
                    st.fail(Some(u), "the download stopped unexpectedly");
                }
            }
        }
        _ => {}
    }
    if restarting {
        // The new version starts; Update is disabled while the design is dirty, so nothing
        // is lost and the close goes through without the prompt.
        app.approve_exit();
    }
}

/// "Version 0.1.0 (v0.1.0-alpha.3)" for About and the dialog.
pub fn version_line() -> String {
    match up::current() {
        Some(c) => format!("Version {} ({})", env!("CARGO_PKG_VERSION"), c.tag),
        None => format!("Version {} (development build)", env!("CARGO_PKG_VERSION")),
    }
}

enum Click {
    None,
    Dismiss,
    Update(up::Update),
    Retry(up::Update),
    Cancel,
}

/// The update dialog.
pub fn window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.update.open {
        return;
    }
    let dirty = app.is_dirty();
    let st = &mut app.update;
    let mut open = true;
    let mut click = Click::None;
    let current = st
        .current
        .as_ref()
        .map_or_else(version_line, |c| c.tag.clone());
    egui::Window::new("OpenStitch Update")
        .open(&mut open)
        .collapsible(false)
        .default_width(600.0)
        .default_height(480.0)
        .default_pos(ctx.content_rect().center() - egui::vec2(300.0, 240.0))
        .show(ctx, |ui| match &st.phase {
            Phase::Available(update) => {
                crate::art::header(ui, crate::art::Art::UpdateAvailable, |ui| {
                    ui.heading(format!("OpenStitch {} is available", update.tag()));
                    ui.label(RichText::new(format!("You have {current}.")).weak());
                });
                if !update.applicable() {
                    ui.add_space(4.0);
                    let why = match &st.install {
                        up::Install::Flatpak => "This OpenStitch is a Flatpak: update it with flatpak update.".to_string(),
                        up::Install::Development => "A development build does not update itself; build or download the new version.".to_string(),
                        up::Install::Unsupported(what) => format!("No package for {what} in this release: download it from the release page."),
                        _ => "This release has no package for this system yet.".to_string(),
                    };
                    ui.label(RichText::new(why).color(ui.visuals().warn_fg_color));
                }
                ui.separator();
                let notes_height = ui.available_height() - 64.0;
                egui::ScrollArea::vertical()
                    .max_height(notes_height.max(120.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (i, r) in update.notes.iter().enumerate() {
                            if i > 0 {
                                ui.separator();
                            }
                            if update.notes.len() > 1 {
                                ui.strong(if r.name.is_empty() { &r.tag } else { &r.name });
                            }
                            if r.body.trim().is_empty() {
                                ui.label(RichText::new("No release notes.").weak());
                            } else {
                                egui_commonmark::CommonMarkViewer::new().show(
                                    ui,
                                    &mut st.cache,
                                    &r.body,
                                );
                            }
                        }
                    });
                ui.separator();
                ui.checkbox(&mut st.skip, "Don't remind me about this version")
                    .on_hover_text("Stays quiet about this version; a newer one is offered again");
                ui.horizontal(|ui| {
                    let can = update.applicable() && !dirty;
                    let b = ui.add_enabled(can, egui::Button::new("Update"));
                    let b = if dirty {
                        b.on_disabled_hover_text("Save or close your design first: the editor restarts after the update")
                    } else {
                        b.on_hover_text("Downloads the release, checks it, installs it and restarts OpenStitch")
                    };
                    if b.clicked() {
                        click = Click::Update(update.clone());
                    }
                    if ui.button("Dismiss").clicked() {
                        click = Click::Dismiss;
                    }
                    if !update.release.html_url.is_empty() {
                        ui.hyperlink_to("Release page", &update.release.html_url);
                    }
                });
                if dirty {
                    ui.label(
                        RichText::new("Save your design before updating.")
                            .weak()
                            .small(),
                    );
                }
            }
            Phase::Downloading {
                update, progress, ..
            } => {
                let name = update
                    .asset
                    .as_ref()
                    .map_or("the update", |a| a.name.as_str());
                crate::art::header(ui, crate::art::Art::UpdateDownloading, |ui| {
                    ui.label(format!("Downloading {name}…"));
                });
                let (done, total) = progress.lock().map(|p| *p).unwrap_or((0, None));
                let bar = match total {
                    Some(t) if t > 0 => egui::ProgressBar::new(done as f32 / t as f32)
                        .show_percentage()
                        .text(format!("{} / {}", mb(done), mb(t))),
                    _ => egui::ProgressBar::new(0.0).animate(true).text(mb(done)),
                };
                ui.add(bar);
                ui.label(
                    RichText::new("The download is checked against the release's SHA-256 sums before it is installed.")
                        .weak()
                        .small(),
                );
                if ui.button("Cancel").clicked() {
                    click = Click::Cancel;
                }
            }
            Phase::Restarting => {
                crate::art::header(ui, crate::art::Art::UpdateRestarting, |ui| {
                    ui.label("Update installed. Restarting OpenStitch…");
                    ui.spinner();
                });
            }
            Phase::UpToDate => {
                crate::art::header(ui, crate::art::Art::UpdateCurrent, |ui| {
                    ui.label(format!("You have the latest version ({current})."));
                });
                if ui.button("Close").clicked() {
                    click = Click::Dismiss;
                }
            }
            Phase::Unavailable(why) => {
                crate::art::header(ui, crate::art::Art::UpdateUnavailable, |ui| {
                    ui.label(RichText::new("Updates are not available for this build.").strong());
                    ui.label(why);
                });
                if ui.button("Close").clicked() {
                    click = Click::Dismiss;
                }
            }
            Phase::Failed { update, error } => {
                crate::art::header(ui, crate::art::Art::UpdateFailed, |ui| {
                    ui.label(RichText::new("The update did not go through.").strong());
                    ui.label(error);
                });
                ui.horizontal(|ui| {
                    if let Some(u) = update
                        && ui.button("Try again").clicked()
                    {
                        click = Click::Retry(u.clone());
                    }
                    if ui.button("Close").clicked() {
                        click = Click::Dismiss;
                    }
                    if let Some(u) = update
                        && !u.release.html_url.is_empty()
                    {
                        ui.hyperlink_to("Release page", &u.release.html_url);
                    }
                });
            }
            Phase::Idle | Phase::Checking(_) => {
                ui.label("Checking for updates…");
                ui.spinner();
            }
        });
    if !open && !matches!(st.phase, Phase::Restarting) {
        click = match st.phase {
            Phase::Downloading { .. } => Click::Cancel,
            _ => Click::Dismiss,
        };
    }
    match click {
        Click::None => {}
        Click::Dismiss => {
            if let Phase::Available(u) = &st.phase
                && st.skip
            {
                app.ui.prefs.updates.skip_version = Some(u.tag().to_string());
                app.ui.save_prefs();
            }
            st.phase = Phase::Idle;
            st.open = false;
        }
        Click::Update(u) | Click::Retry(u) => st.start_download(u, ctx),
        Click::Cancel => {
            if let Phase::Downloading { cancel, .. } = &st.phase {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    }
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

const SAMPLE_NOTES: &str = "# OpenStitch 0.1.0 alpha 4

**Sample notes for the documentation screenshot.** A real release shows the notes from
`docs/releases/<tag>.md` followed by GitHub's list of changes.

## What changed

- Fill, satin and lines sew closer to PE-DESIGN 11's defaults.
- The editor checks for updates at startup and installs them with one click.
- Design Database thumbnails render faster.

## Before you sew

Test on scrap fabric first: this is an alpha.
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Launch;

    #[test]
    fn development_builds_never_start_a_check_and_the_sample_dialog_opens() {
        let mut app = EditorApp::new(Launch::default());
        assert!(
            !app.update.check_at_start,
            "tests run as a development build"
        );
        assert_eq!(app.update.install, up::Install::Development);
        assert!(!app.update.open);
        app.update.show_sample();
        assert!(app.update.open);
        let Phase::Available(u) = &app.update.phase else {
            panic!("sample is an available update");
        };
        assert_eq!(u.tag(), "v0.1.0-alpha.4");
        assert!(u.applicable(), "the sample shows an enabled Update button");
        assert!(version_line().starts_with("Version "));

        // Help ▸ Check for Updates on a build without a release tag: not a failure.
        let mut app = EditorApp::new(Launch::default());
        app.update
            .start_check(true, false, &egui::Context::default());
        assert!(
            matches!(&app.update.phase, Phase::Unavailable(why) if why.contains("no release tag")),
            "{:?}",
            matches!(app.update.phase, Phase::Failed { .. })
        );
        assert!(app.update.open);
    }

    #[test]
    fn a_launch_with_options_does_not_check() {
        let l = Launch {
            screenshot: Some("x.png".into()),
            ..Launch::default()
        };
        assert!(!l.interactive());
        let l = Launch {
            no_splash: true,
            ..Launch::default()
        };
        assert!(l.interactive() && !l.splash_allowed());
        assert!(Launch::default().interactive());
    }
}
