//! Design Database mode, without egui: the folder being browsed, the selection, display
//! options, the forms behind the Search, Convert Format and catalogue dialogs, and the file
//! actions they run through `openstitch-library`. `database_ui.rs` draws it.

use openstitch_core::StitchPattern;
use openstitch_engine::transfer::{SendError, write_to_volume};
use openstitch_formats::FileType;
use openstitch_library::catalog::{self, ColorList, Setup, Units};
use openstitch_library::convert::{self, ConvertOptions, Converted, Output};
use openstitch_library::ops::{self, Conflict, Outcome, Transfer, Trash};
use openstitch_library::search::{self, Query, Range};
use openstitch_library::{DesignInfo, Index, Kind, Limits, scan, thumb};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Display ▸ Large Thumbnails / Small Thumbnails / Details.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Large,
    Small,
    Details,
}

impl View {
    pub const ALL: [View; 3] = [View::Large, View::Small, View::Details];

    pub fn label(self) -> &'static str {
        match self {
            View::Large => "Large Thumbnails",
            View::Small => "Small Thumbnails",
            View::Details => "Details",
        }
    }

    pub fn from_key(key: &str) -> Option<View> {
        Some(match key {
            "large" => View::Large,
            "small" => View::Small,
            "details" => View::Details,
            _ => return None,
        })
    }

    /// Thumbnail side in points.
    pub fn thumb_size(self) -> f32 {
        match self {
            View::Large => 128.0,
            View::Small => 72.0,
            View::Details => 28.0,
        }
    }
}

/// Pixels thumbnails are rendered at (shown smaller in the small and details views).
pub const THUMB_PIXELS: u32 = 160;

/// Search dialog fields: a criterion is used only when its box is ticked.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchForm {
    pub use_name: bool,
    pub name: String,
    pub use_formats: bool,
    pub formats: BTreeSet<&'static str>,
    pub use_stitches: bool,
    pub stitches: (usize, usize),
    pub use_changes: bool,
    pub changes: (usize, usize),
    pub use_colors: bool,
    pub colors: (usize, usize),
    pub use_width: bool,
    /// In the display unit.
    pub width: (f64, f64),
    pub use_height: bool,
    pub height: (f64, f64),
    pub use_date: bool,
    /// `YYYY-MM-DD`, inclusive.
    pub date: (String, String),
    /// Design name, category, author, keywords, comment (PES properties).
    pub properties: [(bool, String); 5],
    pub subfolders: bool,
}

impl Default for SearchForm {
    fn default() -> Self {
        SearchForm {
            use_name: true,
            name: String::new(),
            use_formats: false,
            formats: Kind::extensions().into_iter().collect(),
            use_stitches: false,
            stitches: (0, 100_000),
            use_changes: false,
            changes: (1, 20),
            use_colors: false,
            colors: (1, 20),
            use_width: false,
            width: (0.0, 200.0),
            use_height: false,
            height: (0.0, 200.0),
            use_date: false,
            date: ("2000-01-01".into(), "2099-12-31".into()),
            properties: Default::default(),
            subfolders: true,
        }
    }
}

impl SearchForm {
    pub fn query(&self, units: Units) -> Result<Query, String> {
        let mm = |v: f64| match units {
            Units::Mm => v,
            Units::Inch => v * 25.4,
        };
        let pick = |on: bool, (a, b): (usize, usize)| {
            if on {
                Range::new(Some(a.min(b)), Some(a.max(b)))
            } else {
                Range::default()
            }
        };
        let pick_mm = |on: bool, (a, b): (f64, f64)| {
            if on {
                Range::new(Some(mm(a.min(b))), Some(mm(a.max(b))))
            } else {
                Range::default()
            }
        };
        let modified = if self.use_date {
            let from = parse_date(&self.date.0)?;
            // The end date includes its whole day.
            let to = parse_date(&self.date.1)? + Duration::from_secs(86_399);
            Range::new(Some(from.min(to)), Some(from.max(to)))
        } else {
            Range::default()
        };
        let kinds = if self.use_formats {
            if self.formats.is_empty() {
                return Err("tick at least one format".into());
            }
            self.formats
                .iter()
                .filter_map(|e| Kind::of(Path::new(&format!("x.{e}"))))
                .collect()
        } else {
            Vec::new()
        };
        let properties = openstitch_library::info::PROPERTY_KEYS
            .iter()
            .zip(&self.properties)
            .filter(|(_, (on, v))| *on && !v.trim().is_empty())
            .map(|((k, _), (_, v))| ((*k).to_string(), v.clone()))
            .collect();
        Ok(Query {
            name: (self.use_name && !self.name.trim().is_empty()).then(|| self.name.trim().into()),
            kinds,
            stitches: pick(self.use_stitches, self.stitches),
            color_changes: pick(self.use_changes, self.changes),
            total_colors: pick(self.use_colors, self.colors),
            width_mm: pick_mm(self.use_width, self.width),
            height_mm: pick_mm(self.use_height, self.height),
            modified,
            properties,
        })
    }
}

