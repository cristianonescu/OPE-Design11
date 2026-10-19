//! Window preferences kept between runs in `ui.json` next to the user settings file: colour
//! theme, Quick Access Toolbar, which panes show (docked or floating) and the most recently
//! used files. Unknown or malformed entries fall back to the defaults.

use super::ribbon::{Action, QAT_DEFAULT};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// How many files the Application menu lists.
pub const RECENT_MAX: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
    /// Follow the operating system.
    System,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Light, Theme::Dark, Theme::System];

    pub fn key(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::System => "system",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::System => "System",
        }
    }

    pub fn from_key(key: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.key() == key)
    }
}

/// A pane: shown or hidden, docked to the window or floating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaneState {
    pub visible: bool,
    pub floating: bool,
}

impl Default for PaneState {
    fn default() -> Self {
        PaneState {
            visible: true,
            floating: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Panes {
    pub sewing_order: PaneState,
    pub simulator: PaneState,
    /// Import / Color / Sewing Attributes / Text Attributes.
    pub attributes: PaneState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiPrefs {
    pub theme: Theme,
    pub qat: Vec<Action>,
    /// Quick Access Toolbar under the ribbon instead of above it.
    pub qat_below: bool,
    pub panes: Panes,
    /// Newest first.
    pub recent: Vec<PathBuf>,
}

impl Default for UiPrefs {
    fn default() -> Self {
        UiPrefs {
            theme: Theme::default(),
            qat: QAT_DEFAULT.to_vec(),
            qat_below: false,
            panes: Panes::default(),
            recent: Vec::new(),
        }
    }
}

fn pane_json(p: PaneState) -> Value {
    json!({ "visible": p.visible, "floating": p.floating })
}

fn pane_from(v: &Value) -> PaneState {
    let d = PaneState::default();
    PaneState {
        visible: v["visible"].as_bool().unwrap_or(d.visible),
        floating: v["floating"].as_bool().unwrap_or(d.floating),
    }
}

impl UiPrefs {
    pub fn to_json(&self) -> Value {
        json!({
            "theme": self.theme.key(),
            "quickAccess": self.qat.iter().map(|a| a.info().key).collect::<Vec<_>>(),
            "quickAccessBelow": self.qat_below,
            "panes": {
                "sewingOrder": pane_json(self.panes.sewing_order),
                "simulator": pane_json(self.panes.simulator),
                "attributes": pane_json(self.panes.attributes),
            },
            "recent": self.recent.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        })
    }

    pub fn from_json(v: &Value) -> UiPrefs {
        let d = UiPrefs::default();
        let qat = match v["quickAccess"].as_array() {
            Some(list) => {
                let mut out: Vec<Action> = Vec::new();
                for a in list
                    .iter()
                    .filter_map(|k| k.as_str().and_then(Action::from_key))
                {
                    if !out.contains(&a) {
                        out.push(a);
                    }
                }
                out
            }
            None => d.qat.clone(),
        };
        UiPrefs {
            theme: v["theme"]
                .as_str()
                .and_then(Theme::from_key)
                .unwrap_or(d.theme),
            qat,
            qat_below: v["quickAccessBelow"].as_bool().unwrap_or(d.qat_below),
            panes: Panes {
                sewing_order: pane_from(&v["panes"]["sewingOrder"]),
                simulator: pane_from(&v["panes"]["simulator"]),
                attributes: pane_from(&v["panes"]["attributes"]),
            },
            recent: v["recent"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .filter_map(Value::as_str)
                        .map(PathBuf::from)
                        .take(RECENT_MAX)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Reads `path`; a missing or unreadable file gives the defaults.
    pub fn load(path: &Path) -> UiPrefs {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map_or_else(UiPrefs::default, |v| UiPrefs::from_json(&v))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&self.to_json()).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    /// Puts `path` first in the recent files list.
    pub fn add_recent(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(RECENT_MAX);
    }

    /// Adds `a` to the Quick Access Toolbar, or removes it.
    pub fn toggle_qat(&mut self, a: Action) {
        if let Some(i) = self.qat.iter().position(|x| *x == a) {
            self.qat.remove(i);
        } else {
            self.qat.push(a);
        }
    }
}

/// `ui.json` beside the user settings file.
pub fn path_beside(user_settings: &Path) -> PathBuf {
    user_settings.with_file_name("ui.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_junk() {
        let mut p = UiPrefs {
            theme: Theme::System,
            qat_below: true,
            ..UiPrefs::default()
        };
        p.toggle_qat(Action::Help);
        p.toggle_qat(Action::Print);
        p.panes.simulator.visible = false;
        p.panes.attributes.floating = true;
        for i in 0..12 {
            p.add_recent(Path::new(&format!("/d/{i}.osd")));
        }
        p.add_recent(Path::new("/d/5.osd"));
        assert_eq!(p.recent.len(), RECENT_MAX);
        assert_eq!(p.recent[0], Path::new("/d/5.osd"));
        assert_eq!(p.recent.iter().filter(|r| r.ends_with("5.osd")).count(), 1);
        assert!(!p.qat.contains(&Action::Help) && p.qat.last() == Some(&Action::Print));
        assert_eq!(UiPrefs::from_json(&p.to_json()), p);
        let junk = json!({ "theme": "neon", "quickAccess": ["save", "nope", "save"], "panes": 3 });
        let q = UiPrefs::from_json(&junk);
        assert_eq!(q.theme, Theme::Light);
        assert_eq!(q.qat, [Action::Save]);
        assert_eq!(q.panes, Panes::default());
        assert_eq!(UiPrefs::from_json(&json!(null)), UiPrefs::default());
    }

    #[test]
    fn saves_beside_the_user_settings() {
        let dir = std::env::temp_dir().join(format!("openstitch-ui-{}", std::process::id()));
        let path = path_beside(&dir.join("user.json"));
        assert!(path.ends_with("ui.json"));
        assert_eq!(UiPrefs::load(&path), UiPrefs::default());
        let p = UiPrefs {
            theme: Theme::Dark,
            ..UiPrefs::default()
        };
        p.save(&path).expect("save");
        assert_eq!(UiPrefs::load(&path), p);
        let _ = std::fs::remove_dir_all(dir);
    }
}
