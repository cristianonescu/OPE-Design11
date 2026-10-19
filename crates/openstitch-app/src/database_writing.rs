//! The Design Database's transfer area (toolbar ▸ Writing List): a Send To selector, the
//! Writing List of queued designs and the Add, Remove and Write buttons. USB media is the one
//! destination OpenStitch can write; the guide's other destinations are listed disabled with
//! the reason. The queue and Write live in [`openstitch_app::editor::database`].

use crate::app::EditorApp;
use crate::database_ui::DragPaths;
use eframe::egui::{self, RichText};
use openstitch_app::editor::database::{OnExists, WriteOutcome, file_name};
use openstitch_engine::transfer::{Volume, detect};
use std::path::PathBuf;

/// Destinations in the guide's Send To selector that OpenStitch cannot write, with the reason.
const BLOCKED: [(&str, &str); 4] = [
    (
        "Machine (USB cable)",
        "Blocked: Brother has not published the protocol for writing to a machine's memory.",
    ),
    (
        "Wireless network",
        "Blocked: Brother's wireless transfer protocol is undocumented.",
    ),
    (
        "Original card (USB card writer)",
        "Blocked: the card writer's protocol and card layout are undocumented.",
    ),
    (
        "USB Link / Wireless Link",
        "Blocked: the Link protocol is undocumented.",
    ),
];

/// Panel state that is only about drawing: the volumes found, the choice and a clash waiting
/// for an answer.
#[derive(Default)]
pub struct State {
    /// `None` until the volumes are detected (on first show and Refresh).
    volumes: Option<Vec<Volume>>,
    selected: usize,
    /// Designs picked in the list for Remove.
    picked: Vec<PathBuf>,
    /// The design at this index exists on the volume.
    clash: Option<(usize, PathBuf)>,
    /// Designs written by the current Write (across clash answers).
    written: usize,
    message: Option<String>,
}

pub fn panel(app: &mut EditorApp, ui: &mut egui::Ui) {
    let st = &mut app.db_chrome.writing;
    let volumes = st.volumes.get_or_insert_with(detect).clone();
    st.selected = st.selected.min(volumes.len().saturating_sub(1));
    let mut write: Option<(usize, OnExists)> = None;
    ui.horizontal(|ui| {
        ui.strong("Send To");
        let current = volumes
            .get(st.selected)
            .map_or_else(|| "No USB media found".to_string(), label);
        egui::ComboBox::from_id_salt("db-send-to")
            .selected_text(current)
            .width(280.0)
            .show_ui(ui, |ui| {
                for (i, v) in volumes.iter().enumerate() {
                    ui.selectable_value(&mut st.selected, i, label(v));
                }
                for (name, why) in BLOCKED {
                    ui.add_enabled(false, egui::Button::selectable(false, name))
                        .on_disabled_hover_text(why);
                }
            });
        if ui
            .button("Refresh")
            .on_hover_text("Look for USB media again")
            .clicked()
        {
            st.volumes = None;
        }
    });
    ui.separator();
    let db = &mut app.database;
    let st = &mut app.db_chrome.writing;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let any = !db.selection.is_empty();
            if ui
                .add_enabled(any, egui::Button::new("Add"))
                .on_hover_text("Add the selected designs to the Writing List")
                .clicked()
            {
                let sel = db.selected();
                db.add_to_writing(&sel);
            }
            if ui
                .add_enabled(!st.picked.is_empty(), egui::Button::new("Remove"))
                .on_hover_text("Remove the picked designs from the Writing List")
                .clicked()
            {
                db.remove_from_writing(&st.picked);
                st.picked.clear();
                st.clash = None;
            }
            let ready = !db.writing.is_empty() && !volumes.is_empty() && st.clash.is_none();
            if ui
                .add_enabled(ready, egui::Button::new("Write"))
                .on_hover_text("Copy the designs in the Writing List to the USB media")
                .clicked()
            {
                st.written = 0;
                write = Some((0, OnExists::Ask));
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label(RichText::new(format!("Writing List ({})", db.writing.len())).weak());
            let frame = egui::Frame::group(ui.style());
            let (_, dropped) = ui.dnd_drop_zone::<DragPaths, _>(frame, |ui| {
                ui.set_min_size(egui::vec2(ui.available_width(), 60.0));
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height().max(60.0))
                    .show(ui, |ui| {
                        if db.writing.is_empty() {
                            ui.label(
                                RichText::new("Add designs, or drag them here.")
                                    .weak()
                                    .small(),
                            );
                        }
                        let mut remove = None;
                        for p in &db.writing {
                            let on = st.picked.contains(p);
                            let r = ui.selectable_label(on, file_name(p));
                            if r.clicked() {
                                if on {
                                    st.picked.retain(|x| x != p);
                                } else {
                                    st.picked.push(p.clone());
                                }
                            }
                            r.context_menu(|ui| {
                                if ui.button("Remove from Writing List").clicked() {
                                    remove = Some(p.clone());
                                }
                            });
                        }
                        if let Some(p) = remove {
                            db.remove_from_writing(std::slice::from_ref(&p));
                            st.picked.retain(|x| *x != p);
                            st.clash = None;
                        }
                    });
            });
            if let Some(paths) = dropped {
                db.add_to_writing(&paths.0);
            }
        });
    });
    if let Some((index, path)) = st.clash.clone() {
        ui.horizontal(|ui| {
            ui.label(format!("{} already exists.", path.display()));
            if ui.button("Overwrite").clicked() {
                write = Some((index, OnExists::Overwrite));
            }
            if ui.button("Copy with a different name").clicked() {
                write = Some((index, OnExists::NewName));
            }
            if ui.button("Stop").clicked() {
                st.clash = None;
                st.message = Some(format!("Stopped after {} design(s)", st.written));
            }
        });
    } else if let Some(m) = &st.message {
        ui.label(RichText::new(m).weak());
    }
    let Some((from, first)) = write else {
        return;
    };
    let Some(volume) = volumes.get(st.selected) else {
        return;
    };
    st.clash = None;
    let msg = match db.write_list(&volume.path, from, first) {
        WriteOutcome::Done { written } => {
            st.written += written;
            "Finished outputting data.".to_string()
        }
        WriteOutcome::Exists {
            index,
            path,
            written,
        } => {
            st.written += written;
            st.clash = Some((index, path));
            return;
        }
        WriteOutcome::Failed { written, error } => {
            st.written += written;
            format!("Stopped after {} design(s): {error}", st.written)
        }
    };
    st.message = Some(msg.clone());
    db.status = msg;
}

fn label(v: &Volume) -> String {
    format!("{} ({})", v.name, v.path.display())
}