/// `YYYY-MM-DD` as midnight UTC.
pub fn parse_date(s: &str) -> Result<SystemTime, String> {
    let bad = || format!("'{s}' is not a date (YYYY-MM-DD)");
    let mut parts = s.trim().split('-');
    let mut num = |lo: i64, hi: i64| -> Result<i64, String> {
        let v: i64 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
        if (lo..=hi).contains(&v) {
            Ok(v)
        } else {
            Err(bad())
        }
    };
    let (y, m, d) = (num(1970, 9999)?, num(1, 12)?, num(1, 31)?);
    // Days from civil (Howard Hinnant).
    let yy = if m <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Ok(UNIX_EPOCH + Duration::from_secs(days as u64 * 86_400))
}

/// `YYYY-MM-DD HH:MM` (UTC) for the Property panel and details view.
pub fn format_time(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// File ▸ Convert Format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConvertForm {
    pub target: FileType,
    /// `None` = the same folder as each file.
    pub folder: Option<PathBuf>,
    pub dst_trim_jumps: usize,
}

impl Default for ConvertForm {
    fn default() -> Self {
        ConvertForm {
            target: FileType::Pes,
            folder: None,
            dst_trim_jumps: ConvertOptions::new(FileType::Dst, Output::SameFolder).dst_trim_jumps,
        }
    }
}

/// What a catalogue is written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CatalogFormat {
    /// Printable pages (File ▸ Print hands them to the system print dialog).
    #[default]
    Pdf,
    Png,
    Html,
    Csv,
}

impl CatalogFormat {
    pub const ALL: [CatalogFormat; 4] = [
        CatalogFormat::Pdf,
        CatalogFormat::Png,
        CatalogFormat::Html,
        CatalogFormat::Csv,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CatalogFormat::Pdf => "Printable pages (PDF)",
            CatalogFormat::Png => "Page images (PNG)",
            CatalogFormat::Html => "Image files and HTML",
            CatalogFormat::Csv => "CSV",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            CatalogFormat::Pdf => "pdf",
            CatalogFormat::Png => "png",
            CatalogFormat::Html => "html",
            CatalogFormat::Csv => "csv",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CatalogForm {
    pub format: CatalogFormat,
    pub setup: Setup,
    /// Only the selected designs (otherwise every design shown).
    pub selected_only: bool,
}

/// Designs being cut or copied for Paste.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub mode: Transfer,
}

/// Which dialog is open (at most one).
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Dialog {
    #[default]
    None,
    Search,
    Convert,
    Catalog,
    NewFolder(String),
    Rename(PathBuf, String),
    /// Delete to the trash, waiting for confirmation.
    ConfirmDelete(Vec<PathBuf>),
}

/// A decoded design for thumbnails and the preview.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub info: DesignInfo,
    pub pattern: Option<StitchPattern>,
    pub thumb: Option<thumb::Image>,
}

pub struct Database {
    pub open: bool,
    pub folder: Option<PathBuf>,
    pub listing: scan::Listing,
    /// Search results shown instead of the folder (with the search's description).
    pub results: Option<(String, Vec<PathBuf>)>,
    pub selection: BTreeSet<PathBuf>,
    anchor: Option<PathBuf>,
    pub view: View,
    pub units: Units,
    pub color_list: ColorList,
    pub show_preview: bool,
    pub limits: Limits,
    pub index: Index,
    loaded: HashMap<PathBuf, Loaded>,
    pub clipboard: Option<Clipboard>,
    pub dialog: Dialog,
    pub search: SearchForm,
    pub convert: ConvertForm,
    pub catalog: CatalogForm,
    /// Folder tree: folders to open (the tree takes them once) and cached subfolders.
    pub expanded: BTreeSet<PathBuf>,
    /// Scroll the tree to the current folder once.
    pub reveal: bool,
    subfolders: HashMap<PathBuf, Vec<PathBuf>>,
    pub status: String,
    /// Bumped whenever files change, so the UI drops stale textures.
    pub generation: u64,
    /// The Writing List: designs queued for Write to USB media, and whether it is shown.
    pub writing: Vec<PathBuf>,
    pub show_writing: bool,
}

impl Default for Database {
    fn default() -> Self {
        Database {
            open: false,
            folder: None,
            listing: scan::Listing::default(),
            results: None,
            selection: BTreeSet::new(),
            anchor: None,
            view: View::Large,
            units: Units::Mm,
            color_list: ColorList::Changes,
            show_preview: true,
            limits: Limits::default(),
            index: Index::default(),
            loaded: HashMap::new(),
            clipboard: None,
            dialog: Dialog::None,
            search: SearchForm::default(),
            convert: ConvertForm::default(),
            catalog: CatalogForm::default(),
            expanded: BTreeSet::new(),
            reveal: false,
            subfolders: HashMap::new(),
            status: String::new(),
            generation: 0,
            writing: Vec::new(),
            show_writing: false,
        }
    }
}

/// Top-level places for the folder tree: home, its usual design folders and the file
/// system roots (drive letters on Windows).
pub fn roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(h) = home() {
        out.push(h);
    }
    if cfg!(windows) {
        for letter in b'A'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", letter as char));
            if p.is_dir() {
                out.push(p);
            }
        }
    } else {
        out.push(PathBuf::from("/"));
        for v in ["/Volumes", "/media", "/mnt"] {
            if Path::new(v).is_dir() {
                out.push(PathBuf::from(v));
            }
        }
    }
    out
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .filter(|h| h.is_dir())
}

