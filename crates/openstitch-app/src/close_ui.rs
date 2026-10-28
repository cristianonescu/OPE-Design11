//! The unsaved-changes prompt: a centred modal, "Save changes to NAME?", with Save (Enter),
//! Don't Save and Cancel (Escape). The state machine is
//! [`openstitch_app::editor::close_prompt`]; [`EditorApp::answer_close_prompt`] acts on the
//! answer.

use crate::app::EditorApp;
use eframe::egui::{self, Key, Modifiers, RichText};
use openstitch_app::editor::close_prompt::{self, Choice, Pending};

pub fn window(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(pending) = app.close_prompt.pending().cloned() else {
        return;
    };
    let title = close_prompt::title(&app.document_name());
    let what = match pending {
        Pending::Exit => "Your changes will be lost if you close without saving.",
        Pending::New => "Your changes will be lost if you start a new design without saving.",
        Pending::Open(_) => "Your changes will be lost if you open another design without saving.",
    };
    let mut choice = None;
    let modal = egui::Modal::new(egui::Id::unique("close-prompt")).show(ctx, |ui| {
        ui.set_width(400.0);
        ui.heading(title);
        ui.add_space(4.0);
        ui.label(RichText::new(what).weak());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new("Save").strong())
                .on_hover_text(
                    "Saves the design (Save As when it has no file yet), then goes on · Enter",
                )
                .clicked()
            {
                choice = Some(Choice::Save);
            }
            if ui
                .button("Don't Save")
                .on_hover_text("Goes on and discards the changes")
                .clicked()
            {
                choice = Some(Choice::DontSave);
            }
            if ui
                .button("Cancel")
                .on_hover_text("Back to the editor · Esc")
                .clicked()
            {
                choice = Some(Choice::Cancel);
            }
        });
    });
    if choice.is_none() {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            choice = Some(Choice::Save);
        } else if modal.should_close() {
            choice = Some(Choice::Cancel);
        }
    }
    if let Some(c) = choice {
        app.answer_close_prompt(c);
    }
}
