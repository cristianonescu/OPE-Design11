//! Reading one design file: its stitch pattern and the Property information the Design
//! Database shows (size, stitches, colours, sewing time, PES design properties).

use crate::{LibraryError, Limits, Result, io_err};
use openstitch_core::{Command, StitchPattern, Thread};
use openstitch_formats::FileType;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What kind of design file a path holds, from its extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Machine(FileType),
    /// An OpenStitch `.osd` project.
    Project,
}

impl Kind {
    pub fn of(path: &Path) -> Option<Kind> {
        let ext = path.extension()?.to_str()?;
        if ext.eq_ignore_ascii_case(openstitch_design::project::EXTENSION) {
            return Some(Kind::Project);
        }
        FileType::from_extension(ext).map(Kind::Machine)
    }

    /// Lower-case extension without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Kind::Machine(t) => t.extension(),
            Kind::Project => openstitch_design::project::EXTENSION,
        }
    }

    /// Every extension the Design Database lists.
    pub fn extensions() -> Vec<&'static str> {
        let mut v: Vec<_> = FileType::ALL.iter().map(|t| t.extension()).collect();
        v.push(openstitch_design::project::EXTENSION);
        v
    }
}

/// The PES design properties the guide lists (PES 4.0 and later), as stored by the reader.
pub const PROPERTY_KEYS: [(&str, &str); 5] = [
    ("name", "Design name"),
    ("category", "Category"),
    ("author", "Author"),
    ("keywords", "Keywords"),
    ("comments", "Comment"),
];

/// Sewing information of a decoded design.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub width_mm: f64,
    pub height_mm: f64,
    /// Needle penetrations.
    pub stitches: usize,
    pub jumps: usize,
    pub trims: usize,
    /// Thread of each colour block in sewing order (the guide's "Color changes" list).
    pub sequence: Vec<Thread>,
    /// Distinct threads in order of first use (the guide's "Total colors" list).
    pub totals: Vec<Thread>,
    /// Estimated with OpenStitch's default machine settings (an estimate, as in the guide).
    pub sewing_minutes: f64,
    /// Design name, category, author, keywords, comments when the file has them.
    pub properties: BTreeMap<String, String>,
}

impl Summary {
    pub fn of(pattern: &StitchPattern) -> Summary {
        let (width_mm, height_mm) = if pattern.count(Command::Stitch) > 0 {
            let b = pattern.bounds();
            (b.width() / 10.0, b.height() / 10.0)
        } else {
            (0.0, 0.0)
        };
        let sequence: Vec<Thread> = if pattern.count(Command::Stitch) == 0 {
            Vec::new()
        } else {
            pattern
                .color_blocks()
                .into_iter()
                .filter(|(_, s)| s.iter().any(|s| s.command == Command::Stitch))
                .map(|(t, _)| t.cloned().unwrap_or_else(|| Thread::rgb(0)))
                .collect()
        };
        let mut totals: Vec<Thread> = Vec::new();
        for t in &sequence {
            if !totals.iter().any(|u| same_thread(u, t)) {
                totals.push(t.clone());
            }
        }
        let estimate = openstitch_design::estimate(pattern, &Default::default());
        let properties = PROPERTY_KEYS
            .iter()
            .filter_map(|(k, _)| {
                let v = pattern.metadata.get(*k)?.trim();
                (!v.is_empty()).then(|| ((*k).to_string(), v.to_string()))
            })
            .collect();
        Summary {
            width_mm,
            height_mm,
            stitches: pattern.count(Command::Stitch),
            jumps: pattern.count(Command::Jump),
            trims: pattern.count(Command::Trim),
            sequence,
            totals,
            sewing_minutes: estimate.seconds / 60.0,
            properties,
        }
    }

    pub fn color_changes(&self) -> usize {
        self.sequence.len()
    }

    pub fn total_colors(&self) -> usize {
        self.totals.len()
    }
}

fn same_thread(a: &Thread, b: &Thread) -> bool {
    a.rgb_value() == b.rgb_value()
        && a.name == b.name
        && a.brand == b.brand
        && a.catalog_number == b.catalog_number
}

