//! Window chrome of the other suite windows, drawn with the editor's ribbon pieces
//! ([`crate::ribbon`]): the Application button and menu, Quick Access Toolbar, tabs, groups and
//! ScreenTips of the Programmable Stitch Creator and Stitch Design Factory, and the menu bar and
//! toolbar of the Design Database. The layouts are in [`openstitch_app::editor::suite`]; each
//! window says through [`Host`] which commands are enabled or on and how they run.

use crate::ribbon::{self, MAC};
use eframe::egui::{self, RichText};
use openstitch_app::editor::suite::{Command, Kind, MenuWindow, RibbonWindow};

/// What a window does with its commands.
pub trait Host<A: Command> {
    /// Whether `a` can run now (only asked for commands OpenStitch implements).
    fn enabled(&self, a: A) -> bool;
    /// Whether an on/off command or a tool is on.
    fn active(&self, a: A) -> bool;
    fn run(&mut self, a: A, ctx: &egui::Context);
    /// The list a menu command opens.
    fn menu(&mut self, a: A, ui: &mut egui::Ui);
    /// ScreenTip of the command under the pointer, for the status bar.
    fn set_hover(&mut self, tip: &'static str);
}

pub fn enabled<A: Command, H: Host<A>>(h: &H, a: A) -> bool {
    a.info().available && h.enabled(a)
}

/// The icon of `a`: its own when it has one, else its editor command's.
fn icon<A: Command>(a: A) -> crate::icons::Icon {
    crate::icons::Icon::Suite(a.info().key, a.icon())
}

/// A ribbon button of a suite window.
pub fn command<A: Command, H: Host<A>>(h: &mut H, ui: &mut egui::Ui, a: A, large: bool) {
    let info = a.info();
    let on = enabled(h, a);
    let pressed = a.kind() == Kind::Toggle && on && h.active(a);
    let resp = ribbon::button(ui, &info, icon(a), large, on, pressed, a.is_menu());
    after_click(h, ui, a, on, &resp);
}

fn after_click<A: Command, H: Host<A>>(
    h: &mut H,
    ui: &egui::Ui,
    a: A,
    on: bool,
    resp: &egui::Response,
) {
    if resp.hovered() {
        h.set_hover(a.info().tip);
    }
    if a.is_menu() {
        if on {
            egui::Popup::menu(resp).show(|ui| h.menu(a, ui));
        }
    } else if resp.clicked() && on {
        h.run(a, ui.ctx());
    }
}

/// A command as a menu row: label, shortcut, check mark when on; a submenu for a menu command.
pub fn menu_item<A: Command, H: Host<A>>(h: &mut H, ui: &mut egui::Ui, a: A) {
    let info = a.info();
    let on = enabled(h, a);
    let tip = info.screen_tip(MAC);
    if a.is_menu() {
        ui.add_enabled_ui(on, |ui| {
            ui.menu_button(info.label, |ui| h.menu(a, ui))
                .response
                .on_hover_text(&tip)
                .on_disabled_hover_text(&tip);
        });
        return;
    }
    let mut b = egui::Button::new(info.label);
    if a.kind() == Kind::Toggle {
        b = b.selected(on && h.active(a));
    }
    if let Some(s) = info.shortcut_for(MAC) {
        b = b.shortcut_text(s);
    }
    let r = ui
        .add_enabled(on, b)
        .on_hover_text(&tip)
        .on_disabled_hover_text(&tip);
    if r.clicked() {
        h.run(a, ui.ctx());
        ui.close();
    }
}

/// Runs the commands whose shortcut was pressed (shifted shortcuts first). Copy, Cut and Paste
/// arrive as clipboard events.
pub fn shortcuts<A: Command, H: Host<A>>(h: &mut H, ctx: &egui::Context, actions: &[A]) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let events = ctx.input(|i| i.events.clone());
    let mut keyed: Vec<(A, &str)> = actions
        .iter()
        .filter(|a| !a.is_menu())
        .filter_map(|&a| a.info().shortcut.map(|s| (a, s)))
        .collect();
    keyed.sort_by_key(|(_, s)| std::cmp::Reverse(s.matches('+').count()));
    for (a, s) in keyed {
        let hit = match s {
            "Ctrl+C" => events.iter().any(|e| matches!(e, egui::Event::Copy)),
            "Ctrl+X" => events.iter().any(|e| matches!(e, egui::Event::Cut)),
            "Ctrl+V" => events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            _ => parse_shortcut(s).is_some_and(|k| ctx.input_mut(|i| i.consume_shortcut(&k))),
        };
        if hit {
            h.run(a, ctx);
        }
    }
}

