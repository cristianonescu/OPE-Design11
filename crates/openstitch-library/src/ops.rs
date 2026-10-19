//! Organising files (guide: "Organizing Embroidery Designs"): new folder, copy, move, rename,
//! delete to the trash. Nothing is ever overwritten: an existing target is skipped or the
//! copy gets a free name ("rose (2).pes"); rename keeps the extension, as in the guide.

use crate::{LibraryError, Limits, Result, io_err};
use std::path::{Component, Path, PathBuf};

/// Names Windows reserves in every folder (also refused elsewhere, so files stay portable).
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Checks a single file or folder name typed by the user.
pub fn validate_name(name: &str) -> Result<()> {
    let bad = |why: &str| Err(LibraryError::InvalidName(format!("'{name}': {why}")));
    if name.trim().is_empty() {
        return bad("the name is empty");
    }
    if name == "." || name == ".." {
        return bad("not a name");
    }
    if name.len() > 255 {
        return bad("longer than 255 bytes");
    }
    if let Some(c) = name.chars().find(|c| {
        c.is_control() || matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*')
    }) {
        return bad(&format!("contains '{}'", c.escape_default()));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return bad("ends with a dot or space");
    }
    let stem = name.split('.').next().unwrap_or("").trim().to_lowercase();
    if RESERVED.contains(&stem.as_str()) {
        return bad("reserved by Windows");
    }
    Ok(())
}

fn exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// Creates `parent/name`; fails if it exists.
pub fn create_folder(parent: &Path, name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let path = parent.join(name);
    std::fs::create_dir(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            LibraryError::Exists(path.clone())
        } else {
            LibraryError::Io {
                path: path.clone(),
                error: e,
            }
        }
    })?;
    Ok(path)
}

/// Renames a file or folder in place. For files `new_name` is the name without extension
/// and the extension is kept (it cannot be changed, as in the guide); an extension typed
/// by the user that equals the current one is accepted. Fails if the target exists, except
/// for a change of letter case only.
pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf> {
    let meta = std::fs::symlink_metadata(path).map_err(io_err(path))?;
    let parent = path
        .parent()
        .ok_or_else(|| LibraryError::Refused(format!("cannot rename {}", path.display())))?;
    let name = if meta.is_dir() {
        new_name.to_string()
    } else {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => {
                let typed = Path::new(new_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .filter(|e| e.eq_ignore_ascii_case(ext));
                let stem = match typed {
                    Some(_) => &new_name[..new_name.len() - ext.len() - 1],
                    None => new_name,
                };
                format!("{stem}.{ext}")
            }
            None => new_name.to_string(),
        }
    };
    validate_name(&name)?;
    let target = parent.join(&name);
    if target == path {
        return Ok(target);
    }
    let case_only = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(&name) || n.to_lowercase() == name.to_lowercase());
    if exists(&target) && !(case_only && same_file(path, &target)) {
        return Err(LibraryError::Exists(target));
    }
    if case_only {
        // Through a free temporary name, so case-insensitive file systems apply the change.
        let tmp = free_name(parent, &format!(".{name}.renaming"));
        std::fs::rename(path, &tmp).map_err(io_err(path))?;
        std::fs::rename(&tmp, &target).map_err(io_err(&tmp))?;
    } else {
        std::fs::rename(path, &target).map_err(io_err(path))?;
    }
    Ok(target)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// `dir/name` if free, else `dir/stem (2).ext`, `(3)`, … (up to 9999).
pub fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !exists(&first) {
        return first;
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map_or_else(|| name.to_string(), |s| s.to_string_lossy().into_owned());
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..10_000 {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !exists(&candidate) {
            return candidate;
        }
    }
    dir.join(format!("{stem} (copy){ext}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    Copy,
    Move,
}

/// What to do when the destination already has an entry of the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Conflict {
    #[default]
    Skip,
    /// Give the new entry a free name ("rose (2).pes").
    KeepBoth,
}

/// One entry's result.
#[derive(Debug)]
pub enum Outcome {
    Done { from: PathBuf, to: PathBuf },
    Skipped { from: PathBuf, reason: String },
    Failed { from: PathBuf, error: LibraryError },
}

impl Outcome {
    pub fn is_done(&self) -> bool {
        matches!(self, Outcome::Done { .. })
    }
}

/// Copies or moves files and folders into `dest`. Folders are copied recursively within
/// `limits` (symbolic links inside are skipped, never followed); a folder cannot go into
/// itself. Moving across drives copies and then removes the source only after the copy
/// fully succeeded.
pub fn transfer(
    sources: &[PathBuf],
    dest: &Path,
    mode: Transfer,
    conflict: Conflict,
    limits: &Limits,
) -> Vec<Outcome> {
    sources
        .iter()
        .map(
            |from| match transfer_one(from, dest, mode, conflict, limits) {
                Ok(Some(to)) => Outcome::Done {
                    from: from.clone(),
                    to,
                },
                Ok(None) => Outcome::Skipped {
                    from: from.clone(),
                    reason: "already exists at the destination".into(),
                },
                Err(error) => Outcome::Failed {
                    from: from.clone(),
                    error,
                },
            },
        )
        .collect()
}

fn transfer_one(
    from: &Path,
    dest: &Path,
    mode: Transfer,
    conflict: Conflict,
    limits: &Limits,
) -> Result<Option<PathBuf>> {
    let meta = std::fs::symlink_metadata(from).map_err(io_err(from))?;
    let name = from
        .file_name()
        .ok_or_else(|| LibraryError::Refused(format!("{} has no name", from.display())))?;
    if !std::fs::metadata(dest).map_err(io_err(dest))?.is_dir() {
        return Err(LibraryError::Refused(format!(
            "{} is not a folder",
            dest.display()
        )));
    }
    if meta.is_dir() {
        let (src, dst) = (
            std::fs::canonicalize(from).map_err(io_err(from))?,
            std::fs::canonicalize(dest).map_err(io_err(dest))?,
        );
        if dst.starts_with(&src) {
            return Err(LibraryError::Refused(format!(
                "cannot put {} inside itself",
                from.display()
            )));
        }
    }
    let same_folder = from.parent().is_some_and(|p| same_file(p, dest));
    if same_folder && mode == Transfer::Move {
        return Ok(None);
    }
    let direct = dest.join(name);
    let target = if exists(&direct) {
        match conflict {
            Conflict::Skip => return Ok(None),
            Conflict::KeepBoth => free_name(dest, &name.to_string_lossy()),
        }
    } else {
        direct
    };
    match mode {
        Transfer::Copy => copy_entry(from, &target, limits, &mut 0)?,
        Transfer::Move => match std::fs::rename(from, &target) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                copy_entry(from, &target, limits, &mut 0)?;
                let removed = if meta.is_dir() {
                    std::fs::remove_dir_all(from)
                } else {
                    std::fs::remove_file(from)
                };
                removed.map_err(io_err(from))?;
            }
            Err(e) => {
                return Err(LibraryError::Io {
                    path: from.into(),
                    error: e,
                });
            }
        },
    }
    Ok(Some(target))
}