/// A path for display: inside the home folder it starts with `~`.
pub fn display_path(p: &Path) -> String {
    match home().and_then(|h| p.strip_prefix(&h).ok().map(Path::to_path_buf)) {
        Some(rel) if rel.as_os_str().is_empty() => "~".into(),
        Some(rel) => format!("~{}{}", std::path::MAIN_SEPARATOR, rel.display()),
        None => p.display().to_string(),
    }
}

/// Whether two paths are on the same volume (moving between volumes copies, as in the guide).
pub fn same_volume(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let prefix = |p: &Path| {
            std::fs::canonicalize(p).ok().and_then(|c| {
                c.components()
                    .next()
                    .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            })
        };
        prefix(a).is_some() && prefix(a) == prefix(b)
    }
}

impl Database {
    /// Opens the mode at `folder` (or keeps the current folder).
    pub fn show(&mut self, folder: Option<&Path>) {
        self.open = true;
        if let Some(f) = folder {
            self.navigate(f);
        } else if self.folder.is_none()
            && let Some(first) = roots().into_iter().next()
        {
            self.navigate(&first);
        }
    }

    /// Browses a folder; a file path browses its folder and selects the file.
    pub fn navigate(&mut self, path: &Path) {
        let (folder, select) = if path.is_file() {
            (
                path.parent().map(Path::to_path_buf).unwrap_or_default(),
                Some(path.to_path_buf()),
            )
        } else {
            (path.to_path_buf(), None)
        };
        self.folder = Some(folder.clone());
        self.results = None;
        self.selection.clear();
        self.anchor = None;
        // Reveal the folder in the tree.
        self.reveal = true;
        // Open the tree from the closest root (home before "/") down to the folder.
        let roots = roots();
        let top = roots
            .iter()
            .filter(|r| folder.starts_with(r))
            .max_by_key(|r| r.components().count());
        for a in folder.ancestors().skip(1) {
            self.expanded.insert(a.to_path_buf());
            if Some(a) == top.map(PathBuf::as_path) {
                break;
            }
        }
        self.refresh();
        if let Some(p) = select
            && self.listing.designs.contains(&p)
        {
            self.selection.insert(p.clone());
            self.anchor = Some(p);
        }
    }

    /// Display ▸ Refresh: re-reads the folder and drops cached thumbnails of changed files.
    pub fn refresh(&mut self) {
        self.subfolders.clear();
        self.loaded.clear();
        self.generation += 1;
        let Some(folder) = self.folder.clone() else {
            return;
        };
        match scan::list(&folder, &self.limits) {
            Ok(l) => {
                self.status = format!(
                    "{} designs, {} folders{}",
                    l.designs.len(),
                    l.folders.len(),
                    if l.truncated {
                        " (folder too large: list cut short)"
                    } else {
                        ""
                    }
                );
                self.listing = l;
            }
            Err(e) => {
                self.listing = scan::Listing::default();
                self.status = e.to_string();
            }
        }
        self.selection.retain(|p| p.exists());
    }

    /// The designs shown: search results or the folder's designs.
    pub fn items(&self) -> &[PathBuf] {
        match &self.results {
            Some((_, r)) => r,
            None => &self.listing.designs,
        }
    }

    /// Click on a design: plain replaces the selection, `toggle` (Ctrl/Cmd) adds or removes,
    /// `range` (Shift) selects from the last clicked design.
    pub fn click(&mut self, path: &Path, toggle: bool, range: bool) {
        let items = self.items().to_vec();
        if range
            && let Some(anchor) = &self.anchor
            && let (Some(a), Some(b)) = (
                items.iter().position(|p| p == anchor),
                items.iter().position(|p| p == path),
            )
        {
            if !toggle {
                self.selection.clear();
            }
            self.selection
                .extend(items[a.min(b)..=a.max(b)].iter().cloned());
            return;
        }
        if toggle {
            if !self.selection.remove(path) {
                self.selection.insert(path.to_path_buf());
            }
        } else {
            self.selection.clear();
            self.selection.insert(path.to_path_buf());
        }
        self.anchor = Some(path.to_path_buf());
    }

    pub fn select_all(&mut self) {
        self.selection = self.items().iter().cloned().collect();
    }

    /// Selected designs in display order.
    pub fn selected(&self) -> Vec<PathBuf> {
        self.items()
            .iter()
            .filter(|p| self.selection.contains(*p))
            .cloned()
            .collect()
    }

    /// The one selected design (preview and Property panel).
    pub fn focused(&self) -> Option<&PathBuf> {
        self.anchor
            .as_ref()
            .filter(|a| self.selection.contains(*a))
            .or_else(|| self.selection.iter().next())
    }

    /// Subfolders of `dir` for the tree (cached until Refresh).
    pub fn subfolders(&mut self, dir: &Path) -> &[PathBuf] {
        let limits = self.limits;
        self.subfolders.entry(dir.to_path_buf()).or_insert_with(|| {
            scan::list(dir, &limits)
                .map(|l| l.folders)
                .unwrap_or_default()
        })
    }

