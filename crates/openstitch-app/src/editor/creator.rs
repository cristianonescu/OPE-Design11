//! Programmable Stitch Creator: the editable pattern (unit-box polylines), its editing
//! operations, the preview design and the user pattern library folder. No egui here, so all
//! of it is unit-tested; `creator_ui` draws it.
//!
//! The Creator edits OpenStitch's own `openstitch.pattern` documents (see
//! `openstitch_design::patterns`): motifs (motif lines and motif fill), fill tiles
//! (programmable fill; PE-DESIGN's fill/stamp patterns) and decorative-fill rows. A pattern
//! reaches a design through `pattern.define` (one undo step) and is kept between sessions in
//! the user library folder as one `<kind>-<id>.json` file per pattern.

use super::request::CommandRequest;
use openstitch_core::Thread;
use openstitch_design::patterns::{self, MAX_DOCUMENT, MAX_POINTS, Pattern, PatternKind};
use openstitch_design::{
    Contour, Design, DesignObject, DesignPage, LineSewing, LineType, RegionSewing, RegionType,
    Vec2, v2,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Grid the editor snaps to: 1/16 of the unit box.
pub const SNAP: f64 = 1.0 / 16.0;
/// Most files read from the library folder.
pub const MAX_LIBRARY_FILES: usize = 500;

pub const KINDS: [(PatternKind, &str); 3] = [
    (PatternKind::Fill, "Fill / stamp tile"),
    (PatternKind::Motif, "Motif"),
    (PatternKind::Decorative, "Decorative fill"),
];

pub fn kind_name(k: PatternKind) -> &'static str {
    match k {
        PatternKind::Motif => "motif",
        PatternKind::Fill => "fill",
        PatternKind::Decorative => "decorative",
    }
}

/// The pattern being edited.
#[derive(Clone, Debug, PartialEq)]
pub struct Creator {
    pub kind: PatternKind,
    pub id: String,
    pub name: String,
    /// Decorative rows: shift of each row, in tile widths.
    pub row_shift: f64,
    pub paths: Vec<Vec<Vec2>>,
    /// The path new points go into.
    pub current: usize,
    pub snap: bool,
}

impl Default for Creator {
    fn default() -> Self {
        Creator::new(PatternKind::Fill)
    }
}

impl Creator {
    /// A fresh pattern of `kind` with a minimal valid path.
    pub fn new(kind: PatternKind) -> Self {
        let paths = match kind {
            PatternKind::Motif => vec![vec![v2(0.0, 0.0), v2(0.5, -0.8), v2(1.0, 0.0)]],
            PatternKind::Fill => vec![vec![v2(0.0, 0.5), v2(0.5, 0.0), v2(1.0, 0.5)]],
            PatternKind::Decorative => vec![vec![v2(0.0, 0.7), v2(0.5, 0.2), v2(1.0, 0.7)]],
        };
        Creator {
            kind,
            id: format!("my-{}", kind_name(kind)),
            name: format!("My {}", kind_name(kind)),
            row_shift: 0.0,
            paths,
            current: 0,
            snap: true,
        }
    }

    /// Starts from an existing pattern (a bundled one as a template, or a user one to edit).
    pub fn from_pattern(p: &Pattern) -> Self {
        Creator {
            kind: p.kind,
            id: p.id.clone(),
            name: p.name.clone(),
            row_shift: p.row_shift,
            paths: p.paths.clone(),
            current: 0,
            snap: true,
        }
    }

    /// The unit box points live in: (min x, min y, max x, max y).
    pub fn unit_box(&self) -> (f64, f64, f64, f64) {
        Pattern::unit_box(self.kind)
    }

    fn clamp(&self, p: Vec2) -> Vec2 {
        let (x0, y0, x1, y1) = self.unit_box();
        let p = if self.snap {
            v2((p.x / SNAP).round() * SNAP, (p.y / SNAP).round() * SNAP)
        } else {
            p
        };
        v2(p.x.clamp(x0, x1), p.y.clamp(y0, y1))
    }