/// "Ctrl+Shift+S" as a shortcut (Ctrl is Cmd on macOS).
fn parse_shortcut(s: &str) -> Option<egui::KeyboardShortcut> {
    let mut parts: Vec<&str> = s.split('+').collect();
    // "Ctrl+=" splits into "Ctrl", "="; "Ctrl+-" keeps "-" as the key.
    let key = egui::Key::from_name(parts.pop()?)?;
    let mut m = egui::Modifiers::NONE;
    for p in parts {
        m |= match p {
            "Ctrl" => egui::Modifiers::COMMAND,
            "Shift" => egui::Modifiers::SHIFT,
            "Alt" => egui::Modifiers::ALT,
            _ => return None,
        };
    }
    Some(egui::KeyboardShortcut::new(m, key))
}

/// Quick Access Toolbar, tab row (Application button, tabs, Help and Options) and the active
/// tab's groups. `tab` indexes `w.tabs`; `open_app_menu` opens the Application menu once.
pub fn ribbon<A: Command, H: Host<A>>(
    h: &mut H,
    ui: &mut egui::Ui,
    w: &RibbonWindow<A>,
    tab: &mut usize,
    open_app_menu: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for &a in w.qat {
            let on = enabled(h, a);
            let resp = ribbon::qat_button(ui, &a.info(), icon(a), on, false);
            after_click(h, ui, a, on, &resp);
        }
        ui.add_space(12.0);
        ui.label(RichText::new(w.title).weak());
    });
    *tab = (*tab).min(w.tabs.len().saturating_sub(1));
    ui.horizontal(|ui| {
        let resp =
            ribbon::app_button_widget(ui).on_hover_text(format!("{}: new, open, save", w.title));
        let cmd = if resp.clicked() {
            Some(egui::SetOpenCommand::Toggle)
        } else if std::mem::take(open_app_menu) {
            Some(egui::SetOpenCommand::Bool(true))
        } else {
            None
        };
        egui::Popup::menu(&resp)
            .open_memory(cmd)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| application_menu(h, ui, w));
        ui.add_space(6.0);
        for (i, t) in w.tabs.iter().enumerate() {
            let selected = *tab == i;
            if ui
                .add(egui::Button::selectable(
                    selected,
                    ribbon::tab_text(t.label, selected),
                ))
                .clicked()
            {
                *tab = i;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for &a in w.buttons {
                let info = a.info();
                let r = ui
                    .add(egui::Button::new(format!("{} ▾", info.label)).frame(false))
                    .on_hover_text(info.screen_tip(MAC));
                egui::Popup::menu(&r).show(|ui| h.menu(a, ui));
            }
        });
    });
    if let Some(t) = w.tabs.get(*tab) {
        ribbon::body(ui, t.groups, |ui, a, large| command(h, ui, a, large));
    }
}

fn application_menu<A: Command, H: Host<A>>(h: &mut H, ui: &mut egui::Ui, w: &RibbonWindow<A>) {
    ui.set_width(260.0);
    for &a in w.app_menu {
        let on = enabled(h, a);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
            crate::icons::paint(
                ui.painter(),
                r,
                icon(a),
                ui.visuals().strong_text_color(),
                on,
            );
            ui.style_mut().override_font_id = Some(egui::FontId::proportional(14.0));
            menu_item(h, ui, a);
        });
    }
    ui.separator();
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        menu_item(h, ui, w.exit);
    });
}

/// The status bar: `text` (or the ScreenTip under the pointer) at the left.
pub fn status_bar(
    ui: &mut egui::Ui,
    hover: Option<&str>,
    text: &str,
    right: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        let shown = hover.unwrap_or(text);
        ui.add(egui::Label::new(RichText::new(shown).weak()).truncate());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
}

/// A classic menu bar (Design Database).
pub fn menu_bar<A: Command, H: Host<A>>(h: &mut H, ui: &mut egui::Ui, w: &MenuWindow<A>) {
    egui::MenuBar::new().ui(ui, |ui| {
        for m in w.menus {
            ui.menu_button(m.name, |ui| {
                for (k, section) in m.sections.iter().enumerate() {
                    if k > 0 {
                        ui.separator();
                    }
                    for &a in *section {
                        menu_item(h, ui, a);
                    }
                }
            });
        }
    });
}