    /// The decoded design, if already loaded.
    pub fn loaded(&self, path: &Path) -> Option<&Loaded> {
        self.loaded.get(path)
    }

    /// Decodes up to `budget` designs that are not loaded yet, in `order`; returns how many
    /// are still waiting. The UI calls this each frame so a big folder never stalls it.
    pub fn load_some(&mut self, order: &[PathBuf], budget: usize) -> usize {
        let mut waiting = 0;
        let mut done = 0;
        for p in order {
            if self.loaded.contains_key(p) {
                continue;
            }
            if done >= budget {
                waiting += 1;
                continue;
            }
            done += 1;
            match self.index.inspect(p, &self.limits) {
                Ok((info, pattern)) => {
                    let thumb = pattern
                        .as_ref()
                        .map(|pt| thumb::render(pt, THUMB_PIXELS, [255, 255, 255]));
                    self.loaded.insert(
                        p.clone(),
                        Loaded {
                            info,
                            pattern,
                            thumb,
                        },
                    );
                }
                Err(e) => {
                    // Gone or not a regular file: keep a stub so it is not retried each frame.
                    self.loaded.insert(
                        p.clone(),
                        Loaded {
                            info: DesignInfo {
                                path: p.clone(),
                                kind: Kind::of(p).unwrap_or(Kind::Project),
                                bytes: 0,
                                modified: None,
                                summary: Err(e.to_string()),
                            },
                            pattern: None,
                            thumb: None,
                        },
                    );
                }
            }
        }
        waiting
    }

    fn changed(&mut self) {
        self.refresh();
    }

    // MARK: organising

    pub fn create_folder(&mut self, name: &str) -> Result<PathBuf, String> {
        let parent = self.folder.clone().ok_or("no folder is open")?;
        let p = ops::create_folder(&parent, name.trim()).map_err(|e| e.to_string())?;
        self.expanded.insert(parent);
        self.changed();
        self.status = format!("Created {}", p.display());
        Ok(p)
    }

    pub fn rename(&mut self, path: &Path, new_name: &str) -> Result<PathBuf, String> {
        let to = ops::rename(path, new_name.trim()).map_err(|e| e.to_string())?;
        self.index.forget(path);
        let was_selected = self.selection.remove(path);
        if let Some((_, r)) = &mut self.results {
            for p in r.iter_mut().filter(|p| p.as_path() == path) {
                *p = to.clone();
            }
        }
        self.changed();
        if was_selected {
            self.selection.insert(to.clone());
            self.anchor = Some(to.clone());
        }
        self.status = format!("Renamed to {}", to.display());
        Ok(to)
    }

    pub fn cut(&mut self) {
        self.set_clipboard(Transfer::Move);
    }

    pub fn copy(&mut self) {
        self.set_clipboard(Transfer::Copy);
    }

    fn set_clipboard(&mut self, mode: Transfer) {
        let paths = self.selected();
        if paths.is_empty() {
            return;
        }
        self.status = format!(
            "{} {} design(s): choose a folder and Paste",
            if mode == Transfer::Move {
                "Cut"
            } else {
                "Copied"
            },
            paths.len()
        );
        self.clipboard = Some(Clipboard { paths, mode });
    }

    /// Pastes into the open folder; a cut is used once.
    pub fn paste(&mut self) -> String {
        let (Some(c), Some(dest)) = (self.clipboard.clone(), self.folder.clone()) else {
            return "Nothing to paste".into();
        };
        if c.mode == Transfer::Move {
            self.clipboard = None;
        }
        self.transfer(&c.paths, &dest, c.mode)
    }

    /// Dropping designs on a folder: a move on the same volume, a copy across volumes
    /// (as in the guide); `force_copy` (Ctrl) copies, `force_move` (Shift) moves.
    pub fn drop_on(
        &mut self,
        paths: &[PathBuf],
        dest: &Path,
        force_copy: bool,
        force_move: bool,
    ) -> String {
        let same = paths.first().is_some_and(|p| same_volume(p, dest));
        let mode = if !force_copy && (force_move || same) {
            Transfer::Move
        } else {
            Transfer::Copy
        };
        self.transfer(paths, dest, mode)
    }

    fn transfer(&mut self, paths: &[PathBuf], dest: &Path, mode: Transfer) -> String {
        let outcomes = ops::transfer(paths, dest, mode, Conflict::KeepBoth, &self.limits);
        let done = outcomes.iter().filter(|o| o.is_done()).count();
        let mut msg = format!(
            "{} {done} of {} to {}",
            if mode == Transfer::Move {
                "Moved"
            } else {
                "Copied"
            },
            outcomes.len(),
            display_path(dest)
        );
        for o in &outcomes {
            match o {
                Outcome::Failed { from, error } => {
                    msg = format!("{msg} · {}: {error}", file_name(from));
                }
                Outcome::Skipped { from, reason } => {
                    msg = format!("{msg} · {} skipped: {reason}", file_name(from));
                }
                Outcome::Done { from, .. } if mode == Transfer::Move => self.index.forget(from),
                Outcome::Done { .. } => {}
            }
        }
        if let Some((_, r)) = &mut self.results
            && mode == Transfer::Move
        {
            r.retain(|p| p.exists());
        }
        self.changed();
        self.status = msg.clone();
        msg
    }

