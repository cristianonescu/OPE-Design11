//! Update preferences and the UI-independent decisions of the self-updater
//! (`docs/research/updater.md`). The dialog itself is `update_ui` in the binary.

use serde_json::{Value, json};

/// Options ▸ updates, kept in `ui.json` under `updates`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePrefs {
    /// Check GitHub for a newer release at an interactive start.
    pub check: bool,
    /// Offer alpha, beta and rc releases to a stable build (a pre-release build always sees
    /// them).
    pub include_prereleases: bool,
    /// The tag the user asked not to be reminded of ("Don't remind me about this version");
    /// forgotten as soon as a newer release appears.
    pub skip_version: Option<String>,
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        UpdatePrefs {
            check: true,
            include_prereleases: false,
            skip_version: None,
        }
    }
}

impl UpdatePrefs {
    pub fn to_json(&self) -> Value {
        json!({
            "check": self.check,
            "includePrereleases": self.include_prereleases,
            "skipVersion": self.skip_version,
        })
    }

    pub fn from_json(v: &Value) -> UpdatePrefs {
        let d = UpdatePrefs::default();
        UpdatePrefs {
            check: v["check"].as_bool().unwrap_or(d.check),
            include_prereleases: v["includePrereleases"]
                .as_bool()
                .unwrap_or(d.include_prereleases),
            skip_version: v["skipVersion"]
                .as_str()
                .filter(|t| openstitch_update::is_release_tag(t))
                .map(str::to_string),
        }
    }

    /// Whether the dialog should open for `tag`, the newest release found. A skipped version
    /// stays quiet; anything newer than it is shown, and the skip is forgotten (returns
    /// `true` in `.1` when the preferences changed and need saving). A check the user asked
    /// for always shows.
    pub fn offer(&mut self, tag: &str, manual: bool) -> (bool, bool) {
        match self.skip_version.as_deref() {
            Some(skipped) if skipped == tag => (manual, false),
            Some(_) => {
                self.skip_version = None;
                (true, true)
            }
            None => (true, false),
        }
    }
}

/// The arguments this process was started with, for restarting the new version the same way.
pub fn restart_args() -> Vec<std::ffi::OsString> {
    std::env::args_os().skip(1).collect()
}

/// Set in the environment of the restarted process: the tag it was updated from.
pub const UPDATED_FROM_ENV: &str = "OPENSTITCH_UPDATED_FROM";
/// Set to anything to turn the startup check off (CI, kiosks, packagers).
pub const NO_CHECK_ENV: &str = "OPENSTITCH_NO_UPDATE_CHECK";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_junk() {
        let p = UpdatePrefs {
            check: false,
            include_prereleases: true,
            skip_version: Some("v0.1.0-alpha.4".into()),
        };
        assert_eq!(UpdatePrefs::from_json(&p.to_json()), p);
        assert_eq!(UpdatePrefs::from_json(&json!(null)), UpdatePrefs::default());
        let junk = json!({ "check": "yes", "skipVersion": "nightly" });
        let q = UpdatePrefs::from_json(&junk);
        assert!(q.check, "a non-boolean keeps the default");
        assert_eq!(q.skip_version, None, "only release tags are kept");
    }

    #[test]
    fn skipped_version_stays_quiet_until_a_newer_one() {
        let mut p = UpdatePrefs::default();
        assert_eq!(p.offer("v0.1.0-alpha.4", false), (true, false));
        p.skip_version = Some("v0.1.0-alpha.4".into());
        assert_eq!(p.offer("v0.1.0-alpha.4", false), (false, false));
        assert_eq!(p.skip_version.as_deref(), Some("v0.1.0-alpha.4"));
        assert_eq!(
            p.offer("v0.1.0-alpha.4", true),
            (true, false),
            "Check now always shows"
        );
        assert_eq!(p.offer("v0.1.0-alpha.5", false), (true, true));
        assert_eq!(p.skip_version, None, "the skip is forgotten");
    }
}
