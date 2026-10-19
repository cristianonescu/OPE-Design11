//! Listing folders and walking folder trees within [`Limits`].

use crate::info::Kind;
use crate::{Limits, Result, io_err};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The contents of one folder: subfolders and design files, each sorted by name
/// (case-insensitively). Hidden entries (names starting with `.`) are left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listing {
    pub folders: Vec<PathBuf>,
    pub designs: Vec<PathBuf>,
    /// The folder had more entries than [`Limits::max_folder_entries`].
    pub truncated: bool,
    /// Entries that could not be read.
    pub unreadable: usize,
}

/// Lists one folder. Symbolic links are listed as what they point to; broken links and
/// links to devices are left out.
pub fn list(dir: &Path, limits: &Limits) -> Result<Listing> {
    let mut out = Listing::default();
    let entries = std::fs::read_dir(dir).map_err(io_err(dir))?;
    for (n, entry) in entries.enumerate() {
        if n >= limits.max_folder_entries {
            out.truncated = true;
            break;
        }
        let Ok(entry) = entry else {
            out.unreadable += 1;
            continue;
        };
        let path = entry.path();
        if is_hidden(&path) {
            continue;
        }
        let Ok(ft) = entry.file_type() else {
            out.unreadable += 1;
            continue;
        };
        let (is_dir, is_file) = if ft.is_symlink() {
            match std::fs::metadata(&path) {
                Ok(m) => (m.is_dir(), m.is_file()),
                Err(_) => continue,
            }
        } else {
            (ft.is_dir(), ft.is_file())
        };
        if is_dir {
            out.folders.push(path);
        } else if is_file && Kind::of(&path).is_some() {
            out.designs.push(path);
        }
    }
    out.folders.sort_by_key(|p| sort_key(p));
    out.designs.sort_by_key(|p| sort_key(p));
    Ok(out)
}

fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

fn sort_key(p: &Path) -> String {
    p.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().to_lowercase())
}

/// What a walk covered.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Walk {
    pub designs: Vec<PathBuf>,
    pub folders_visited: usize,
    /// A limit (files, folders or entries per folder) cut the walk short.
    pub truncated: bool,
    /// Folders deeper than [`Limits::max_depth`] that were not entered.
    pub too_deep: usize,
    /// Folders that could not be read.
    pub unreadable: usize,
    /// Symbolic links to folders, which a walk never follows.
    pub links_skipped: usize,
}

/// Collects design files under `root` (including `root` itself), breadth first. Symbolic
/// links to folders are never followed and each real folder is visited once, so link loops
/// and bind-mount cycles end; depth, folder and file counts are capped by `limits`.
pub fn walk(root: &Path, limits: &Limits) -> Walk {
    let mut out = Walk::default();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut queue = std::collections::VecDeque::from([(root.to_path_buf(), 0usize)]);
    while let Some((dir, depth)) = queue.pop_front() {
        if out.folders_visited >= limits.max_walk_folders {
            out.truncated = true;
            break;
        }
        let real = std::fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
        if !seen.insert(real) {
            continue;
        }
        out.folders_visited += 1;
        let Ok(entries) = std::fs::read_dir(&dir) else {
            out.unreadable += 1;
            continue;
        };
        let mut subfolders = Vec::new();
        let mut designs = Vec::new();
        for (n, entry) in entries.enumerate() {
            if n >= limits.max_folder_entries {
                out.truncated = true;
                break;
            }
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if is_hidden(&path) {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                match std::fs::metadata(&path) {
                    Ok(m) if m.is_dir() => out.links_skipped += 1,
                    Ok(m) if m.is_file() && Kind::of(&path).is_some() => designs.push(path),
                    _ => {}
                }
            } else if ft.is_dir() {
                subfolders.push(path);
            } else if ft.is_file() && Kind::of(&path).is_some() {
                designs.push(path);
            }
        }
        designs.sort_by_key(|p| sort_key(p));
        for d in designs {
            if out.designs.len() >= limits.max_walk_files {
                out.truncated = true;
                return out;
            }
            out.designs.push(d);
        }
        subfolders.sort_by_key(|p| sort_key(p));
        for f in subfolders {
            if queue.len() >= limits.max_walk_folders {
                out.truncated = true;
                break;
            }
            if depth < limits.max_depth {
                queue.push_back((f, depth + 1));
            } else {
                out.too_deep += 1;
            }
        }
    }
    out
}
