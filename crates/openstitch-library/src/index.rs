//! A cache of [`DesignInfo`] keyed by path, refreshed when a file's size or modification
//! time changes, so browsing and searching the same folders decodes each file once.

use crate::info::{self, DesignInfo};
use crate::{Limits, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Bounded: the cache is cleared when it would grow past `capacity` entries.
#[derive(Debug)]
pub struct Index {
    entries: HashMap<PathBuf, (u64, Option<SystemTime>, DesignInfo)>,
    capacity: usize,
    /// Files decoded (cache misses), for tests and the status bar.
    pub decoded: usize,
}

impl Default for Index {
    fn default() -> Self {
        Index::with_capacity(200_000)
    }
}

impl Index {
    pub fn with_capacity(capacity: usize) -> Self {
        Index {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            decoded: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The file's information, decoding it only when it is new or changed on disk.
    pub fn info(&mut self, path: &Path, limits: &Limits) -> Result<DesignInfo> {
        let meta = info::regular_file(path)?;
        let stamp = (meta.len(), meta.modified().ok());
        if let Some((len, modified, info)) = self.entries.get(path)
            && (*len, *modified) == stamp
        {
            return Ok(info.clone());
        }
        let (info, _) = info::inspect(path, limits)?;
        self.decoded += 1;
        if self.entries.len() >= self.capacity {
            self.entries.clear();
        }
        self.entries
            .insert(path.to_path_buf(), (stamp.0, stamp.1, info.clone()));
        Ok(info)
    }

    /// Reads a file once for both its information and its stitches (for thumbnails), and
    /// caches the information. The pattern is `None` when the file cannot be decoded.
    pub fn inspect(
        &mut self,
        path: &Path,
        limits: &Limits,
    ) -> Result<(DesignInfo, Option<openstitch_core::StitchPattern>)> {
        let meta = info::regular_file(path)?;
        let (info, pattern) = info::inspect(path, limits)?;
        self.decoded += 1;
        if self.entries.len() >= self.capacity {
            self.entries.clear();
        }
        self.entries.insert(
            path.to_path_buf(),
            (meta.len(), meta.modified().ok(), info.clone()),
        );
        Ok((info, pattern))
    }

    /// Forgets a path (after it was renamed, moved or deleted).
    pub fn forget(&mut self, path: &Path) {
        self.entries.retain(|p, _| !p.starts_with(path));
    }
}
