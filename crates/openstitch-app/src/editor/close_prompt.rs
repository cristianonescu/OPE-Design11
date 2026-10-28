//! The unsaved-changes prompt ("Save changes to NAME?"): what the editor was about to do
//! when it found unsaved changes, and what the user's answer means for it. Pure, so the
//! state machine is unit-tested without a window; `close_ui.rs` draws it.

use std::path::PathBuf;

/// What the editor was asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    /// Close the window: the X button, Alt+F4, the OS, File ▸ Exit.
    Exit,
    /// File ▸ New.
    New,
    /// File ▸ Open: the file dialog (`None`) or a given file (Recent Files, Design Database).
    Open(Option<PathBuf>),
}

/// The prompt's buttons (Enter = Save, Escape = Cancel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Save,
    DontSave,
    Cancel,
}

/// What the editor does after an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Save first (Save As when the design has no file yet); go on with the pending action
    /// only if the save left the design clean (a cancelled Save As stays in the editor).
    SaveThen(Pending),
    /// Discard the changes and go on.
    Proceed(Pending),
    /// Back to the editor.
    Stay,
}

/// The prompt: closed, or open for one pending action.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClosePrompt {
    pending: Option<Pending>,
}

impl ClosePrompt {
    /// Asks whether `what` may run now. A clean design: yes, and nothing is pending. Unsaved
    /// changes: no, the prompt opens for `what` and the caller waits for [`Self::answer`]. A
    /// request while the prompt is open replaces the pending action (the X button pressed
    /// after File ▸ New asks about closing).
    pub fn request(&mut self, what: Pending, dirty: bool) -> bool {
        if dirty {
            self.pending = Some(what);
            false
        } else {
            self.pending = None;
            true
        }
    }

    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    /// The user's answer closes the prompt and says what to do with the pending action.
    pub fn answer(&mut self, choice: Choice) -> Outcome {
        let Some(pending) = self.pending.take() else {
            return Outcome::Stay;
        };
        match choice {
            Choice::Save => Outcome::SaveThen(pending),
            Choice::DontSave => Outcome::Proceed(pending),
            Choice::Cancel => Outcome::Stay,
        }
    }
}

/// The prompt's title for a design shown as `name` (its file name, or the design name).
pub fn title(name: &str) -> String {
    format!("Save changes to {name}?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_design_goes_straight_on() {
        let mut p = ClosePrompt::default();
        assert!(p.request(Pending::Exit, false));
        assert!(!p.is_open());
        assert_eq!(p.answer(Choice::Save), Outcome::Stay, "nothing was pending");
    }

    #[test]
    fn unsaved_changes_open_the_prompt_and_each_answer_has_its_outcome() {
        let mut p = ClosePrompt::default();
        assert!(!p.request(Pending::Exit, true));
        assert!(p.is_open());
        assert_eq!(p.pending(), Some(&Pending::Exit));
        assert_eq!(p.answer(Choice::Cancel), Outcome::Stay);
        assert!(!p.is_open(), "Cancel closes the prompt");

        assert!(!p.request(Pending::New, true));
        assert_eq!(p.answer(Choice::DontSave), Outcome::Proceed(Pending::New));
        assert!(!p.is_open());

        let file = Some(PathBuf::from("a.osd"));
        assert!(!p.request(Pending::Open(file.clone()), true));
        assert_eq!(
            p.answer(Choice::Save),
            Outcome::SaveThen(Pending::Open(file))
        );
        assert!(!p.is_open());
    }

    #[test]
    fn a_later_request_replaces_the_pending_action() {
        let mut p = ClosePrompt::default();
        assert!(!p.request(Pending::New, true));
        assert!(!p.request(Pending::Exit, true));
        assert_eq!(p.answer(Choice::DontSave), Outcome::Proceed(Pending::Exit));
    }

    #[test]
    fn a_clean_request_clears_a_stale_prompt() {
        let mut p = ClosePrompt::default();
        p.request(Pending::Exit, true);
        assert!(p.request(Pending::Open(None), false));
        assert!(!p.is_open());
    }

    #[test]
    fn title_names_the_design() {
        assert_eq!(title("rose.osd"), "Save changes to rose.osd?");
    }
}
