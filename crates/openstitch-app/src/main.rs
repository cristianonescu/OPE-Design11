//! `openstitch` — the OpenStitch desktop editor.
//!
//! ```text
//! openstitch [FILE] [--demo] [--select ID,ID] [--simulate FRACTION] [--tool select|point|rectangle|ellipse|polygon|pen]
//!            [--size WxH] [--zoom F] [--show realistic,reference,settings,measure,norulers,text,usb,arrange,print]
//!            [--script STEPS] [--screenshot OUT.png]
//!            [--image IMAGE [--convert autopunch|cross|photofill|photolines|designcenter]]
//!            [--pattern PATTERN.json]
//!            [--wizard background [--apply]]
//!            [--wizard background|applique [--apply]]
//!            [--wizard background|applique|patch [--apply]]
//!            [--wizard background|applique|patch|cutwork [--apply]]
//!            [--database FOLDER|FILE [--db-view large|small|details] [--db-dialog search|convert|catalog|delete|newfolder]]
//!            [--listen PORT [--listen-read-only] [--listen-no-files] [--listen-root DIR]... [--listen-read-root DIR]...]
//!            [--ribbon home|image|view|text|outline|rectangle|punch|shapes|stitches|hoop]
//!            [--pane import|color|attributes|text] [--theme light|dark|system] [--no-splash]
//! ```
//! A plain start (at most a FILE) shows the logo splash for about a second; any other option
//! above, or `--no-splash`, starts without it (see `Launch::splash_allowed`).
//! `--listen` opens the loopback automation socket on `127.0.0.1:PORT` (`0`: any free port) so
//! `stitchtool connect` and `openstitch-mcp --live` can drive this session. It prints the
//! address, the per-launch token and the endpoint file holding both. Clients get no file access
//! unless roots are given (`--listen-root` read/write, `--listen-read-root` read only).
//! `--ribbon` picks the ribbon tab and `--pane` the right-hand pane tab; `--show appmenu`
//! opens the Application menu, `--show floating` floats the side panes and `--show print`
//! opens the Print dialog (for screenshots).
//! `--database` opens the Design Database at a folder (a file is selected in its folder).
//! `--image` opens the Image to Embroidery dialog with that image chosen; `--convert` also runs
//! the conversion with that method. `--wizard` opens a pattern wizard dialog for the selection;
//! `--apply` also applies it once (the dialog stays open for screenshots).
//! A plain start also checks GitHub for a newer release and offers it (Options ▸ "Check for
//! updates at startup"; `OPENSTITCH_NO_UPDATE_CHECK` turns it off); `--show update` opens the
//! dialog with sample notes for screenshots, `--show close-prompt` the unsaved-changes prompt.
//! Closing the window, File ▸ New or Open with unsaved changes asks "Save changes to NAME?"
//! (Save, Don't Save, Cancel); a `--screenshot` run closes without asking once its image is
//! written, and a `--script` can answer the prompt (`sclick`, `key:Enter`, `key:Escape`).
//! `--screenshot` renders a few frames, writes the window contents as PNG and exits (used for
//! the documentation images and for checking the build on a platform).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod art;
mod canvas;
mod close_ui;
mod convert_ui;
mod creator_ui;
mod database_ui;
mod database_writing;
mod edit_ui;
mod icons;
mod lettering_ui;
mod panels;
mod panes;
mod pattern_wizard_ui;
mod print_ui;
mod remote;
mod ribbon;
mod script;
mod sections_ui;
mod settings_ui;
mod sewing_ui;
mod splash;
mod suite_chrome;
mod theme;
mod transfer_ui;
mod update_ui;
mod view_ui;
mod wizard_ui;
mod workspace_ui;

use app::{EditorApp, Launch};
use eframe::egui;
use openstitch_app::editor::Tool;
use std::path::PathBuf;

