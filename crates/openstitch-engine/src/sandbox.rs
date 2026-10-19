//! File roots for automation front ends (MCP, sockets). Write roots are also readable; read
//! roots are read-only; a sandbox with no roots refuses every path.
//!
//! A path is refused if it is empty, contains NUL, a `.`/`..` component or (outside Windows) a
//! backslash, or if its real location lies outside every root allowed for the access. The real
//! location is the file's canonical path when anything (even a dangling symlink) exists there,
//! else the parent's canonical path plus the file name; the parent must exist. The check runs
//! just before the command opens the file, so a process that changes the filesystem
//! concurrently could race it, and hard links are not detected.

use std::fmt;
use std::path::{Component, Path, PathBuf};

/// What a command does with a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

impl Access {
    pub fn as_str(self) -> &'static str {
        match self {
            Access::Read => "read",
            Access::Write => "write",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxError(pub String);

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SandboxError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sandbox {
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
}

impl Sandbox {
    /// Canonicalises every root; each must be an existing directory.
    pub fn new<R: AsRef<Path>, W: AsRef<Path>>(
        read_roots: &[R],
        write_roots: &[W],
    ) -> Result<Self, SandboxError> {
        fn root(p: &Path) -> Result<PathBuf, SandboxError> {
            let fail = || {
                SandboxError(format!(
                    "root is not an existing directory: {}",
                    p.display()
                ))
            };
            let real = p.canonicalize().map_err(|_| fail())?;
            if !real.is_dir() {
                return Err(fail());
            }
            Ok(real)
        }
        Ok(Sandbox {
            read_roots: read_roots
                .iter()
                .map(|r| root(r.as_ref()))
                .collect::<Result<_, _>>()?,
            write_roots: write_roots
                .iter()
                .map(|r| root(r.as_ref()))
                .collect::<Result<_, _>>()?,
        })
    }

    /// A sandbox that refuses every path.
    pub fn deny_all() -> Self {
        Sandbox::default()
    }

    pub fn read_roots(&self) -> &[PathBuf] {
        &self.read_roots
    }

    pub fn write_roots(&self) -> &[PathBuf] {
        &self.write_roots
    }

    /// Relative paths resolve against the first write root, else the first read root.
    pub fn base_dir(&self) -> Option<&Path> {
        self.write_roots
            .first()
            .or(self.read_roots.first())
            .map(PathBuf::as_path)
    }

    /// The canonical path to open, or why the path is refused.
    pub fn resolve(&self, path: &str, access: Access) -> Result<PathBuf, SandboxError> {
        let fail = |m: String| Err(SandboxError(m));
        let allowed: Vec<&PathBuf> = match access {
            Access::Write => self.write_roots.iter().collect(),
            Access::Read => self.write_roots.iter().chain(&self.read_roots).collect(),
        };
        if allowed.is_empty() {
            let flag = match access {
                Access::Write => "--root or --write-root",
                Access::Read => "--root or --read-root",
            };
            return fail(format!(
                "no {} root is configured; start the server with {flag}",
                access.as_str()
            ));
        }
        if path.is_empty() || path.contains('\0') || (cfg!(not(windows)) && path.contains('\\')) {
            return fail("invalid path".into());
        }
        let p = Path::new(path);
        // `components()` drops interior `.`; reject it textually too, like a leading one.
        if path.split(['/', '\\']).any(|c| c == "." || c == "..")
            || p.components()
                .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        {
            return fail(format!("path must not contain '.' or '..': {path}"));
        }
        let absolute = if p.is_absolute() {
            p.to_path_buf()
        } else {
            match self.base_dir() {
                Some(base) => base.join(p),
                None => return fail("no root to resolve relative paths against".into()),
            }
        };
        let Some(real) = canonicalize(&absolute) else {
            return fail(format!("cannot resolve {path} (its directory must exist)"));
        };
        if !allowed.iter().any(|root| real.starts_with(root)) {
            return fail(format!(
                "path is outside the allowed {} roots: {path}",
                access.as_str()
            ));
        }
        Ok(real)
    }
}

/// The real path if anything exists at `path` (a dangling symlink fails), else the parent's
/// real path plus the last component.
fn canonicalize(path: &Path) -> Option<PathBuf> {
    if std::fs::symlink_metadata(path).is_ok() {
        return path.canonicalize().ok();
    }
    let name = path.file_name()?;
    Some(path.parent()?.canonicalize().ok()?.join(name))
}