    /// Asks for confirmation before deleting the selection.
    pub fn request_delete(&mut self) {
        let paths = self.selected();
        if !paths.is_empty() {
            self.dialog = Dialog::ConfirmDelete(paths);
        }
    }

    /// Runs a confirmed delete.
    pub fn delete_confirmed(&mut self, trash: &dyn Trash) -> Result<usize, String> {
        let Dialog::ConfirmDelete(paths) = std::mem::take(&mut self.dialog) else {
            return Ok(0);
        };
        let n = ops::delete(&paths, trash).map_err(|e| e.to_string())?;
        for p in &paths {
            self.index.forget(p);
            self.selection.remove(p);
        }
        if let Some((_, r)) = &mut self.results {
            r.retain(|p| !paths.contains(p));
        }
        self.changed();
        self.status = format!("Moved {n} item(s) to the trash");
        Ok(n)
    }

    // MARK: search, convert, catalogue

    pub fn run_search(&mut self) -> Result<usize, String> {
        let folder = self.folder.clone().ok_or("no folder is open")?;
        let query = self.search.query(self.units)?;
        let found = search::search(
            &folder,
            &query,
            self.search.subfolders,
            &self.limits,
            &mut self.index,
        );
        let n = found.designs.len();
        let label = format!(
            "Search results in {}: {n} of {} designs{}",
            display_path(&folder),
            found.searched,
            if found.truncated {
                " (stopped at the search limit)"
            } else {
                ""
            }
        );
        self.status = label.clone();
        self.selection.clear();
        self.anchor = None;
        self.results = Some((label, found.designs.into_iter().map(|d| d.path).collect()));
        Ok(n)
    }

    pub fn clear_results(&mut self) {
        self.results = None;
        self.selection.clear();
        self.refresh();
    }

    /// Converts the selection; returns a one-line report.
    pub fn run_convert(&mut self) -> String {
        let files = self.selected();
        if files.is_empty() {
            return "Select designs to convert".into();
        }
        let f = &self.convert;
        let mut o = ConvertOptions::new(
            f.target,
            f.folder.clone().map_or(Output::SameFolder, Output::Folder),
        );
        o.dst_trim_jumps = f.dst_trim_jumps;
        o.conflict = Conflict::KeepBoth;
        let results = convert::convert(&files, &o, &self.limits);
        let count = |pred: fn(&Converted) -> bool| results.iter().filter(|r| pred(r)).count();
        let written = count(|r| matches!(r, Converted::Written { .. }));
        let copied = count(|r| matches!(r, Converted::Copied { .. }));
        let skipped = count(|r| matches!(r, Converted::Skipped { .. }));
        let mut msg = format!(
            "Converted {written} to {}",
            o.target.extension().to_uppercase()
        );
        if copied > 0 {
            msg += &format!(", copied {copied} already in that format");
        }
        if skipped > 0 {
            msg += &format!(", skipped {skipped} already in that format");
        }
        for r in &results {
            if let Converted::Failed { from, error } = r {
                msg += &format!(" · {}: {error}", file_name(from));
            }
        }
        self.changed();
        self.status = msg.clone();
        msg
    }

    /// The designs a catalogue covers.
    pub fn catalog_paths(&self) -> Vec<PathBuf> {
        if self.catalog.selected_only {
            self.selected()
        } else {
            self.items().to_vec()
        }
    }

    /// Writes the catalogue to `path` (the user chose it in a save dialog, which asked about
    /// replacing an existing file). Returns the files written.
    pub fn write_catalog(&mut self, path: &Path) -> Result<Vec<PathBuf>, String> {
        let paths = self.catalog_paths();
        if paths.is_empty() {
            return Err("No designs to put in the catalogue".into());
        }
        let items = catalog::load_items(&paths, &self.limits);
        let mut setup = self.catalog.setup.clone();
        setup.units = self.units;
        let written = match self.catalog.format {
            CatalogFormat::Pdf => {
                convert::write_file(path, &catalog::pdf(&items, &setup), true)
                    .map_err(|e| e.to_string())?;
                vec![path.to_path_buf()]
            }
            CatalogFormat::Csv => {
                convert::write_file(path, catalog::csv(&items, &setup).as_bytes(), true)
                    .map_err(|e| e.to_string())?;
                vec![path.to_path_buf()]
            }
            CatalogFormat::Html => {
                catalog::write_html(&items, &setup, path, true).map_err(|e| e.to_string())?
            }
            CatalogFormat::Png => {
                let pages = catalog::png_pages(&items, &setup, 150.0);
                let stem = path
                    .file_stem()
                    .map_or_else(|| "catalogue".into(), |s| s.to_string_lossy().into_owned());
                let dir = path.parent().unwrap_or(Path::new("."));
                let mut out = Vec::new();
                for (i, page) in pages.iter().enumerate() {
                    let p = if pages.len() == 1 {
                        path.to_path_buf()
                    } else {
                        dir.join(format!("{stem}-{}.png", i + 1))
                    };
                    convert::write_file(&p, page, true).map_err(|e| e.to_string())?;
                    out.push(p);
                }
                out
            }
        };
        self.status = format!(
            "Wrote catalogue of {} designs: {}",
            items.len(),
            written
                .last()
                .map_or_else(String::new, |p| p.display().to_string())
        );
        self.refresh();
        Ok(written)
    }
}