    /// Ends that must stay put: a motif runs from (0, 0) to (1, 0); a decorative row from
    /// x = 0 to x = 1 at one height.
    fn is_anchor(&self, path: usize, index: usize) -> bool {
        let n = self.paths.get(path).map_or(0, Vec::len);
        self.kind != PatternKind::Fill && (index == 0 || index + 1 == n)
    }

    fn point_count(&self) -> usize {
        self.paths.iter().map(Vec::len).sum()
    }

    /// Adds `p` to the current path: before the end anchor for motifs and decorative rows,
    /// at the end for fill tiles. Returns the new point's index.
    pub fn add_point(&mut self, p: Vec2) -> Option<usize> {
        if self.point_count() >= MAX_POINTS {
            return None;
        }
        let q = self.clamp(p);
        let path = self.paths.get_mut(self.current)?;
        let at = if self.kind == PatternKind::Fill {
            path.len()
        } else {
            path.len().saturating_sub(1)
        };
        path.insert(at, q);
        Some(at)
    }

    /// Moves a point; anchors move only along y (decorative, both ends together) or not at
    /// all (motif).
    pub fn move_point(&mut self, path: usize, index: usize, p: Vec2) {
        let q = self.clamp(p);
        let anchor = self.is_anchor(path, index);
        let kind = self.kind;
        let Some(pts) = self.paths.get_mut(path) else {
            return;
        };
        if index >= pts.len() {
            return;
        }
        match (anchor, kind) {
            (false, _) => pts[index] = q,
            (true, PatternKind::Decorative) => {
                let last = pts.len() - 1;
                pts[0].y = q.y;
                pts[last].y = q.y;
            }
            (true, _) => {}
        }
    }

    /// Deletes a point (never an anchor, never below two points per path).
    pub fn delete_point(&mut self, path: usize, index: usize) -> bool {
        if self.is_anchor(path, index) {
            return false;
        }
        match self.paths.get_mut(path) {
            Some(pts) if pts.len() > 2 && index < pts.len() => {
                pts.remove(index);
                true
            }
            _ => false,
        }
    }

    /// Starts a new path (fill tiles and decorative rows; a motif is one path).
    pub fn new_path(&mut self) -> bool {
        if self.kind == PatternKind::Motif || self.paths.len() >= patterns::MAX_PATHS {
            return false;
        }
        let path = match self.kind {
            PatternKind::Decorative => vec![v2(0.0, 0.5), v2(1.0, 0.5)],
            _ => vec![v2(0.25, 0.5), v2(0.75, 0.5)],
        };
        self.paths.push(path);
        self.current = self.paths.len() - 1;
        true
    }

    /// Deletes the current path (one always remains).
    pub fn delete_path(&mut self) -> bool {
        if self.paths.len() <= 1 {
            return false;
        }
        self.paths.remove(self.current);
        self.current = self.current.min(self.paths.len() - 1);
        true
    }

