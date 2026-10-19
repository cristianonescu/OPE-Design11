//! File ▸ Print: what to print (actual size, reduced, positioning sheets, Print and Stitch),
//! paper and options, then `design.print` writes the PDF. Print hands it to the system print
//! dialog ([`openstitch_app::os_print`]); Save PDF keeps it for any viewer to print at 100 %.
//! The dialog keeps its state in egui's memory.

use crate::app::EditorApp;
use eframe::egui;
use openstitch_app::editor::CommandRequest;
use serde_json::json;

#[derive(Clone)]
struct PrintDialog {
    open: bool,
    mode: &'static str,
    paper: &'static str,
    box_axes: bool,
    mark: bool,
    grid: bool,
    mirror: bool,
}

impl Default for PrintDialog {
    fn default() -> Self {
        PrintDialog {
            open: false,
            mode: "actual",
            paper: "a4",
            box_axes: true,
            mark: true,
            grid: false,
            mirror: false,
        }
    }
}

const MODES: [(&str, &str); 4] = [
    ("actual", "Actual size (1:1) with sewing information"),
    ("reduced", "Reduced size, one page"),
    ("positioning", "Positioning sheets (one per hooping)"),
    (
        "image",
        "Print and Stitch: background image + alignment sheet",
    ),
];
const PAPERS: [(&str, &str); 4] = [
    ("a4", "A4"),
    ("letter", "Letter"),
    ("a3", "A3"),
    ("legal", "Legal"),
];

fn id() -> egui::Id {
    egui::Id::unique("print-dialog")
}

pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<PrintDialog>(id()).open = true);
}

pub fn window(app: &mut EditorApp, ctx: &egui::Context) {
    if std::mem::take(&mut app.ui.open_print) {
        open(ctx);
    }
    let mut d: PrintDialog =
        ctx.data_mut(|m| m.get_temp_mut_or_default::<PrintDialog>(id()).clone());
    if !d.open {
        return;
    }
    let has_background = app.session.design().background.is_some();
    // Some(true): print through the system; Some(false): save the PDF.
    let mut print: Option<bool> = None;
    let mut open = true;
    egui::Window::new("Print")
        .open(&mut open)
        .resizable(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            for (m, label) in MODES {
                let enabled = m != "image" || has_background;
                ui.add_enabled_ui(enabled, |ui| {
                    ui.radio_value(&mut d.mode, m, label)
                        .on_disabled_hover_text("Needs a background image");
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Paper");
                for (p, label) in PAPERS {
                    ui.selectable_value(&mut d.paper, p, label);
                }
            });
            ui.checkbox(&mut d.box_axes, "Sewing area box and centre axes");
            ui.checkbox(&mut d.mark, "Embroidery positioning mark");
            ui.checkbox(&mut d.grid, "Template grid (10 mm)");
            if d.mode == "image" {
                ui.checkbox(&mut d.mirror, "Mirror the image (iron-on transfer sheets)");
            }
            ui.label(
                egui::RichText::new(
                    "Print the PDF at 100 % (no fit to page); check the 50 mm scale bar with a ruler.",
                )
                .weak()
                .small(),
            );
            ui.horizontal(|ui| {
                if ui
                    .button("Print…")
                    .on_hover_text("Open the system print dialog (or the PDF viewer)")
                    .clicked()
                {
                    print = Some(true);
                }
                if ui.button("Save PDF…").clicked() {
                    print = Some(false);
                }
            });
        });
    d.open = open && print.is_none();
    ctx.data_mut(|m| m.insert_temp(id(), d.clone()));
    if let Some(system) = print {
        let name = app.session.design().name.clone();
        let path = if system {
            let path = openstitch_app::os_print::temp_pdf(&name);
            if let Some(dir) = path.parent()
                && let Err(e) = std::fs::create_dir_all(dir)
            {
                app.status = format!("{}: {e}", dir.display());
                return;
            }
            path
        } else {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("{name}.pdf"))
                .save_file()
            else {
                return;
            };
            path
        };
        let params = json!({
            "path": path.display().to_string(), "mode": d.mode, "paper": d.paper,
            "sewingAreaBox": d.box_axes, "centerMark": d.mark, "templateGrid": d.grid,
            "mirror": d.mirror,
        });
        if let Some(r) = app.execute(&CommandRequest::new("design.print", params)) {
            if system {
                app.print_pdf(&path);
            } else {
                app.status = format!("Saved {} ({} pages)", path.display(), r["pages"]);
            }
        }
    }
}
