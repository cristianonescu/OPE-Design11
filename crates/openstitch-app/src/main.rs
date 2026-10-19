//! `openstitch` — the OpenStitch desktop editor.
//!
//! ```text
//! openstitch [FILE] [--demo] [--select ID,ID] [--simulate FRACTION] [--tool select|point|rectangle|ellipse|polygon|pen]
//!            [--size WxH] [--zoom F] [--show realistic,reference,settings,measure,norulers,text,usb]
//!            [--script STEPS] [--screenshot OUT.png]
//!            [--image IMAGE [--convert autopunch|cross|photofill|photolines|designcenter]]
//!            [--pattern PATTERN.json]
//!            [--wizard background [--apply]]
//!            [--wizard background|applique [--apply]]
//!            [--wizard background|applique|patch [--apply]]
//!            [--wizard background|applique|patch|cutwork [--apply]]
//!            [--database FOLDER|FILE [--db-view large|small|details] [--db-dialog search|convert|catalog|delete|newfolder]]
//! ```
//! `--database` opens the Design Database at a folder (a file is selected in its folder).
//! `--image` opens the Image to Embroidery dialog with that image chosen; `--convert` also runs
//! the conversion with that method. `--wizard` opens a pattern wizard dialog for the selection;
//! `--apply` also applies it once (the dialog stays open for screenshots).
//! `--screenshot` renders a few frames, writes the window contents as PNG and exits (used for
//! the documentation images and for checking the build on a platform).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod convert_ui;
mod creator_ui;
mod database_ui;
mod edit_ui;
mod lettering_ui;
mod panels;
mod pattern_wizard_ui;
mod print_ui;
mod script;
mod sections_ui;
mod settings_ui;
mod sewing_ui;
mod transfer_ui;
mod wizard_ui;
mod workspace_ui;

use app::{EditorApp, Launch};
use eframe::egui;
use openstitch_app::editor::Tool;
use std::path::PathBuf;

fn parse(args: impl Iterator<Item = String>) -> Result<(Launch, [f32; 2]), String> {
    let mut l = Launch::default();
    let mut size = [1440.0, 900.0];
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--demo" => l.demo = true,
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
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with('-') => return Err(format!("unknown option {s}")),
            file => l.open = Some(PathBuf::from(file)),
        }
    }
    Ok((l, size))
}

/// The app icon (assets/icons, drawn by tools/gen_icons.py) as 64×64 RGBA.
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
                "usage: openstitch [FILE] [--demo] [--select ID,ID] [--simulate F] [--tool NAME] [--size WxH] [--zoom F] [--show VIEWS] [--script STEPS] [--screenshot OUT.png] [--image IMAGE [--convert METHOD]] [--pattern PATTERN.json] [--wizard NAME [--apply]] [--database PATH [--db-view VIEW] [--db-dialog NAME]]"
            );
            std::process::exit(2);
        }
    };
    launch.user_settings = openstitch_engine::user::default_path();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([900.0, 600.0])
            .with_title("OpenStitch")
            .with_icon(window_icon()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "OpenStitch",
        options,
        Box::new(|_cc| Ok(Box::new(EditorApp::new(launch)))),
    )
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
    }
}