/// One design file as the Design Database lists it.
#[derive(Clone, Debug)]
pub struct DesignInfo {
    pub path: PathBuf,
    pub kind: Kind,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
    /// `Err` holds why the file could not be read; it is still listed.
    pub summary: std::result::Result<Summary, String>,
}

impl DesignInfo {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }

    pub fn folder(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }
}

/// Reads and decodes a design; the pattern is `None` when the file cannot be read.
pub fn inspect(path: &Path, limits: &Limits) -> Result<(DesignInfo, Option<StitchPattern>)> {
    let kind = Kind::of(path).ok_or_else(|| LibraryError::Unsupported(path.to_path_buf()))?;
    let meta = regular_file(path)?;
    let modified = meta.modified().ok();
    let decoded = load_pattern(path, limits);
    let (summary, pattern) = match decoded {
        Ok(p) => (Ok(Summary::of(&p)), Some(p)),
        Err(e) => (Err(e.to_string()), None),
    };
    Ok((
        DesignInfo {
            path: path.to_path_buf(),
            kind,
            bytes: meta.len(),
            modified,
            summary,
        },
        pattern,
    ))
}

/// Follows a symbolic link and requires a regular file, so pipes and devices are never opened.
pub(crate) fn regular_file(path: &Path) -> Result<std::fs::Metadata> {
    let meta = std::fs::metadata(path).map_err(io_err(path))?;
    if !meta.is_file() {
        return Err(LibraryError::NotAFile(path.to_path_buf()));
    }
    Ok(meta)
}

/// Reads at most `limits.max_file_bytes` (a file that grows while being read is refused).
pub fn read_bytes(path: &Path, limits: &Limits) -> Result<Vec<u8>> {
    let meta = regular_file(path)?;
    let too_large = |bytes| LibraryError::TooLarge {
        path: path.to_path_buf(),
        bytes,
        limit: limits.max_file_bytes,
    };
    if meta.len() > limits.max_file_bytes {
        return Err(too_large(meta.len()));
    }
    let file = std::fs::File::open(path).map_err(io_err(path))?;
    let mut data = Vec::with_capacity(meta.len() as usize);
    file.take(limits.max_file_bytes + 1)
        .read_to_end(&mut data)
        .map_err(io_err(path))?;
    if data.len() as u64 > limits.max_file_bytes {
        return Err(too_large(data.len() as u64));
    }
    Ok(data)
}

/// Decodes a design file into stitches. Projects are compiled; machine files go through
/// their reader. A panicking decoder is reported as a decode error, never propagated.
pub fn load_pattern(path: &Path, limits: &Limits) -> Result<StitchPattern> {
    let kind = Kind::of(path).ok_or_else(|| LibraryError::Unsupported(path.to_path_buf()))?;
    let data = read_bytes(path, limits)?;
    decode(kind, &data).map_err(|reason| LibraryError::Decode {
        path: path.to_path_buf(),
        reason,
    })
}

/// Decodes bytes of a known kind behind a panic guard.
pub fn decode(kind: Kind, data: &[u8]) -> std::result::Result<StitchPattern, String> {
    let run = || -> std::result::Result<StitchPattern, String> {
        match kind {
            Kind::Machine(t) => t.read(data).map_err(|e| e.to_string()),
            Kind::Project => {
                let design = openstitch_design::project::decode(data).map_err(|e| e.to_string())?;
                let mut p = openstitch_design::Compiler::for_design(&design).compile(&design);
                if !design.name.is_empty() {
                    p.set_name(design.name.clone());
                }
                Ok(p)
            }
        }
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        .unwrap_or_else(|_| Err("the decoder failed on this file".into()))
}

/// A thread's label for colour lists: its name, or brand plus catalogue number, falling back
/// to the hex colour.
pub fn thread_label(t: &Thread, brand_and_number: bool) -> String {
    if brand_and_number {
        let parts: Vec<&str> = [t.brand.as_deref(), t.catalog_number.as_deref()]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect();
        if !parts.is_empty() {
            return parts.join(" ");
        }
    }
    match t.name.as_deref() {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => t.hex(),
    }
}
