//! Search (guide: "Searching for an Embroidery Design"): every criterion is optional and all
//! given criteria must match.

use crate::info::{DesignInfo, Kind};
use crate::{Index, Limits, scan};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::SystemTime;

/// An inclusive range; either end may be open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T> Default for Range<T> {
    fn default() -> Self {
        Range {
            min: None,
            max: None,
        }
    }
}

impl<T: PartialOrd + Copy> Range<T> {
    pub fn new(min: Option<T>, max: Option<T>) -> Self {
        Range { min, max }
    }

    pub fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    pub fn contains(&self, v: T) -> bool {
        self.min.is_none_or(|m| v >= m) && self.max.is_none_or(|m| v <= m)
    }
}

/// Search criteria.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// File name: `*` matches any run of characters, `?` one character, case-insensitively,
    /// against the whole name. Without a wildcard the text matches anywhere in the name.
    pub name: Option<String>,
    /// Formats to include; empty means all.
    pub kinds: Vec<Kind>,
    pub stitches: Range<usize>,
    pub color_changes: Range<usize>,
    pub total_colors: Range<usize>,
    pub width_mm: Range<f64>,
    pub height_mm: Range<f64>,
    pub modified: Range<SystemTime>,
    /// PES properties (`name`, `category`, `author`, `keywords`, `comments`): the value is
    /// split at spaces and every word must occur in the property, case-insensitively.
    pub properties: BTreeMap<String, String>,
}

impl Query {
    /// Whether only file-system facts (name, format, date) are asked, so files need not be
    /// decoded.
    fn needs_summary(&self) -> bool {
        !(self.stitches.is_open()
            && self.color_changes.is_open()
            && self.total_colors.is_open()
            && self.width_mm.is_open()
            && self.height_mm.is_open()
            && self.properties.values().all(|v| v.trim().is_empty()))
    }

    pub fn matches(&self, info: &DesignInfo) -> bool {
        if let Some(pattern) = self.name.as_deref().filter(|s| !s.is_empty())
            && !name_matches(pattern, &info.file_name())
        {
            return false;
        }
        if !self.kinds.is_empty() && !self.kinds.contains(&info.kind) {
            return false;
        }
        if !self.modified.is_open() && !info.modified.is_some_and(|m| self.modified.contains(m)) {
            return false;
        }
        if !self.needs_summary() {
            return true;
        }
        let Ok(s) = &info.summary else {
            return false;
        };
        self.stitches.contains(s.stitches)
            && self.color_changes.contains(s.color_changes())
            && self.total_colors.contains(s.total_colors())
            && self.width_mm.contains(s.width_mm)
            && self.height_mm.contains(s.height_mm)
            && self.properties.iter().all(|(key, words)| {
                let have = s
                    .properties
                    .get(key)
                    .map(|v| v.to_lowercase())
                    .unwrap_or_default();
                words
                    .split_whitespace()
                    .all(|w| have.contains(&w.to_lowercase()))
            })
    }
}

/// Wildcard match (`*`, `?`), case-insensitive; plain text matches as a substring.
pub fn name_matches(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    if !p.iter().any(|c| matches!(c, '*' | '?')) {
        let needle: String = p.iter().collect();
        return name.to_lowercase().contains(&needle);
    }
    // Iterative glob with backtracking to the last `*`: linear in practice, never exponential.
    let (mut i, mut j) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some((i, j));
            i += 1;
        } else if let Some((si, sj)) = star {
            i = si + 1;
            j = sj + 1;
            star = Some((si, sj + 1));
        } else {
            return false;
        }
    }
    p[i..].iter().all(|c| *c == '*')
}

/// Search results.
#[derive(Clone, Debug, Default)]
pub struct Found {
    pub designs: Vec<DesignInfo>,
    /// The walk stopped at a limit.
    pub truncated: bool,
    pub searched: usize,
}

/// Searches `folder` (and its subfolders when asked) through the index.
pub fn search(
    folder: &Path,
    query: &Query,
    subfolders: bool,
    limits: &Limits,
    index: &mut Index,
) -> Found {
    let (paths, truncated) = if subfolders {
        let w = scan::walk(folder, limits);
        (w.designs, w.truncated)
    } else {
        match scan::list(folder, limits) {
            Ok(l) => (l.designs, l.truncated),
            Err(_) => (Vec::new(), false),
        }
    };
    search_in(&paths, truncated, query, limits, index)
}

/// Matches already collected design paths (`truncated`: the collection stopped at a limit).
/// Callers that must vet each path first (the engine's sandbox) collect with
/// [`scan::list`] or [`scan::walk`], filter, then call this.
pub fn search_in(
    paths: &[std::path::PathBuf],
    truncated: bool,
    query: &Query,
    limits: &Limits,
    index: &mut Index,
) -> Found {
    let mut out = Found {
        truncated,
        ..Found::default()
    };
    for p in paths {
        out.searched += 1;
        if let Some(pattern) = query.name.as_deref().filter(|s| !s.is_empty()) {
            let name = p
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            if !name_matches(pattern, &name) {
                continue;
            }
        }
        if let Ok(info) = index.info(p, limits)
            && query.matches(&info)
        {
            out.designs.push(info);
        }
    }
    out
}