/// What Write does with a design whose name already exists on the destination (the guide's
/// overwrite, stop, or copy with a different name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnExists {
    /// Stop at the file and ask.
    Ask,
    Overwrite,
    /// Write it as "name (2)", "name (3)" and so on.
    NewName,
}

/// How far Write got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// Every design from the start index was written.
    Done {
        written: usize,
    },
    /// The design at `index` exists on the destination as `path`; nothing after it was written.
    Exists {
        index: usize,
        path: PathBuf,
        written: usize,
    },
    Failed {
        written: usize,
        error: String,
    },
}

/// The largest design file Write copies.
const MAX_WRITE_BYTES: u64 = 64 << 20;

impl Database {
    /// Prints the catalogue with the Print Setup options: writes the PDF to the temporary
    /// folder and returns its path for the system print dialog.
    pub fn print_catalog(&mut self) -> Result<PathBuf, String> {
        let paths = self.catalog_paths();
        if paths.is_empty() {
            return Err("No designs to print".into());
        }
        let items = catalog::load_items(&paths, &self.limits);
        let mut setup = self.catalog.setup.clone();
        setup.units = self.units;
        let path = crate::os_print::temp_pdf("catalogue");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        convert::write_file(&path, &catalog::pdf(&items, &setup), true)
            .map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// Whether File ▸ Print has designs to print.
    pub fn can_print(&self) -> bool {
        !self.catalog_paths().is_empty()
    }

    // MARK: writing list

    /// Add to Writing List: machine files not already queued. Returns how many were added.
    pub fn add_to_writing(&mut self, paths: &[PathBuf]) -> usize {
        let mut n = 0;
        for p in paths {
            if matches!(Kind::of(p), Some(Kind::Machine(_))) && !self.writing.contains(p) {
                self.writing.push(p.clone());
                n += 1;
            }
        }
        if n > 0 {
            self.show_writing = true;
        }
        n
    }

    /// Remove from Writing List.
    pub fn remove_from_writing(&mut self, paths: &[PathBuf]) {
        self.writing.retain(|p| !paths.contains(p));
    }

    /// Write: copies the queued designs, unchanged, to the root of `volume` (a USB stick or SD
    /// card from [`openstitch_engine::transfer::detect`]), starting at `from`. `first` decides
    /// a name clash on the first design; later clashes stop and ask.
    pub fn write_list(&self, volume: &Path, from: usize, first: OnExists) -> WriteOutcome {
        let mut written = 0;
        for (index, src) in self.writing.iter().enumerate().skip(from) {
            let on_exists = if index == from { first } else { OnExists::Ask };
            let failed = |error: String| WriteOutcome::Failed { written, error };
            let bytes = match std::fs::metadata(src) {
                Ok(m) if m.len() > MAX_WRITE_BYTES => {
                    return failed(format!("{} is too large", file_name(src)));
                }
                Ok(_) => match std::fs::read(src) {
                    Ok(b) => b,
                    Err(e) => return failed(format!("{}: {e}", file_name(src))),
                },
                Err(e) => return failed(format!("{}: {e}", file_name(src))),
            };
            let stem = src
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let ext = src
                .extension()
                .map_or_else(String::new, |e| e.to_string_lossy().to_lowercase());
            match write_one(volume, &stem, &ext, &bytes, on_exists) {
                Ok(()) => written += 1,
                Err(SendError::Exists(path)) => {
                    return WriteOutcome::Exists {
                        index,
                        path,
                        written,
                    };
                }
                Err(SendError::Failed(e)) => return failed(e),
            }
        }
        WriteOutcome::Done { written }
    }
}

fn write_one(
    volume: &Path,
    stem: &str,
    ext: &str,
    bytes: &[u8],
    on_exists: OnExists,
) -> Result<(), SendError> {
    let overwrite = on_exists == OnExists::Overwrite;
    match write_to_volume(volume, stem, ext, bytes, overwrite) {
        Err(SendError::Exists(_)) if on_exists == OnExists::NewName => {
            for n in 2..100 {
                match write_to_volume(volume, &format!("{stem} ({n})"), ext, bytes, false) {
                    Err(SendError::Exists(_)) => continue,
                    r => return r.map(|_| ()),
                }
            }
            Err(SendError::Failed(format!("no free name for {stem}.{ext}")))
        }
        r => r.map(|_| ()),
    }
}

pub fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Size text in the chosen unit.
pub fn size_text(units: Units, w_mm: f64, h_mm: f64) -> String {
    format!("{} × {}", units.format(w_mm), units.format(h_mm))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use openstitch_core::{Command, Thread};
    use openstitch_library::ops::FolderTrash;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Temp {
            static N: AtomicUsize = AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "openstitch-db-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Temp(p)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn design(path: &Path, size_mm: f64) {
        let mut p = StitchPattern::new();
        p.threads = vec![Thread::rgb(0xff0000), Thread::rgb(0x0000ff)];
        p.stitch_to(0.0, 0.0);
        p.stitch_to(size_mm * 10.0, size_mm * 10.0);
        p.push(Command::ColorChange);
        p.stitch_to(0.0, size_mm * 10.0);
        p.push(Command::End);
        let ty = FileType::from_path(path).unwrap();
        std::fs::write(path, ty.write(&p).unwrap()).unwrap();
    }

    fn setup() -> (Temp, Database) {
        let t = Temp::new();
        std::fs::create_dir(t.0.join("Sub")).unwrap();
        design(&t.0.join("a.pes"), 10.0);
        design(&t.0.join("b.dst"), 40.0);
        design(&t.0.join("c.jef"), 80.0);
        design(&t.0.join("Sub/d.pes"), 20.0);
        let mut db = Database::default();
        db.show(Some(&t.0));
        (t, db)
    }

    #[test]
    fn browsing_lists_designs_and_selects_a_file_path() {
        let (t, mut db) = setup();
        assert!(db.open);
        assert_eq!(db.items().len(), 3);
        assert_eq!(db.listing.folders, [t.0.join("Sub")]);
        db.navigate(&t.0.join("Sub/d.pes"));
        assert_eq!(db.folder.as_deref(), Some(t.0.join("Sub").as_path()));
        assert_eq!(db.focused(), Some(&t.0.join("Sub/d.pes")));
        assert!(db.expanded.contains(&t.0));
        assert_eq!(db.subfolders(&t.0).to_vec(), [t.0.join("Sub")]);
    }

    #[test]
    fn click_toggle_and_range_selection() {
        let (t, mut db) = setup();
        let [a, b, c] = ["a.pes", "b.dst", "c.jef"].map(|n| t.0.join(n));
        db.click(&a, false, false);
        db.click(&c, false, true);
        assert_eq!(db.selected(), [a.clone(), b.clone(), c.clone()]);
        db.click(&b, true, false);
        assert_eq!(db.selected(), [a.clone(), c.clone()]);
        db.click(&b, false, false);
        assert_eq!(db.selected(), std::slice::from_ref(&b));
        db.select_all();
        assert_eq!(db.selected().len(), 3);
    }

    #[test]
    fn loading_is_spread_over_frames() {
        let (_t, mut db) = setup();
        let order = db.items().to_vec();
        assert_eq!(db.load_some(&order, 2), 1);
        assert_eq!(db.load_some(&order, 2), 0);
        let l = db.loaded(&order[2]).unwrap();
        assert!(l.thumb.is_some());
        let s = l.info.summary.as_ref().unwrap();
        assert!((s.width_mm - 80.0).abs() < 0.5);
    }

    #[test]
    fn search_form_builds_queries_and_finds_designs() {
        let (_t, mut db) = setup();
        db.search.name = "*.pes".into();
        assert_eq!(db.run_search().unwrap(), 2, "subfolders included");
        db.search.subfolders = false;
        assert_eq!(db.run_search().unwrap(), 1);
        db.search = SearchForm {
            use_name: false,
            use_width: true,
            width: (30.0, 50.0),
            ..SearchForm::default()
        };
        assert_eq!(db.run_search().unwrap(), 1);
        db.units = Units::Inch;
        db.search.width = (3.0, 4.0); // 76–102 mm
        assert_eq!(db.run_search().unwrap(), 1);
        assert!(db.results.as_ref().unwrap().1[0].ends_with("c.jef"));
        db.search.use_formats = true;
        db.search.formats.clear();
        assert!(db.run_search().is_err());
        db.search.use_date = true;
        db.search.date.0 = "yesterday".into();
        assert!(db.search.query(Units::Mm).is_err());
        db.clear_results();
        assert_eq!(db.items().len(), 3);
    }

    #[test]
    fn dates_round_trip() {
        let t = parse_date("2026-10-19").unwrap();
        assert_eq!(format_time(t), "2026-10-19 00:00");
        assert_eq!(
            format_time(parse_date("2000-02-29").unwrap()),
            "2000-02-29 00:00"
        );
        for bad in ["2026-13-01", "26-1", "x", "1969-01-01"] {
            assert!(parse_date(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn folder_rename_cut_paste_and_delete() {
        let (t, mut db) = setup();
        let made = db.create_folder("Roses").unwrap();
        assert!(db.listing.folders.contains(&made));
        assert!(db.create_folder("Roses").is_err());
        assert!(db.create_folder("a/b").is_err());
        let a = t.0.join("a.pes");
        db.click(&a, false, false);
        let renamed = db.rename(&a, "rose").unwrap();
        assert_eq!(renamed, t.0.join("rose.pes"));
        assert!(db.selection.contains(&renamed));
        db.cut();
        db.navigate(&made);
        db.paste();
        assert!(made.join("rose.pes").exists());
        assert!(!renamed.exists());
        assert!(db.clipboard.is_none(), "a cut pastes once");
        // Copy then paste twice keeps both.
        db.navigate(&t.0);
        db.click(&t.0.join("b.dst"), false, false);
        db.copy();
        db.navigate(&made);
        db.paste();
        db.paste();
        assert!(made.join("b.dst").exists() && made.join("b (2).dst").exists());
        assert!(t.0.join("b.dst").exists());
        // Delete asks first, then goes to the trash.
        std::fs::create_dir(t.0.join("trash")).unwrap();
        db.select_all();
        db.request_delete();
        assert!(matches!(&db.dialog, Dialog::ConfirmDelete(p) if p.len() == 3));
        assert_eq!(
            db.delete_confirmed(&FolderTrash(t.0.join("trash")))
                .unwrap(),
            3
        );
        assert!(db.items().is_empty());
        assert_eq!(std::fs::read_dir(t.0.join("trash")).unwrap().count(), 3);
    }

    #[test]
    fn drop_moves_on_the_same_volume_and_copies_when_asked() {
        let (t, mut db) = setup();
        let sub = t.0.join("Sub");
        db.drop_on(&[t.0.join("a.pes")], &sub, false, false);
        assert!(sub.join("a.pes").exists() && !t.0.join("a.pes").exists());
        db.drop_on(&[t.0.join("b.dst")], &sub, true, false);
        assert!(sub.join("b.dst").exists() && t.0.join("b.dst").exists());
    }

    #[test]
    fn convert_and_catalogue_use_the_selection() {
        let (t, mut db) = setup();
        assert_eq!(db.run_convert(), "Select designs to convert");
        db.select_all();
        db.convert.target = FileType::Dst;
        let msg = db.run_convert();
        assert!(msg.starts_with("Converted 2 to DST, skipped 1"), "{msg}");
        assert!(t.0.join("a.dst").exists() && t.0.join("c.dst").exists());
        let out = t.0.join("Sub");
        db.convert.folder = Some(out.clone());
        let msg = db.run_convert();
        assert!(msg.contains("copied 1"), "{msg}");
        assert!(out.join("b.dst").exists());

        db.select_all();
        db.catalog.selected_only = true;
        db.catalog.format = CatalogFormat::Pdf;
        let pdf = t.0.join("cat.pdf");
        assert_eq!(db.write_catalog(&pdf).unwrap(), std::slice::from_ref(&pdf));
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
        db.catalog.format = CatalogFormat::Png;
        db.catalog.setup.style = openstitch_library::catalog::Style::One;
        let written = db.write_catalog(&t.0.join("cat.png")).unwrap();
        assert_eq!(written.len(), db.selected().len());
        db.catalog.format = CatalogFormat::Csv;
        db.write_catalog(&t.0.join("cat.csv")).unwrap();
        db.catalog.format = CatalogFormat::Html;
        db.write_catalog(&t.0.join("cat.html")).unwrap();
        assert!(t.0.join("cat").is_dir());
        db.selection.clear();
        assert!(db.write_catalog(&t.0.join("none.pdf")).is_err());
    }

    #[test]
    fn print_writes_the_catalogue_pdf_for_the_print_dialog() {
        let (_t, mut db) = setup();
        assert!(db.can_print());
        let pdf = db.print_catalog().unwrap();
        assert!(pdf.starts_with(std::env::temp_dir()));
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
        let _ = std::fs::remove_file(&pdf);
        db.catalog.selected_only = true;
        assert!(!db.can_print());
        assert!(db.print_catalog().is_err());
    }

    #[test]
    fn the_writing_list_queues_designs_and_writes_them_to_the_volume() {
        let (t, mut db) = setup();
        let usb = Temp::new();
        let items = db.items().to_vec();
        let mut queued = items.clone();
        queued.push(t.0.join("Sub")); // not a design
        assert!(!db.show_writing);
        assert_eq!(db.add_to_writing(&queued), 3);
        assert!(db.show_writing);
        assert_eq!(db.add_to_writing(&items[..1]), 0, "no duplicates");
        db.remove_from_writing(&items[1..2]);
        assert_eq!(db.writing, [items[0].clone(), items[2].clone()]);

        assert_eq!(
            db.write_list(&usb.0, 0, OnExists::Ask),
            WriteOutcome::Done { written: 2 }
        );
        for p in &db.writing {
            let copy = usb.0.join(p.file_name().unwrap());
            assert_eq!(std::fs::read(copy).unwrap(), std::fs::read(p).unwrap());
        }
        // Writing again stops at the first clash and asks.
        let again = db.write_list(&usb.0, 0, OnExists::Ask);
        let WriteOutcome::Exists {
            index,
            path,
            written,
        } = again
        else {
            panic!("{again:?}")
        };
        assert_eq!((index, written), (0, 0));
        assert_eq!(path, usb.0.join("a.pes"));
        // Copy with a different name, then overwrite the next clash.
        let next = db.write_list(&usb.0, 0, OnExists::NewName);
        assert!(usb.0.join("a (2).pes").exists());
        assert!(
            matches!(
                next,
                WriteOutcome::Exists {
                    index: 1,
                    written: 1,
                    ..
                }
            ),
            "{next:?}"
        );
        assert_eq!(
            db.write_list(&usb.0, 1, OnExists::Overwrite),
            WriteOutcome::Done { written: 1 }
        );
        assert_eq!(std::fs::read_dir(&usb.0).unwrap().count(), 3);
        // A missing source or volume fails cleanly.
        let gone = usb.0.join("missing");
        assert!(matches!(
            db.write_list(&gone, 0, OnExists::Overwrite),
            WriteOutcome::Failed { written: 0, .. }
        ));
    }
}