fn parse(args: impl Iterator<Item = String>) -> Result<(Launch, [f32; 2]), String> {
    let mut l = Launch::default();
    let mut size = [1440.0, 900.0];
    let mut listen = openstitch_remote::ServerConfig::default();
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--demo" => l.demo = true,
            "--no-splash" => l.no_splash = true,
            "--show" => {
                for v in value("--show")?.split(',') {
                    let v = v.trim().to_string();
                    if !app::SHOW.contains(&v.as_str()) {
                        return Err(format!("--show takes {}", app::SHOW.join(",")));
                    }
                    l.show.push(v);
                }
            }
            "--screenshot" => l.screenshot = Some(PathBuf::from(value("--screenshot")?)),
            "--image" => l.image = Some(PathBuf::from(value("--image")?)),
            "--pattern" => l.pattern = Some(PathBuf::from(value("--pattern")?)),
            "--database" => l.database = Some(PathBuf::from(value("--database")?)),
            "--db-view" => {
                let v = value("--db-view")?;
                l.database_view = Some(
                    openstitch_app::editor::database::View::from_key(&v)
                        .ok_or("--db-view takes large, small or details")?,
                );
            }
            "--db-dialog" => {
                let v = value("--db-dialog")?;
                if !["search", "convert", "catalog", "delete", "newfolder"].contains(&v.as_str()) {
                    return Err(
                        "--db-dialog takes search, convert, catalog, delete or newfolder".into(),
                    );
                }
                l.database_dialog = Some(v);
            }
            "--convert" => {
                let v = value("--convert")?;
                l.convert = Some(
                    openstitch_app::editor::wizard::Method::from_key(&v).ok_or_else(|| {
                        "--convert takes autopunch, cross, photofill, photolines or designcenter"
                            .to_string()
                    })?,
                );
            }
            "--wizard" => {
                let v = value("--wizard")?;
                l.wizard = Some(
                    openstitch_app::editor::pattern_wizards::WizardKind::from_key(&v)
                        .ok_or_else(|| format!("unknown wizard '{v}'"))?,
                );
            }
            "--apply" => l.apply_wizard = true,
            "--ribbon" => {
                let v = value("--ribbon")?;
                l.ribbon = Some(
                    openstitch_app::editor::ribbon::TabId::from_key(&v).ok_or(
                        "--ribbon takes home, image, view, text, outline, rectangle, punch, shapes, stitches or hoop",
                    )?,
                );
            }
            "--pane" => {
                let v = value("--pane")?;
                l.pane = Some(
                    ribbon::PaneTab::from_key(&v)
                        .ok_or("--pane takes import, color, attributes or text")?,
                );
            }
            "--theme" => {
                let v = value("--theme")?;
                l.theme = Some(
                    openstitch_app::editor::prefs::Theme::from_key(&v)
                        .ok_or("--theme takes light, dark or system")?,
                );
            }
            "--select" => {
                l.select = value("--select")?
                    .split(',')
                    .map(|s| s.trim().parse().map_err(|_| format!("bad id '{s}'")))
                    .collect::<Result<_, _>>()?
            }
            "--simulate" => {
                l.simulate = Some(
                    value("--simulate")?
                        .parse()
                        .map_err(|_| "--simulate takes a fraction 0-1")?,
                )
            }
            "--tool" => {
                let t = value("--tool")?;
                l.tool = Some(
                    Tool::ALL
                        .into_iter()
                        .find(|x| x.label().eq_ignore_ascii_case(&t))
                        .ok_or_else(|| format!("unknown tool '{t}'"))?,
                );
            }
            "--size" => {
                let v = value("--size")?;
                let (w, h) = v.split_once('x').ok_or("--size takes WxH")?;
                size = [
                    w.parse().map_err(|_| "bad width")?,
                    h.parse().map_err(|_| "bad height")?,
                ];
            }
            "--script" => l.script = script::parse(&value("--script")?)?,
            "--zoom" => {
                l.zoom = Some(
                    value("--zoom")?
                        .parse()
                        .map_err(|_| "--zoom takes a factor")?,
                )
            }
            "--listen" => {
                listen.port = value("--listen")?
                    .parse()
                    .map_err(|_| "--listen takes a port (0: any free port)")?;
                l.listen = Some(Default::default());
            }
            "--listen-read-only" => listen.capabilities = listen.capabilities.read_only(),
            "--listen-no-files" => listen.capabilities = listen.capabilities.without_files(),
            "--listen-root" => listen.write_roots.push(value("--listen-root")?.into()),
            "--listen-read-root" => listen.read_roots.push(value("--listen-read-root")?.into()),
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with('-') => return Err(format!("unknown option {s}")),
            file => l.open = Some(PathBuf::from(file)),
        }
    }
    match &mut l.listen {
        Some(config) => *config = listen,
        None if listen_options_given(&listen) => {
            return Err("--listen-* options need --listen PORT".into());
        }
        None => {}
    }
    Ok((l, size))
}