/// Copies a file (never overwriting: the target is created new) or a folder tree.
fn copy_entry(from: &Path, to: &Path, limits: &Limits, count: &mut usize) -> Result<()> {
    let meta = std::fs::symlink_metadata(from).map_err(io_err(from))?;
    *count += 1;
    if *count > limits.max_walk_files {
        return Err(LibraryError::Refused(format!(
            "more than {} entries to copy",
            limits.max_walk_files
        )));
    }
    if meta.is_file() {
        let mut src = std::fs::File::open(from).map_err(io_err(from))?;
        let mut dst = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    LibraryError::Exists(to.into())
                } else {
                    LibraryError::Io {
                        path: to.into(),
                        error: e,
                    }
                }
            })?;
        if let Err(e) = std::io::copy(&mut src, &mut dst) {
            drop(dst);
            let _ = std::fs::remove_file(to);
            return Err(LibraryError::Io {
                path: to.into(),
                error: e,
            });
        }
        if let Ok(m) = meta.modified() {
            let _ = dst.set_modified(m);
        }
        Ok(())
    } else if meta.is_dir() {
        copy_dir(from, to, limits, count, 0)
    } else {
        // Symbolic links, devices, pipes: left alone.
        Ok(())
    }
}

fn copy_dir(
    from: &Path,
    to: &Path,
    limits: &Limits,
    count: &mut usize,
    depth: usize,
) -> Result<()> {
    if depth > limits.max_depth {
        return Err(LibraryError::Refused(format!(
            "{} is nested deeper than {} folders",
            from.display(),
            limits.max_depth
        )));
    }
    std::fs::create_dir(to).map_err(io_err(to))?;
    let mut entries: Vec<_> = std::fs::read_dir(from)
        .map_err(io_err(from))?
        .take(limits.max_folder_entries)
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Ok(ft) = e.file_type() else { continue };
        let target = to.join(e.file_name());
        if ft.is_dir() {
            copy_dir(&e.path(), &target, limits, count, depth + 1)?;
        } else if ft.is_file() {
            copy_entry(&e.path(), &target, limits, count)?;
        }
    }
    Ok(())
}

/// Where deleted entries go. The app uses [`SystemTrash`]; tests use a folder.
pub trait Trash {
    fn delete(&self, paths: &[PathBuf]) -> std::result::Result<(), String>;
}

/// The operating system's trash (Finder Trash, Recycle Bin, freedesktop.org trash).
#[cfg(feature = "system-trash")]
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemTrash;

#[cfg(feature = "system-trash")]
impl Trash for SystemTrash {
    fn delete(&self, paths: &[PathBuf]) -> std::result::Result<(), String> {
        trash::delete_all(paths).map_err(|e| e.to_string())
    }
}

/// A trash that moves entries into a folder (free names on collision).
#[derive(Clone, Debug)]
pub struct FolderTrash(pub PathBuf);

impl Trash for FolderTrash {
    fn delete(&self, paths: &[PathBuf]) -> std::result::Result<(), String> {
        for p in paths {
            let name = p.file_name().ok_or("no name")?.to_string_lossy();
            let to = free_name(&self.0, &name);
            std::fs::rename(p, &to).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(())
    }
}

/// Moves entries to the trash after the caller confirmed. Refuses roots and paths with `..`.
pub fn delete(paths: &[PathBuf], trash: &dyn Trash) -> Result<usize> {
    for p in paths {
        let has_parent = p.components().any(|c| c == Component::ParentDir);
        if has_parent || p.parent().is_none_or(|q| q.as_os_str().is_empty()) {
            return Err(LibraryError::Refused(format!(
                "will not delete {}",
                p.display()
            )));
        }
        std::fs::symlink_metadata(p).map_err(io_err(p))?;
    }
    trash.delete(paths).map_err(LibraryError::Refused)?;
    Ok(paths.len())
}
