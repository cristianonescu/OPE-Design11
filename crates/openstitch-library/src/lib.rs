//! The Design Database: browse folders of embroidery files, read their size, stitch and
//! colour information, draw thumbnails, search, organise files (new folder, copy, move,
//! rename, delete to the trash), convert formats in batch and output catalogues as PDF, PNG
//! pages, HTML and CSV.
//!
//! Behaviour follows the public PE-DESIGN 11 guide, chapter "Design Database"
//! (docs/research/pedesign11-manual-toc.md). Every file is treated as hostile: reads are
//! capped by [`Limits`], non-regular files are never opened, decoders run behind a panic
//! guard, folder walks never follow symbolic links into folders and stop at the depth and
//! file limits. Nothing here changes a design file: conversions and catalogues write new
//! files and refuse to overwrite unless the caller says so, and deletion goes to the trash.

#![forbid(unsafe_code)]

pub mod catalog;
pub mod convert;
pub mod index;
pub mod info;
pub mod ops;
pub mod scan;
pub mod search;
pub mod thumb;

#[cfg(test)]
mod tests;

pub use index::Index;
pub use info::{DesignInfo, Kind, Summary};
pub use search::Query;

use std::fmt;
use std::path::PathBuf;

/// Bounds on everything the library reads, so a hostile or huge folder cannot exhaust
/// memory or time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Larger files are listed but not decoded.
    pub max_file_bytes: u64,
    /// Entries read from one folder; the listing is marked truncated beyond this.
    pub max_folder_entries: usize,
    /// Design files a recursive walk (search, catalogue of subfolders) visits.
    pub max_walk_files: usize,
    /// Folders a recursive walk visits.
    pub max_walk_folders: usize,
    /// Folder depth below the starting folder.
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_file_bytes: 64 << 20,
            max_folder_entries: 50_000,
            max_walk_files: 100_000,
            max_walk_folders: 20_000,
            max_depth: 32,
        }
    }
}

/// Why a library operation failed.
#[derive(Debug)]
pub enum LibraryError {
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    /// Not a format the Design Database lists.
    Unsupported(PathBuf),
    TooLarge {
        path: PathBuf,
        bytes: u64,
        limit: u64,
    },
    /// A device, pipe, socket or other non-regular file.
    NotAFile(PathBuf),
    /// The decoder rejected the file (or panicked, which is reported the same way).
    Decode {
        path: PathBuf,
        reason: String,
    },
    InvalidName(String),
    /// The target exists and the operation never overwrites.
    Exists(PathBuf),
    /// A request that makes no sense (moving a folder into itself, writing a read-only format).
    Refused(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Unsupported(p) => write!(f, "{}: not an embroidery design file", p.display()),
            Self::TooLarge { path, bytes, limit } => write!(
                f,
                "{}: {bytes} bytes is larger than the {} MiB limit",
                path.display(),
                limit >> 20
            ),
            Self::NotAFile(p) => write!(f, "{}: not a regular file", p.display()),
            Self::Decode { path, reason } => write!(f, "{}: {reason}", path.display()),
            Self::InvalidName(s) => write!(f, "invalid name: {s}"),
            Self::Exists(p) => write!(f, "{} already exists", p.display()),
            Self::Refused(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for LibraryError {}

pub type Result<T> = std::result::Result<T, LibraryError>;

pub(crate) fn io_err(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> LibraryError {
    let path = path.into();
    move |error| LibraryError::Io { path, error }
}