/// Whether any `--listen-*` option changed the default socket settings.
fn listen_options_given(c: &openstitch_remote::ServerConfig) -> bool {
    let d = openstitch_remote::ServerConfig::default();
    c.capabilities != d.capabilities || !c.read_roots.is_empty() || !c.write_roots.is_empty()
}

/// Starts the automation socket and prints how clients reach it.
fn listen(
    mut config: openstitch_remote::ServerConfig,
) -> Result<openstitch_remote::Server, String> {
    config.endpoint_file = openstitch_remote::default_endpoint_file();
    let server = openstitch_remote::Server::start(config).map_err(|e| format!("--listen: {e}"))?;
    eprintln!("openstitch: automation socket on {}", server.address());
    eprintln!("openstitch: token {}", server.token());
    if let Some(file) = server.endpoint_file() {
        eprintln!("openstitch: endpoint file {}", file.display());
    }
    Ok(server)
}

/// The app icon (assets/icons, written by tools/gen_icons.py from the logo) as 64×64 RGBA.
fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: include_bytes!("../../../assets/icons/openstitch-64.rgba").to_vec(),
        width: 64,
        height: 64,
    }
}

fn main() -> eframe::Result {
    let (mut launch, size) = match parse(std::env::args().skip(1)) {
        Ok(v) => v,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("openstitch: {e}");
            }
            eprintln!(
                "usage: openstitch [FILE] [--demo] [--select ID,ID] [--simulate F] [--tool NAME] [--size WxH] [--zoom F] [--show VIEWS] [--script STEPS] [--screenshot OUT.png] [--image IMAGE [--convert METHOD]] [--pattern PATTERN.json] [--wizard NAME [--apply]] [--database PATH [--db-view VIEW] [--db-dialog NAME]] [--listen PORT [--listen-read-only] [--listen-no-files] [--listen-root DIR] [--listen-read-root DIR]] [--ribbon TAB] [--pane TAB] [--theme THEME] [--no-splash]"
            );
            std::process::exit(2);
        }
    };
    launch.user_settings = openstitch_engine::user::default_path();
    // The automation socket is taken out of `launch` below; it never gets a splash either.
    launch.no_splash |= launch.listen.is_some();
    let remote = match launch.listen.take().map(listen).transpose() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("openstitch: {e}");
            std::process::exit(2);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([900.0, 600.0])
            .with_title("OpenStitch")
            .with_icon(window_icon()),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_options(),
        ..Default::default()
    };
    eframe::run_native(
        "OpenStitch",
        options,
        Box::new(|cc| {
            let mut app = EditorApp::new(launch);
            if let Some(server) = &remote {
                // Requests arrive while the UI may be idle; repaint so it drains them.
                let ctx = cc.egui_ctx.clone();
                server.set_waker(move || ctx.request_repaint());
            }
            app.remote = remote;
            Ok(Box::new(app))
        }),
    )
}

