//! Window preferences kept between runs in `ui.json` next to the user settings file: colour
//! theme, Quick Access Toolbar, which panes show (docked or floating), the most recently used
//! files, the realistic view settings and the font filter. Unknown or malformed entries fall
//! back to the defaults.

use super::lettering::FontChoice;
use super::ribbon::{Action, QAT_DEFAULT};
use super::workspace::RealisticSettings;
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

/// Text ▸ Font Filter: fonts hidden from the font lists while the filter is on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontFilter {
    pub on: bool,
    /// Font ids cleared in Font Filter Settings.
    pub hidden: Vec<String>,
}

impl FontFilter {
    /// Most hidden font ids kept.
    pub const MAX_HIDDEN: usize = 1_000;

    pub fn shows(&self, id: &str) -> bool {
        !self.on || !self.hidden.iter().any(|h| h == id)
    }

    /// The fonts the lists show. With every font hidden the filter is ignored, so a list is
    /// never empty.
    pub fn apply(&self, fonts: Vec<FontChoice>) -> Vec<FontChoice> {
        if !self.on || fonts.iter().all(|f| !self.shows(&f.id)) {
            return fonts;
        }
        fonts.into_iter().filter(|f| self.shows(&f.id)).collect()
    }

    /// Shows or hides one font in the filter settings.
    pub fn set_shown(&mut self, id: &str, shown: bool) {
        self.hidden.retain(|h| h != id);
        if !shown && self.hidden.len() < Self::MAX_HIDDEN {
            self.hidden.push(id.to_string());
        }
    }
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
    pub realistic: RealisticSettings,
    pub font_filter: FontFilter,
}

impl Default for UiPrefs {
    fn default() -> Self {
        UiPrefs {
            theme: Theme::default(),
            qat: QAT_DEFAULT.to_vec(),
            qat_below: false,
            panes: Panes::default(),
            recent: Vec::new(),
            realistic: RealisticSettings::default(),
            font_filter: FontFilter::default(),
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
            "realisticView": {
                "threadWidthMM": self.realistic.thread_width_mm,
                "contrast": self.realistic.contrast,
                "brightness": self.realistic.brightness,
            },
            "fontFilter": { "on": self.font_filter.on, "hidden": self.font_filter.hidden },
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
            realistic: {
                let r = &v["realisticView"];
                let n = |k: &str, d: f64| r[k].as_f64().unwrap_or(d);
                RealisticSettings {
                    thread_width_mm: n("threadWidthMM", d.realistic.thread_width_mm),
                    contrast: n("contrast", d.realistic.contrast),
                    brightness: n("brightness", d.realistic.brightness),
                }
                .clamped()
            },
            font_filter: FontFilter {
                on: v["fontFilter"]["on"].as_bool().unwrap_or(false),
                hidden: v["fontFilter"]["hidden"]
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .take(FontFilter::MAX_HIDDEN)
                            .collect()
                    })
                    .unwrap_or_default(),
            },
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
        p.realistic.contrast = 40.0;
        p.realistic.thread_width_mm = 0.6;
        p.font_filter.on = true;
        p.font_filter.set_shown("lato-bold", false);
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
        let wild = json!({ "realisticView": { "threadWidthMM": 50, "contrast": "x" }, "fontFilter": { "on": 1, "hidden": [3, "a"] } });
        let w = UiPrefs::from_json(&wild);
        assert_eq!(w.realistic.thread_width_mm, 1.0);
        assert_eq!(w.realistic.contrast, 0.0);
        assert_eq!(
            w.font_filter,
            FontFilter {
                on: false,
                hidden: vec!["a".into()]
            }
        );
    }

    #[test]
    fn font_filter_hides_fonts_but_never_all() {
        let font = |id: &str| FontChoice {
            id: id.into(),
            label: id.into(),
        };
        let fonts = vec![font("a"), font("b")];
        let mut f = FontFilter::default();
        f.set_shown("a", false);
        assert_eq!(f.apply(fonts.clone()), fonts, "off: everything shows");
        f.on = true;
        assert_eq!(f.apply(fonts.clone()), [font("b")]);
        assert!(f.shows("b") && !f.shows("a"));
        f.set_shown("b", false);
        assert_eq!(f.apply(fonts.clone()), fonts, "never an empty list");
        f.set_shown("a", true);
        f.set_shown("a", true);
        assert_eq!(f.hidden, ["b"]);
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