    /// The pattern as an `openstitch.pattern` document.
    pub fn document(&self) -> Value {
        let mut doc = json!({
            "format": "openstitch.pattern",
            "version": 1,
            "kind": kind_name(self.kind),
            "id": self.id,
            "name": self.name,
            "paths": self.paths.iter()
                .map(|p| p.iter().map(|q| json!([q.x, q.y])).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
        });
        if self.kind == PatternKind::Decorative && self.row_shift != 0.0 {
            doc["rowShift"] = json!(self.row_shift);
        }
        doc
    }

    /// The validated pattern, or why it is not valid yet.
    pub fn pattern(&self) -> Result<Pattern, String> {
        if patterns::is_bundled(&self.id) {
            return Err(format!(
                "'{}' is the id of a bundled pattern; choose another id",
                self.id
            ));
        }
        patterns::parse(&self.document().to_string())
    }

    /// `pattern.define` for the open design (one undo step).
    pub fn define_request(&self) -> Result<CommandRequest, String> {
        self.pattern()?;
        Ok(CommandRequest::new(
            "pattern.define",
            json!({ "pattern": self.document() }),
        ))
    }

    /// A small design that shows the pattern in use: a motif line and motif fill, a
    /// programmable fill, or a decorative fill. `None` while the pattern is invalid.
    pub fn preview_design(&self) -> Option<Design> {
        let pattern = self.pattern().ok()?;
        let mut d = Design::new(
            "preview",
            DesignPage {
                width: 600.0,
                height: 400.0,
            },
        );
        let thread = Thread::rgb(0x0A55A3);
        let rect = Contour::rectangle(20.0, 20.0, 560.0, 360.0);
        match self.kind {
            PatternKind::Motif => {
                let wave: Vec<Vec2> = (0..=24)
                    .map(|k| {
                        let t = f64::from(k) / 24.0;
                        v2(
                            40.0 + 520.0 * t,
                            70.0 + 25.0 * (t * std::f64::consts::TAU).sin(),
                        )
                    })
                    .collect();
                let mut line = LineSewing::new(LineType::Motif, thread.clone());
                line.motif = Some(pattern.id.clone());
                line.width = 6.0;
                line.height = Some(4.0);
                line.pitch = 1.0;
                d.add(DesignObject::shape(
                    vec![Contour::new(wave, false)],
                    None,
                    Some(line),
                ));
                let mut fill = RegionSewing::new(RegionType::MotifFill, thread);
                fill.pattern = Some(pattern.id.clone());
                fill.pattern_width = Some(5.0);
                fill.pattern_height = Some(4.0);
                fill.run_pitch = Some(1.0);
                d.add(DesignObject::shape(
                    vec![Contour::rectangle(20.0, 140.0, 560.0, 240.0)],
                    Some(fill),
                    None,
                ));
            }
            PatternKind::Fill => {
                let mut fill = RegionSewing::new(RegionType::ProgrammableFill, thread);
                fill.pattern = Some(pattern.id.clone());
                fill.pattern_width = Some(12.0);
                fill.pattern_height = Some(12.0);
                fill.density = 2.2;
                d.add(DesignObject::shape(vec![rect], Some(fill), None));
            }
            PatternKind::Decorative => {
                let mut fill = RegionSewing::new(RegionType::DecorativeFill, thread);
                fill.pattern = Some(pattern.id.clone());
                fill.pattern_width = Some(10.0);
                fill.pattern_height = Some(8.0);
                fill.run_pitch = Some(1.5);
                d.add(DesignObject::shape(vec![rect], Some(fill), None));
            }
        }
        d.patterns.push(pattern);
        Some(d)
    }
}

/// The user pattern library folder: `$OPENSTITCH_PATTERNS` if set, else the platform's
/// per-user data folder (`~/Library/Application Support/OpenStitch/patterns` on macOS,
/// `%APPDATA%\OpenStitch\patterns` on Windows, `$XDG_DATA_HOME/openstitch/patterns` or
/// `~/.local/share/openstitch/patterns` elsewhere).
pub fn library_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = env("OPENSTITCH_PATTERNS") {
        return Some(dir);
    }
    if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/OpenStitch/patterns"))
    } else if cfg!(windows) {
        env("APPDATA").map(|a| a.join("OpenStitch").join("patterns"))
    } else {
        env("XDG_DATA_HOME")
            .map(|d| d.join("openstitch"))
            .or_else(|| env("HOME").map(|h| h.join(".local/share/openstitch")))
            .map(|d| d.join("patterns"))
    }
}

/// One file of the library folder.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryEntry {
    pub path: PathBuf,
    /// The pattern, or why the file could not be used.
    pub pattern: Result<Pattern, String>,
}