/// wgpu's default backends, except on Windows: DirectX 12, then OpenGL. Some Windows Vulkan
/// drivers crash while wgpu starts (seen on Windows 11 with every launch), and DirectX 12 is
/// the system's own API. `WGPU_BACKEND` (e.g. `vulkan`) still chooses any backend.
fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    #[allow(unused_mut)]
    let mut config = eframe::egui_wgpu::WgpuConfiguration::default();
    #[cfg(windows)]
    if std::env::var_os("WGPU_BACKEND").is_none()
        && let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut config.wgpu_setup
    {
        setup.instance_descriptor.backends =
            eframe::wgpu::Backends::DX12 | eframe::wgpu::Backends::GL;
    }
    config
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn args(s: &str) -> impl Iterator<Item = String> {
        s.split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parses_launch_options() {
        let (l, size) = parse(args("a.osd --demo --select 1,3 --simulate 0.5 --tool polygon --size 800x600 --zoom 2 --script click:1,2 --screenshot o.png")).expect("ok");
        assert_eq!((l.zoom, l.script.len()), (Some(2.0), 1));
        assert_eq!(l.open.as_deref(), Some(std::path::Path::new("a.osd")));
        assert!(l.demo);
        assert_eq!(l.select, [1, 3]);
        assert_eq!(l.simulate, Some(0.5));
        assert_eq!(l.tool, Some(openstitch_app::editor::Tool::Polygon));
        assert_eq!(size, [800.0, 600.0]);
        assert!(parse(args("--bogus")).is_err());
        let (l, _) = parse(args("--show realistic,reference")).expect("show");
        assert_eq!(l.show, ["realistic", "reference"]);
        assert!(parse(args("--show sparkles")).is_err());
        assert!(parse(args("--select x")).is_err());
        let (l, _) = parse(args("--wizard background --apply")).expect("wizard");
        assert!(l.wizard.is_some() && l.apply_wizard);
        assert!(parse(args("--wizard sparkle")).is_err());
        assert!(parse(args("--screenshot")).is_err());
        let (l, _) =
            parse(args("--database /tmp --db-view details --db-dialog search")).expect("db");
        assert_eq!(l.database.as_deref(), Some(std::path::Path::new("/tmp")));
        assert_eq!(
            l.database_view,
            Some(openstitch_app::editor::database::View::Details)
        );
        assert!(parse(args("--db-view huge")).is_err());
        assert!(parse(args("--db-dialog format")).is_err());
        let (l, _) = parse(args(
            "--ribbon view --pane color --theme dark --show appmenu",
        ))
        .expect("ribbon");
        assert_eq!(l.ribbon, Some(openstitch_app::editor::ribbon::TabId::View));
        assert_eq!(l.pane, Some(crate::ribbon::PaneTab::Color));
        assert_eq!(l.theme, Some(openstitch_app::editor::prefs::Theme::Dark));
        assert!(parse(args("--ribbon file")).is_err() && parse(args("--theme neon")).is_err());
    }

    #[test]
    fn parses_listen_options() {
        assert!(parse(args("")).expect("ok").0.listen.is_none());
        let (l, _) = parse(args(
            "--listen-read-only --listen 0 --listen-root /tmp --listen-read-root /var",
        ))
        .expect("listen");
        let c = l.listen.expect("config");
        assert_eq!(c.port, 0);
        assert!(!c.capabilities.edit && c.capabilities.read_files);
        assert_eq!(c.write_roots, [std::path::PathBuf::from("/tmp")]);
        assert_eq!(c.read_roots, [std::path::PathBuf::from("/var")]);
        assert_eq!(
            parse(args("--listen 4100 --listen-no-files"))
                .expect("ok")
                .0
                .listen
                .map(|c| (c.port, c.capabilities.read_files)),
            Some((4100, false))
        );
        assert!(parse(args("--listen")).is_err());
        // The splash: a plain start or a file only; never automation or a set-up view.
        assert!(parse(args("a.pes")).expect("ok").0.splash_allowed());
        assert!(
            !parse(args("a.pes --no-splash"))
                .expect("ok")
                .0
                .splash_allowed()
        );
        for a in [
            "--screenshot o.png",
            "--script click:1,2",
            "--listen 0",
            "--demo",
            "--show appmenu",
            "--database /tmp",
            "--ribbon view",
        ] {
            assert!(!parse(args(a)).expect("ok").0.splash_allowed(), "{a}");
        }
        assert!(parse(args("--listen x")).is_err());
        assert!(parse(args("--listen-no-files")).is_err());
    }
}
