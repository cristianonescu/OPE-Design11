//! Batch format conversion (guide: "Converting Embroidery Design Files to Different
//! Formats"). Sources are only read; each output is a new file.

use crate::info::{self, Kind};
use crate::ops::{self, Conflict};
use crate::{LibraryError, Limits, Result, io_err};
use openstitch_formats::FileType;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Where converted files go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    /// Next to each source file.
    SameFolder,
    Folder(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConvertOptions {
    pub target: FileType,
    pub output: Output,
    /// DST: jump records written per trim (guide: Option ▸ DST Settings).
    pub dst_trim_jumps: usize,
    pub conflict: Conflict,
}

impl ConvertOptions {
    pub fn new(target: FileType, output: Output) -> Self {
        ConvertOptions {
            target,
            output,
            dst_trim_jumps: openstitch_formats::dst::WriteOptions::default().trim_jumps,
            conflict: Conflict::Skip,
        }
    }
}

#[derive(Debug)]
pub enum Converted {
    Written {
        from: PathBuf,
        to: PathBuf,
    },
    /// Already in the target format and written to another folder: copied unchanged.
    Copied {
        from: PathBuf,
        to: PathBuf,
    },
    Skipped {
        from: PathBuf,
        reason: String,
    },
    Failed {
        from: PathBuf,
        error: LibraryError,
    },
}

impl Converted {
    pub fn source(&self) -> &Path {
        match self {
            Converted::Written { from, .. }
            | Converted::Copied { from, .. }
            | Converted::Skipped { from, .. }
            | Converted::Failed { from, .. } => from,
        }
    }
}

/// Converts each file. As in the guide: a file already in the target format is skipped when
/// the output is its own folder and copied when the output is another folder.
pub fn convert(files: &[PathBuf], options: &ConvertOptions, limits: &Limits) -> Vec<Converted> {
    files
        .iter()
        .map(|f| convert_one(f, options, limits))
        .collect()
}

fn convert_one(from: &Path, o: &ConvertOptions, limits: &Limits) -> Converted {
    let fail = |error| Converted::Failed {
        from: from.to_path_buf(),
        error,
    };
    if !o.target.can_write() {
        return fail(LibraryError::Refused(format!(
            "OpenStitch cannot write {} files",
            o.target.extension().to_uppercase()
        )));
    }
    let Some(kind) = Kind::of(from) else {
        return fail(LibraryError::Unsupported(from.to_path_buf()));
    };
    let dir = match &o.output {
        Output::SameFolder => from.parent().map(Path::to_path_buf).unwrap_or_default(),
        Output::Folder(d) => d.clone(),
    };
    let stem = from
        .file_stem()
        .map_or_else(|| "design".into(), |s| s.to_string_lossy().into_owned());
    let name = format!("{stem}.{}", o.target.extension());
    let same_format = kind == Kind::Machine(o.target);
    let same_folder = from.parent().is_some_and(|p| {
        match (std::fs::canonicalize(p), std::fs::canonicalize(&dir)) {
            (Ok(a), Ok(b)) => a == b,
            _ => p == dir,
        }
    });
    if same_format && same_folder {
        return Converted::Skipped {
            from: from.to_path_buf(),
            reason: "already in this format".into(),
        };
    }
    let target = match pick_target(&dir, &name, o.conflict) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Converted::Skipped {
                from: from.to_path_buf(),
                reason: format!("{name} already exists"),
            };
        }
        Err(e) => return fail(e),
    };
    let bytes = if same_format {
        info::read_bytes(from, limits)
    } else {
        info::load_pattern(from, limits).and_then(|p| encode(&p, o))
    };
    match bytes.and_then(|b| write_new(&target, &b)) {
        Ok(()) if same_format => Converted::Copied {
            from: from.to_path_buf(),
            to: target,
        },
        Ok(()) => Converted::Written {
            from: from.to_path_buf(),
            to: target,
        },
        Err(e) => fail(e),
    }
}

fn pick_target(dir: &Path, name: &str, conflict: Conflict) -> Result<Option<PathBuf>> {
    if !std::fs::metadata(dir).map_err(io_err(dir))?.is_dir() {
        return Err(LibraryError::Refused(format!(
            "{} is not a folder",
            dir.display()
        )));
    }
    let direct = dir.join(name);
    if std::fs::symlink_metadata(&direct).is_err() {
        return Ok(Some(direct));
    }
    Ok(match conflict {
        Conflict::Skip => None,
        Conflict::KeepBoth => Some(ops::free_name(dir, name)),
    })
}

pub(crate) fn encode(
    pattern: &openstitch_core::StitchPattern,
    o: &ConvertOptions,
) -> Result<Vec<u8>> {
    let r = match o.target {
        FileType::Dst => openstitch_formats::dst::write_with(
            pattern,
            openstitch_formats::dst::WriteOptions {
                trim_jumps: o.dst_trim_jumps,
                ..Default::default()
            },
        ),
        t => t.write(pattern),
    };
    r.map_err(|e| LibraryError::Refused(e.to_string()))
}

/// Writes a new file; fails if it exists (never overwrites) and removes a partial write.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                LibraryError::Exists(path.into())
            } else {
                LibraryError::Io {
                    path: path.into(),
                    error: e,
                }
            }
        })?;
    if let Err(e) = f.write_all(bytes).and_then(|()| f.sync_all()) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(LibraryError::Io {
            path: path.into(),
            error: e,
        });
    }
    Ok(())
}

/// Writes a file, replacing an existing one only when `overwrite` is set (the caller asked
/// the user). The data goes to a temporary file first, so a failed write leaves the old
/// file intact.
pub fn write_file(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    if !overwrite {
        return write_new(path, bytes);
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map_or_else(|| "output".into(), |n| n.to_string_lossy().into_owned());
    let tmp = ops::free_name(dir, &format!(".{name}.tmp"));
    write_new(&tmp, bytes)?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        LibraryError::Io {
            path: path.into(),
            error: e,
        }
    })
}