/// Reads every `.json` file in `dir` (sorted by name; at most [`MAX_LIBRARY_FILES`], each at
/// most 1 MiB). Unreadable or invalid files are listed with their error, never fatal.
pub fn load_library(dir: &Path) -> Vec<LibraryEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        })
        .collect();
    files.sort();
    files.truncate(MAX_LIBRARY_FILES);
    files
        .into_iter()
        .map(|path| {
            let pattern = read_pattern(&path);
            LibraryEntry { path, pattern }
        })
        .collect()
}

fn read_pattern(path: &Path) -> Result<Pattern, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file".into());
    }
    if meta.len() > MAX_DOCUMENT as u64 {
        return Err("larger than 1 MiB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let p = patterns::parse(&text)?;
    if patterns::is_bundled(&p.id) {
        return Err(format!("'{}' is a bundled pattern id", p.id));
    }
    Ok(p)
}

/// File name of a pattern in the library: `<kind>-<id>.json` (ids are `[a-z0-9-]`, so the
/// name cannot leave the folder).
pub fn library_file_name(p: &Pattern) -> String {
    format!("{}-{}.json", kind_name(p.kind), p.id)
}

/// Writes `p` into `dir` (created if missing) and returns the file path.
pub fn save_to_library(dir: &Path, p: &Pattern) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(library_file_name(p));
    std::fs::write(&path, p.to_json() + "\n")?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_core::Command as Cmd;
    use openstitch_engine::Session;

    fn temp_dir(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("openstitch-creator-{tag}-{n}"));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn every_kind_starts_valid_and_reaches_the_design() {
        for (kind, _) in KINDS {
            let c = Creator::new(kind);
            let p = c.pattern().expect("valid");
            assert_eq!(p.kind, kind);
            let mut s = Session::new(std::env::temp_dir());
            let r = c.define_request().expect("request");
            s.execute(r.command, Some(&r.params))
                .expect("pattern.define");
            assert_eq!(s.design().patterns, vec![p]);
            let d = c.preview_design().expect("preview");
            let compiled = openstitch_design::Compiler::default().compile(&d);
            assert!(compiled.count(Cmd::Stitch) > 100, "{kind:?}");
        }
    }

    #[test]
    fn points_are_added_snapped_and_kept_inside_the_unit_box() {
        let mut c = Creator::new(PatternKind::Motif);
        // Motif points go before the (1, 0) end anchor.
        assert_eq!(c.add_point(v2(0.8, -0.33)), Some(2));
        assert_eq!(c.paths[0][2], v2(0.8125, -0.3125));
        assert_eq!(*c.paths[0].last().expect("end"), v2(1.0, 0.0));
        c.add_point(v2(9.0, -9.0));
        assert_eq!(c.paths[0][3], v2(1.5, -1.0), "clamped to the motif box");
        assert!(c.pattern().is_ok());
        let mut f = Creator::new(PatternKind::Fill);
        f.snap = false;
        assert_eq!(f.add_point(v2(0.3, 0.71)), Some(3));
        assert_eq!(f.paths[0][3], v2(0.3, 0.71));
        f.add_point(v2(-1.0, 2.0));
        assert_eq!(f.paths[0][4], v2(0.0, 1.0));
    }

    #[test]
    fn anchors_stay_where_the_format_needs_them() {
        let mut m = Creator::new(PatternKind::Motif);
        m.move_point(0, 0, v2(0.5, 0.5));
        assert_eq!(m.paths[0][0], v2(0.0, 0.0), "motif start is fixed");
        assert!(!m.delete_point(0, 0));
        m.move_point(0, 1, v2(0.25, 0.5));
        assert_eq!(m.paths[0][1], v2(0.25, 0.5));
        assert!(m.delete_point(0, 1));
        assert_eq!(m.paths[0], vec![v2(0.0, 0.0), v2(1.0, 0.0)]);
        assert!(!m.delete_point(0, 1), "only the anchors are left");
        assert!(m.pattern().is_ok());
        let mut d = Creator::new(PatternKind::Decorative);
        d.move_point(0, 2, v2(0.9, 0.25));
        assert_eq!(
            (d.paths[0][0], d.paths[0][2]),
            (v2(0.0, 0.25), v2(1.0, 0.25))
        );
        assert!(d.pattern().is_ok());
        let mut f = Creator::new(PatternKind::Fill);
        f.move_point(0, 0, v2(0.5, 0.5));
        assert_eq!(f.paths[0][0], v2(0.5, 0.5), "fill tiles have no anchors");
        assert!(f.delete_point(0, 1));
        assert!(!f.delete_point(0, 1), "two points remain");
        // Out-of-range indexes are ignored.
        f.move_point(5, 0, v2(0.0, 0.0));
        assert!(!f.delete_point(0, 99));
    }

    #[test]
    fn paths_are_added_and_removed_within_limits() {
        let mut m = Creator::new(PatternKind::Motif);
        assert!(!m.new_path(), "a motif is one path");
        let mut f = Creator::new(PatternKind::Fill);
        assert!(!f.delete_path());
        assert!(f.new_path());
        assert_eq!((f.paths.len(), f.current), (2, 1));
        assert!(f.delete_path());
        assert_eq!((f.paths.len(), f.current), (1, 0));
        while f.new_path() {}
        assert_eq!(f.paths.len(), patterns::MAX_PATHS);
        assert!(f.pattern().is_ok());
    }

    #[test]
    fn bundled_ids_and_bad_names_are_refused() {
        let mut c =
            Creator::from_pattern(patterns::find(PatternKind::Motif, "heart").expect("heart"));
        assert!(c.pattern().is_err(), "bundled id");
        assert!(c.define_request().is_err());
        c.id = "my-heart".into();
        let p = c.pattern().expect("valid copy");
        assert_eq!(
            p.paths,
            patterns::find(PatternKind::Motif, "heart")
                .expect("heart")
                .paths
        );
        assert_eq!(Creator::from_pattern(&p).pattern(), Ok(p));
        for bad in ["", "Has Space", "UPPER", "a/b", "../x"] {
            c.id = bad.into();
            assert!(c.pattern().is_err(), "{bad}");
        }
        c.id = "ok".into();
        c.name = String::new();
        assert!(c.pattern().is_err());
        assert!(c.preview_design().is_none());
    }

    #[test]
    fn the_library_folder_round_trips_and_reports_bad_files() {
        let dir = temp_dir("lib");
        let a = Creator::new(PatternKind::Fill).pattern().expect("fill");
        let mut m = Creator::new(PatternKind::Motif);
        m.id = "star".into();
        let b = m.pattern().expect("motif");
        let pa = save_to_library(&dir, &a).expect("save");
        save_to_library(&dir, &b).expect("save");
        assert_eq!(
            pa.file_name().and_then(|n| n.to_str()),
            Some("fill-my-fill.json")
        );
        std::fs::write(dir.join("broken.json"), "{not json").expect("write");
        std::fs::write(dir.join("notes.txt"), "ignored").expect("write");
        std::fs::write(dir.join("huge.json"), vec![b' '; MAX_DOCUMENT + 1]).expect("write");
        std::fs::write(
            dir.join("reserved.json"),
            patterns::find(PatternKind::Motif, "heart")
                .expect("heart")
                .to_json(),
        )
        .expect("write");
        std::fs::create_dir_all(dir.join("folder.json")).expect("dir");
        let lib = load_library(&dir);
        let names: Vec<String> = lib
            .iter()
            .map(|e| {
                e.path
                    .file_name()
                    .expect("name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "broken.json",
                "fill-my-fill.json",
                "folder.json",
                "huge.json",
                "motif-star.json",
                "reserved.json"
            ]
        );
        let ok: Vec<&Pattern> = lib.iter().filter_map(|e| e.pattern.as_ref().ok()).collect();
        assert_eq!(ok, vec![&a, &b]);
        assert!(load_library(&dir.join("missing")).is_empty());
    }
}