/// A toolbar of small buttons in sections (Design Database); `labelled` says which show their
/// label next to the icon.
pub fn toolbar<A: Command, H: Host<A>>(
    h: &mut H,
    ui: &mut egui::Ui,
    w: &MenuWindow<A>,
    labelled: impl Fn(A) -> bool,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (k, section) in w.toolbar.iter().enumerate() {
            if k > 0 {
                ui.separator();
            }
            for &a in *section {
                let on = enabled(h, a);
                let pressed = a.kind() == Kind::Toggle && on && h.active(a);
                if !labelled(a) {
                    let resp = ribbon::qat_button(ui, &a.info(), icon(a), on, pressed);
                    after_click(h, ui, a, on, &resp);
                    continue;
                }
                let resp = ribbon::button(ui, &a.info(), icon(a), false, on, pressed, a.is_menu());
                after_click(h, ui, a, on, &resp);
            }
        }
    });
}

/// Colour theme choices (Options ▸, Application Color ▸).
pub fn theme_menu(app: &mut crate::app::EditorApp, ui: &mut egui::Ui) {
    use openstitch_app::editor::prefs::Theme;
    let mut changed = false;
    for t in Theme::ALL {
        changed |= ui
            .radio_value(&mut app.ui.prefs.theme, t, t.label())
            .changed();
    }
    if changed {
        app.ui.save_prefs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{EditorApp, Launch};
    use openstitch_app::editor::suite::{
        CREATOR, CreatorAction, DATABASE, DbAction, FACTORY, FactoryAction,
    };
    use std::path::PathBuf;

    fn demo() -> EditorApp {
        EditorApp::new(Launch {
            demo: true,
            ..Launch::default()
        })
    }

    /// Runs every command of a window (or draws its list) in each of `setups`, and checks the
    /// ones OpenStitch lacks are disabled with the "not yet available" ScreenTip.
    fn every_command_runs_or_is_unavailable<A: Command>(
        actions: Vec<A>,
        interactive: &[A],
        setups: &[&dyn Fn() -> EditorApp],
    ) where
        EditorApp: Host<A>,
    {
        let ctx = egui::Context::default();
        for a in actions {
            let info = a.info();
            if !info.available {
                let app = setups[0]();
                assert!(!enabled(&app, a), "{a:?} is unavailable but enabled");
                assert!(info.screen_tip(false).contains("Not yet available"));
                continue;
            }
            if interactive.contains(&a) {
                continue;
            }
            for setup in setups {
                let mut app = setup();
                if a.is_menu() {
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.menu(a, ui));
                    out.textures_delta.clear();
                } else {
                    app.run(a, &ctx);
                }
            }
        }
    }

    #[test]
    fn every_creator_command_runs_or_is_marked_unavailable() {
        use CreatorAction as C;
        let plain = || {
            let mut app = demo();
            app.creator.open = true;
            app
        };
        let copied = || {
            let mut app = plain();
            app.run(C::Copy, &egui::Context::default());
            app.run(C::NewLine, &egui::Context::default());
            app
        };
        let traced = || {
            let mut app = plain();
            app.creator.template = Some(openstitch_app::editor::creator::Template::fitted(
                PathBuf::from("trace.png"),
                (200, 100),
                &app.creator.doc,
            ));
            app.creator.selected = Some((0, 1));
            app
        };
        every_command_runs_or_is_unavailable(
            CREATOR.actions(),
            &[C::Open, C::SaveAs, C::OpenTemplate],
            &[&plain, &copied, &traced],
        );
    }

    #[test]
    fn creator_tools_template_and_guidelines() {
        use CreatorAction as C;
        use openstitch_design::patterns::RegionEffect;
        let ctx = egui::Context::default();
        let mut app = demo();
        app.creator.open = true;
        for (a, tool) in [
            (C::RegionReset, crate::creator_ui::CreatorTool::Region(None)),
            (
                C::RegionEngrave,
                crate::creator_ui::CreatorTool::Region(Some(RegionEffect::Engrave)),
            ),
            (
                C::RegionEmboss,
                crate::creator_ui::CreatorTool::Region(Some(RegionEffect::Emboss)),
            ),
            (C::Pan, crate::creator_ui::CreatorTool::Pan),
        ] {
            app.run(a, &ctx);
            assert_eq!(app.creator.tool, tool);
            assert!(app.active(a) && !app.active(C::DrawLine), "{a:?}");
        }
        // Region tools are for fill/stamp tiles; a motif drops back to Draw Line.
        app.run(C::NewMotif, &ctx);
        assert!(!enabled(&app, C::RegionEmboss));
        app.run(C::RegionEmboss, &ctx);
        assert_eq!(app.creator.tool, crate::creator_ui::CreatorTool::Pan);
        app.run(C::NewFill, &ctx);
        // Split at Point needs a selected inner point.
        assert!(!enabled(&app, C::SplitAtPoint));
        app.creator.selected = Some((0, 1));
        app.run(C::SplitAtPoint, &ctx);
        assert_eq!(app.creator.doc.paths.len(), 2);
        assert_eq!(app.creator.doc.current, 1);
        assert!(!enabled(&app, C::SplitAtPoint), "now the start of a line");
        // Template commands wait for an image.
        for a in [C::ModifyTemplate, C::DeleteImage, C::ImageDensity] {
            assert!(!enabled(&app, a), "{a:?}");
        }
        app.creator.template = Some(openstitch_app::editor::creator::Template::fitted(
            PathBuf::from("trace.png"),
            (100, 100),
            &app.creator.doc,
        ));
        app.run(C::ModifyTemplate, &ctx);
        assert!(app.active(C::ModifyTemplate));
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.menu(C::ImageDensity, ui);
        });
        out.textures_delta.clear();
        app.run(C::DeleteImage, &ctx);
        assert!(app.creator.template.is_none() && !app.active(C::ModifyTemplate));
        assert!(!app.active(C::Guideline));
        app.run(C::Guideline, &ctx);
        assert!(app.active(C::Guideline));
    }

    #[test]
    fn creator_commands_edit_the_pattern() {
        use CreatorAction as C;
        let ctx = egui::Context::default();
        let mut app = demo();
        app.creator.open = true;
        assert!(app.active(C::DrawLine));
        assert!(!enabled(&app, C::Delete), "one line always remains");
        app.run(C::Copy, &ctx);
        app.run(C::Paste, &ctx);
        assert_eq!(app.creator.doc.paths.len(), 2);
        assert!(enabled(&app, C::Delete));
        app.run(C::Cut, &ctx);
        assert_eq!(app.creator.doc.paths.len(), 1);
        app.run(C::NewMotif, &ctx);
        assert!(!enabled(&app, C::Paste), "a motif is one line");
        app.run(C::EditPoint, &ctx);
        assert!(app.active(C::EditPoint) && !app.active(C::DrawLine));
        app.run(C::PreviewWindow, &ctx);
        assert!(!app.creator.show_preview);
        let n = app.session.design().patterns.len();
        app.creator.doc.id = "my-test-motif".into();
        app.run(C::AddToDesign, &ctx);
        assert_eq!(app.session.design().patterns.len(), n + 1);
        app.run(C::Exit, &ctx);
        assert!(!app.creator.open);
    }

    #[test]
    fn every_factory_command_runs_or_is_marked_unavailable() {
        use FactoryAction as F;
        let plain = || {
            let mut app = demo();
            app.sewing.open = true;
            app
        };
        let cross = || {
            let mut app = plain();
            app.sewing.editor = openstitch_app::editor::sewing::SewingEditor::new(
                openstitch_design::sewing::SewingKind::Cross,
            );
            app
        };
        let selected = || {
            let mut app = plain();
            app.sewing.selected = Some(openstitch_app::editor::sewing::Handle::Point(1));
            app.sewing.tool = crate::sewing_ui::FactoryTool::SelectPoint;
            app
        };
        let templated = || {
            let mut app = plain();
            let path = template_png("runs");
            crate::sewing_ui::open_template(&mut app.sewing, &path, &egui::Context::default())
                .expect("template");
            app.sewing.modify_template = true;
            app
        };
        every_command_runs_or_is_unavailable(
            FACTORY.actions(),
            &[F::Open, F::SaveAs, F::Export, F::OpenTemplate],
            &[&plain, &cross, &selected, &templated],
        );
    }

    #[test]
    fn factory_commands_edit_and_save_the_pattern() {
        use FactoryAction as F;
        let ctx = egui::Context::default();
        let mut app = demo();
        app.sewing.open = true;
        assert!(!enabled(&app, F::Export), "save it in the project first");
        assert!(!enabled(&app, F::TripleStitch), "cross stitches only");
        let n = app.sewing.editor.doc.points.len();
        app.sewing.selected = Some(openstitch_app::editor::sewing::Handle::Point(1));
        app.run(F::Delete, &ctx);
        assert_eq!(app.sewing.editor.doc.points.len(), n - 1);
        let first = app.sewing.editor.doc.points[0];
        app.run(F::FlipVertical, &ctx);
        assert_eq!(app.sewing.editor.doc.points[0].y, -first.y);
        app.run(F::Save, &ctx);
        assert!(enabled(&app, F::Export));
        app.run(F::SnapToGrid, &ctx);
        assert!(!app.sewing.editor.snap);
        app.run(F::ImportPane, &ctx);
        assert!(!app.active(F::ImportPane));
    }

    /// A 4×2 PNG of the test's own in a temp folder (templates are user-supplied images).
    fn template_png(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openstitch-sdf-template-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("template.png");
        let rgba: Vec<u8> = (0..8).flat_map(|i| [i * 30, 90, 160, 255]).collect();
        std::fs::write(
            &path,
            openstitch_app::png::encode_rgba(4, 2, &rgba).expect("png"),
        )
        .expect("write");
        path
    }

    #[test]
    fn factory_tools_swap_duplicate_and_view_commands() {
        use crate::sewing_ui::FactoryTool;
        use FactoryAction as F;
        let ctx = egui::Context::default();
        let mut app = demo();
        app.sewing.open = true;
        for (a, tool) in [
            (F::SelectObject, FactoryTool::SelectObject),
            (F::PenTool, FactoryTool::Pen),
            (F::Pan, FactoryTool::Pan),
            (F::LineTool, FactoryTool::Line),
        ] {
            app.run(a, &ctx);
            assert_eq!(app.sewing.tool, tool);
            assert!(app.active(a));
        }
        // Swap: sewn from the other end; twice restores it.
        let first = app.sewing.editor.doc.needles()[0];
        app.run(F::Swap, &ctx);
        assert_ne!(app.sewing.editor.doc.needles()[0], first);
        app.run(F::Swap, &ctx);
        assert_eq!(app.sewing.editor.doc.needles()[0], first);
        // Duplicate doubles the decorative pattern; decorative satin cannot.
        let n = app.sewing.editor.doc.points.len();
        app.run(F::Duplicate, &ctx);
        assert_eq!(app.sewing.editor.doc.points.len(), 2 * n);
        app.sewing.editor = openstitch_app::editor::sewing::SewingEditor::new(
            openstitch_design::sewing::SewingKind::DecorativeSatin,
        );
        assert!(!enabled(&app, F::Duplicate));
        // Selected object zoom fits the pattern into the design area.
        app.run(F::SelectedZoom, &ctx);
        assert!(app.sewing.zoom > 1.0, "a short pattern is magnified");
        // Orientation, simulator, grid spacing.
        assert!(app.active(F::HorizontalOrientation));
        app.run(F::HorizontalOrientation, &ctx);
        assert!(!app.sewing.horizontal);
        app.run(F::StitchSimulator, &ctx);
        assert!(app.active(F::StitchSimulator));
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.menu(F::GridSpacing, ui));
        out.textures_delta.clear();
        // Template commands wait for a template.
        for a in [F::ModifyTemplate, F::DeleteImage, F::TemplateDensity] {
            assert!(!enabled(&app, a), "{a:?}");
        }
    }

    #[test]
    fn factory_template_opens_modifies_and_is_deleted() {
        use FactoryAction as F;
        let ctx = egui::Context::default();
        let mut app = demo();
        app.sewing.open = true;
        let path = template_png("edit");
        crate::sewing_ui::open_template(&mut app.sewing, &path, &ctx).expect("template");
        let t = app.sewing.template.as_ref().expect("open");
        assert_eq!(t.size.y * 2.0, t.size.x, "keeps the image's proportions");
        assert!(enabled(&app, F::DeleteImage));
        app.run(F::ModifyTemplate, &ctx);
        assert!(app.active(F::ModifyTemplate));
        assert!(
            enabled(&app, F::Delete),
            "Delete removes the template while modifying"
        );
        app.run(F::Delete, &ctx);
        assert!(app.sewing.template.is_none() && !app.sewing.modify_template);
        // Not an image: refused, nothing open.
        let bad = path.with_file_name("not-an-image.png");
        std::fs::write(&bad, b"hello").expect("write");
        assert!(crate::sewing_ui::open_template(&mut app.sewing, &bad, &ctx).is_err());
        assert!(app.sewing.template.is_none());
    }

    #[test]
    fn factory_simulator_plays_needle_by_needle() {
        use FactoryAction as F;
        let mut app = demo();
        app.sewing.open = true;
        app.run(F::StitchSimulator, &egui::Context::default());
        app.sewing.sim.playing = true;
        app.sewing.sim.pos = Some(0);
        let ctx = egui::Context::default();
        let mut frames = 0;
        while app.sewing.sim.playing && frames < 400 {
            let input = egui::RawInput {
                predicted_dt: 0.05,
                ..egui::RawInput::default()
            };
            let mut out = ctx.run_ui(input, |ui| crate::sewing_ui::factory(&mut app, ui));
            out.textures_delta.clear();
            frames += 1;
        }
        assert!(
            !app.sewing.sim.playing && app.sewing.sim.pos.is_none(),
            "played to the end"
        );
        assert!(frames > 3, "one needle point at a time, not all at once");
    }

    fn temp_folder() -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("openstitch-suite-db-{n}"));
        std::fs::create_dir_all(&dir).expect("dir");
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../openstitch-formats/tests/fixtures");
        for f in ["oracle.pes", "oracle.dst"] {
            std::fs::copy(fixtures.join(f), dir.join(f)).expect("copy");
        }
        dir
    }

    #[test]
    fn every_database_command_runs_or_is_marked_unavailable() {
        use DbAction as D;
        let dir = temp_folder();
        let empty = || {
            let mut app = demo();
            app.database.open = true;
            app
        };
        let folder = || {
            let mut app = empty();
            app.database.show(Some(&dir));
            app.database.select_all();
            app.database.copy();
            app
        };
        every_command_runs_or_is_unavailable(
            DATABASE.actions(),
            &[D::ChooseFolder],
            &[&empty, &folder],
        );
        let ctx = egui::Context::default();
        let mut app = folder();
        // Print hands the catalogue PDF over; test builds never start the OS print dialog.
        app.run(D::Print, &ctx);
        let pdf = app
            .printed
            .clone()
            .expect("a catalogue PDF for the print dialog");
        assert!(app.print_job.is_none(), "tests must not start a real print");
        let _ = std::fs::remove_file(pdf);
        assert!(enabled(&app, D::ConvertFormat) && enabled(&app, D::Paste));
        app.run(D::Details, &ctx);
        assert!(app.active(D::Details) && !app.active(D::LargeThumbnails));
        app.run(D::Toolbar, &ctx);
        assert!(!app.db_chrome.toolbar);
        app.run(D::Exit, &ctx);
        assert!(!app.database.open);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shortcuts_parse() {
        for a in CREATOR.actions() {
            if let Some(s) = a.info().shortcut
                && !matches!(s, "Ctrl+C" | "Ctrl+X" | "Ctrl+V")
            {
                assert!(parse_shortcut(s).is_some(), "{s}");
            }
        }
        for a in FACTORY.actions() {
            if let Some(s) = a.info().shortcut {
                assert!(parse_shortcut(s).is_some(), "{s}");
            }
        }
        assert_eq!(
            parse_shortcut("Ctrl+Shift+S"),
            Some(egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S
            ))
        );
    }

    #[test]
    fn suite_windows_draw() {
        let ctx = egui::Context::default();
        for k in 0..3 {
            let mut app = demo();
            match k {
                0 => app.creator.open = true,
                1 => app.sewing.open = true,
                _ => app.database.open = true,
            }
            for tab in 0..2 {
                app.creator.tab = tab;
                app.sewing.tab = tab;
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| match k {
                    0 => crate::creator_ui::creator(&mut app, ui),
                    1 => crate::sewing_ui::factory(&mut app, ui),
                    _ => crate::database_ui::database(&mut app, ui),
                });
                out.textures_delta.clear();
            }
        }
    }
}
