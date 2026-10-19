//! Per-command capabilities for automation clients (MCP, the JSON control channel): a client can
//! be limited to reading the design (read-only mode) and/or denied all file access.
//!
//! Enforced twice: [`crate::Session::execute`] refuses a command whose required capabilities are
//! not granted before it runs, and [`crate::EngineState::resolve`] refuses every file path when
//! file access is off, so a file command missing from [`FILE_READING_COMMANDS`] still cannot
//! touch the disk.

use crate::commands::{CommandSpec, Effect};
use crate::sandbox::Access;

/// Something a command may need beyond reading the open design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    /// Change the design or its history.
    Edit,
    /// Read files from disk.
    ReadFiles,
    /// Create or overwrite files.
    WriteFiles,
}

impl Capability {
    pub const ALL: [Capability; 3] = [
        Capability::Edit,
        Capability::ReadFiles,
        Capability::WriteFiles,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Edit => "edit",
            Capability::ReadFiles => "readFiles",
            Capability::WriteFiles => "writeFiles",
        }
    }
}

/// Commands that read a file (they also need [`Capability::ReadFiles`]).
#[cfg(not(feature = "digitize"))]
pub const FILE_READING_COMMANDS: &[&str] = &[
    "stitches.import",
    "project.open",
    "import.svg",
    "fonts.loadTrueType",
    "design.print",
    "pattern.import",
    "sewing.import",
    "threads.importCsv",
    "transfer.volumes",
    "fonts.mapCharacter",
    "fonts.mapFiles",
    "fonts.open",
    "nameDrop.readCsv",
    "nameDrop.writeCsv",
    "nameDrop.export",
    "template.list",
    "template.open",
];

/// Commands that read a file (they also need [`Capability::ReadFiles`]), including the image
/// conversions.
#[cfg(feature = "digitize")]
pub const FILE_READING_COMMANDS: &[&str] = &[
    "stitches.import",
    "project.open",
    "digitize.autoPunch",
    "digitize.crossStitch",
    "digitize.photoStitch",
    "digitize.lineImage",
    "digitize.figure",
    "digitize.designCenter",
    "import.svg",
    "background.set",
    "background.save",
    "fonts.loadTrueType",
    "design.print",
    "pattern.import",
    "sewing.import",
    "threads.importCsv",
    "transfer.volumes",
    "fonts.mapCharacter",
    "fonts.mapFiles",
    "fonts.open",
    "fonts.traceImage",
    "fonts.fromImage",
    "nameDrop.readCsv",
    "nameDrop.writeCsv",
    "nameDrop.export",
    "template.list",
    "template.open",
];

/// Design Database commands that read files (they also need [`Capability::ReadFiles`]):
/// browsing and searching are `Effect::Read`, so read-only mode keeps them; converting,
/// cataloguing and copying also write. Creating folders, renaming and moving only write.
#[cfg(feature = "library")]
pub const LIBRARY_READING_COMMANDS: &[&str] = &[
    "library.list",
    "library.info",
    "library.search",
    "library.convert",
    "library.catalog",
    "library.copy",
];

/// Whether a command reads files.
pub fn reads_files(id: &str) -> bool {
    #[cfg(feature = "library")]
    if LIBRARY_READING_COMMANDS.contains(&id) {
        return true;
    }
    FILE_READING_COMMANDS.contains(&id)
}

/// What a command needs: edits and history need `Edit`; file commands (`Effect::File`) write;
/// commands that [`reads_files`] read. Read commands need nothing.
pub fn required(spec: &CommandSpec) -> Vec<Capability> {
    let mut v = Vec::new();
    match spec.effect {
        Effect::Read => {}
        Effect::Edit | Effect::History | Effect::Settings => v.push(Capability::Edit),
        Effect::File => v.push(Capability::WriteFiles),
    }
    if reads_files(spec.id) {
        v.push(Capability::ReadFiles);
    }
    v
}

/// The capabilities granted to a session's client. Reading the design is always allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub edit: bool,
    pub read_files: bool,
    pub write_files: bool,
}

impl Default for Capabilities {
    fn default() -> Self {
        Capabilities::FULL
    }
}

impl Capabilities {
    pub const FULL: Capabilities = Capabilities {
        edit: true,
        read_files: true,
        write_files: true,
    };

    /// Read-only mode: inspect, measure and preview the design; no edits, no history, no file
    /// writes.
    pub fn read_only(self) -> Self {
        Capabilities {
            edit: false,
            write_files: false,
            ..self
        }
    }

    /// File access off: no command reads or writes any file.
    pub fn without_files(self) -> Self {
        Capabilities {
            read_files: false,
            write_files: false,
            ..self
        }
    }

    pub fn has(&self, c: Capability) -> bool {
        match c {
            Capability::Edit => self.edit,
            Capability::ReadFiles => self.read_files,
            Capability::WriteFiles => self.write_files,
        }
    }

    /// The first capability `spec` needs that is not granted.
    pub fn missing(&self, spec: &CommandSpec) -> Option<Capability> {
        required(spec).into_iter().find(|c| !self.has(*c))
    }

    pub fn allows(&self, spec: &CommandSpec) -> bool {
        self.missing(spec).is_none()
    }

    /// Whether a file may be opened for `access`.
    pub fn allows_access(&self, access: Access) -> bool {
        match access {
            Access::Read => self.read_files,
            Access::Write => self.write_files,
        }
    }

    /// Granted capabilities as names, for clients.
    pub fn names(&self) -> Vec<&'static str> {
        Capability::ALL
            .into_iter()
            .filter(|c| self.has(*c))
            .map(Capability::as_str)
            .collect()
    }
}
