//! Design Database commands (`library.*`) over `openstitch-library`: list folders with the
//! Property information, inspect one file, search, convert in batch, output catalogues, and
//! create, rename, copy and move files and folders. Deleting stays in the app, which moves
//! entries to the operating system's trash after the user confirms.
//!
//! Every path goes through [`EngineState::resolve`], so the sandbox roots and the client's
//! capabilities apply. Entries found by listing or walking a folder are vetted again before
//! they are opened: a symbolic link to a file outside the read roots is left out (counted in
//! `outsideRoots`), and walks never follow links into folders. Rename and move act on the
//! entry itself, so its parent folder must be writable, which also keeps a root from being
//! renamed or moved.

use crate::commands::{Effect, opt, req, spec};
use crate::params::{CommandError, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use crate::{CommandResult, CommandSpec};
use openstitch_core::Thread;
use openstitch_formats::FileType;
use openstitch_library::catalog::{self, ColorList, Paper, Setup, Style, Units};
use openstitch_library::convert::{self, ConvertOptions, Converted, Output};
use openstitch_library::ops::{self, Conflict, Outcome, Transfer};
use openstitch_library::search::Range;
use openstitch_library::{DesignInfo, Index, Kind, LibraryError, Limits, Query, info, scan};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Most rows one list or search returns (`limit`).
const MAX_ROWS: i64 = 10_000;
const DEFAULT_ROWS: i64 = 1_000;
/// Most source files one convert takes.
const MAX_CONVERT: usize = 10_000;
/// Most designs one catalogue lays out.
const MAX_CATALOG: usize = 2_000;
/// Most entries one copy or move takes.
const MAX_TRANSFER: usize = 10_000;

const CONFLICT: &str =
    "\"skip\" (default) leaves an existing target alone; \"keepBoth\" writes \"name (2).ext\"";
const SOURCES: &str = "Design files and/or folders (a folder adds its design files)";

pub(crate) fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "library.list",
            "List a folder: subfolders and design files with size, stitches, colours, sewing time and PES properties",
            Effect::Read,
            vec![
                req("path", "string", "Folder"),
                opt(
                    "info",
                    "bool",
                    "Decode each design for its Property information (default true)",
                ),
                opt(
                    "colors",
                    "bool",
                    "Include each design's thread lists (default false)",
                ),
                opt("offset", "int", "Skip this many design files (default 0)"),
                opt(
                    "limit",
                    "int",
                    "Most design files to return, 1-10000 (default 1000)",
                ),
            ],
            list,
        ),
        spec(
            "library.info",
            "Property information of one design file: size, stitches, jumps, trims, colour changes, thread lists, sewing time, PES properties",
            Effect::Read,
            vec![req("path", "string", "Design file")],
            info_one,
        ),
        spec(
            "library.search",
            "Search a folder (and its subfolders) by name, format, stitches, colours, size, date and PES properties; all given criteria must match",
            Effect::Read,
            vec![
                req("path", "string", "Folder to search"),
                opt(
                    "subfolders",
                    "bool",
                    "Search subfolders too (default false)",
                ),
                opt(
                    "name",
                    "string",
                    "File name: * and ? wildcards, else matches anywhere in the name",
                ),
                opt(
                    "formats",
                    "[string]",
                    "Extensions to include, e.g. [\"pes\", \"dst\"] (default all)",
                ),
                opt("stitches", "{min?, max?}", "Stitch count range"),
                opt("colorChanges", "{min?, max?}", "Colour change range"),
                opt("totalColors", "{min?, max?}", "Distinct thread range"),
                opt("widthMm", "{min?, max?}", "Width range, mm"),
                opt("heightMm", "{min?, max?}", "Height range, mm"),
                opt(
                    "modified",
                    "{min?, max?}",
                    "Modification time range, Unix seconds",
                ),
                opt(
                    "properties",
                    "object",
                    "PES properties (name, category, author, keywords, comments): every word must occur",
                ),
                opt(
                    "colors",
                    "bool",
                    "Include each design's thread lists (default false)",
                ),
                opt(
                    "limit",
                    "int",
                    "Most matches to return, 1-10000 (default 1000)",
                ),
            ],
            search,
        ),
        spec(
            "library.convert",
            "Convert design files to another format; sources are only read, outputs are new files",
            Effect::File,
            vec![
                req("paths", "[string]", SOURCES),
                req(
                    "format",
                    "string",
                    "Target extension: pes, pec, dst, exp, jef, pcs, sew, xxx, hus, vip",
                ),
                opt(
                    "output",
                    "string",
                    "Folder for the new files (default next to each source)",
                ),
                opt(
                    "subfolders",
                    "bool",
                    "Folders in paths add their subfolders' designs too (default false)",
                ),
                opt("conflict", "string", CONFLICT),
                opt(
                    "dstTrimJumps",
                    "int",
                    "DST: jump records written per trim, 0-8",
                ),
            ],
            convert_files,
        ),
        spec(
            "library.catalog",
            "Output a catalogue of designs as PDF, PNG pages, HTML (with an image folder) or CSV",
            Effect::File,
            vec![
                req("paths", "[string]", SOURCES),
                req(
                    "output",
                    "string",
                    "File to write; PNG with several pages writes name-1.png, name-2.png, …; HTML also writes a folder named after the file",
                ),
                opt(
                    "format",
                    "string",
                    "pdf, png, html or csv (default from the output extension)",
                ),
                opt(
                    "subfolders",
                    "bool",
                    "Folders in paths add their subfolders' designs too (default false)",
                ),
                opt(
                    "style",
                    "string | int",
                    "1, 4 or 12 designs per page, or \"instructions\" (default 4)",
                ),
                opt("paper", "string", "a4 (default) or letter"),
                opt("landscape", "bool", "Landscape pages (default false)"),
                opt(
                    "colorList",
                    "string",
                    "\"changes\" (default, every colour block) or \"totals\" (each thread once)",
                ),
                opt(
                    "brandNumbers",
                    "bool",
                    "Label threads by brand and number (default false)",
                ),
                opt("units", "string", "mm (default) or inch"),
                opt("title", "string", "Page and HTML title"),
                opt("dpi", "number", "PNG resolution, 36-300 (default 150)"),
                opt(
                    "overwrite",
                    "bool",
                    "Replace existing output (default false)",
                ),
            ],
            catalog_out,
        ),
        spec(
            "library.createFolder",
            "Create a folder",
            Effect::File,
            vec![
                req("path", "string", "Parent folder"),
                req("name", "string", "New folder name"),
            ],
            create_folder,
        ),
        spec(
            "library.rename",
            "Rename a design file (the extension is kept) or folder in place",
            Effect::File,
            vec![
                req("path", "string", "File or folder"),
                req("name", "string", "New name (for a file, without extension)"),
            ],
            rename,
        ),
        spec(
            "library.copy",
            "Copy files and folders into a folder; nothing is overwritten",
            Effect::File,
            vec![
                req("paths", "[string]", "Files and folders"),
                req("destination", "string", "Folder"),
                opt("conflict", "string", CONFLICT),
            ],
            |s, p| transfer(s, p, Transfer::Copy),
        ),
        spec(
            "library.move",
            "Move files and folders into a folder; nothing is overwritten",
            Effect::File,
            vec![
                req("paths", "[string]", "Files and folders"),
                req("destination", "string", "Folder"),
                opt("conflict", "string", CONFLICT),
            ],
            |s, p| transfer(s, p, Transfer::Move),
        ),
    ]
}

// MARK: paths

fn text(path: &Path) -> String {
    path.display().to_string()
}

fn lib_err(p: &Params, e: LibraryError) -> CommandError {
    p.failed(e.to_string())
}

/// A path found inside a folder, re-checked against the roots (it may be a link out of them).
fn vetted(s: &EngineState, path: &Path, access: Access) -> Option<PathBuf> {
    s.resolve(path.to_str()?, access, "library").ok()
}

/// The entry `raw` itself (not what a symbolic link points to), in a parent folder allowed
/// for `access`.
fn entry(s: &EngineState, p: &Params, raw: &str, access: Access) -> Result<PathBuf, CommandError> {
    s.resolve(raw, access, &p.command)?;
    let path = Path::new(raw);
    let name = path
        .file_name()
        .ok_or_else(|| p.failed(format!("{raw} has no file name")))?;
    let parent = match path.parent().filter(|q| !q.as_os_str().is_empty()) {
        Some(q) => q.to_path_buf(),
        None => s
            .sandbox
            .as_ref()
            .and_then(|b| b.base_dir())
            .map_or_else(|| s.base_dir.clone(), Path::to_path_buf),
    };
    let parent_text = parent
        .to_str()
        .ok_or_else(|| p.failed(format!("invalid path {raw}")))?;
    Ok(s.resolve(parent_text, access, &p.command)?.join(name))
}

fn folder(s: &EngineState, p: &Params, key: &str, access: Access) -> Result<PathBuf, CommandError> {
    let dir = s.resolve(&p.string(key, None)?, access, &p.command)?;
    if !dir.is_dir() {
        return Err(p.failed(format!("{} is not a folder", dir.display())));
    }
    // Already canonical inside a sandbox; without one, clean `.` out of reported paths.
    Ok(dir.canonicalize().unwrap_or(dir))
}

fn strings(p: &Params, key: &str, max: usize) -> Result<Vec<String>, CommandError> {
    let a = p.array(key)?;
    if a.is_empty() {
        return Err(p.invalid(key, "must not be empty"));
    }
    if a.len() > max {
        return Err(p.invalid(key, format!("at most {max} entries")));
    }
    a.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| p.invalid(key, "must be an array of strings"))
        })
        .collect()
}

/// Design files from `paths`: files as given, folders expanded (walked with `subfolders`).
struct Sources {
    files: Vec<PathBuf>,
    outside: usize,
    truncated: bool,
}

fn sources(
    s: &EngineState,
    p: &Params,
    max: usize,
    limits: &Limits,
) -> Result<Sources, CommandError> {
    let subfolders = p.bool("subfolders", Some(false))?;
    let mut out = Sources {
        files: Vec::new(),
        outside: 0,
        truncated: false,
    };
    for raw in strings(p, "paths", max)? {
        let path = s.resolve(&raw, Access::Read, &p.command)?;
        if path.is_dir() {
            let (found, truncated) = if subfolders {
                let w = scan::walk(&path, limits);
                (w.designs, w.truncated)
            } else {
                let l = scan::list(&path, limits).map_err(|e| lib_err(p, e))?;
                (l.designs, l.truncated)
            };
            out.truncated |= truncated;
            for f in found {
                match vetted(s, &f, Access::Read) {
                    Some(real) => out.files.push(real),
                    None => out.outside += 1,
                }
            }
        } else {
            out.files.push(path);
        }
        if out.files.len() > max {
            out.files.truncate(max);
            out.truncated = true;
            break;
        }
    }
    Ok(out)
}

// MARK: JSON

fn unix(t: SystemTime) -> Option<u64> {
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

fn thread_json(t: &Thread) -> Value {
    let mut m = Map::new();
    m.insert("color".into(), json!(t.hex()));
    for (k, v) in [
        ("name", &t.name),
        ("brand", &t.brand),
        ("number", &t.catalog_number),
    ] {
        if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
            m.insert(k.into(), json!(v));
        }
    }
    Value::Object(m)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn info_json(i: &DesignInfo, colors: bool) -> Value {
    let mut v = json!({
        "path": text(&i.path),
        "name": i.file_name(),
        "format": i.kind.extension(),
        "bytes": i.bytes,
        "modified": i.modified.and_then(unix),
    });
    match &i.summary {
        Ok(s) => {
            v["readable"] = json!(true);
            v["widthMm"] = json!(round1(s.width_mm));
            v["heightMm"] = json!(round1(s.height_mm));
            v["stitches"] = json!(s.stitches);
            v["jumps"] = json!(s.jumps);
            v["trims"] = json!(s.trims);
            v["colorChanges"] = json!(s.color_changes());
            v["totalColors"] = json!(s.total_colors());
            v["sewingMinutes"] = json!(round1(s.sewing_minutes));
            v["properties"] = json!(s.properties);
            if colors {
                v["colorSequence"] = s.sequence.iter().map(thread_json).collect();
                v["colors"] = s.totals.iter().map(thread_json).collect();
            }
        }
        Err(e) => {
            v["readable"] = json!(false);
            v["error"] = json!(e);
        }
    }
    v
}

fn plain_json(path: &Path) -> Value {
    json!({
        "path": text(path),
        "name": path.file_name().map(|n| n.to_string_lossy().into_owned()),
        "format": Kind::of(path).map(Kind::extension),
    })
}

// MARK: browse

fn limit(p: &Params) -> Result<usize, CommandError> {
    Ok(p.int("limit", Some(DEFAULT_ROWS), 1, MAX_ROWS)? as usize)
}

fn list(s: &mut EngineState, p: &Params) -> CommandResult {
    let dir = folder(s, p, "path", Access::Read)?;
    let decode = p.bool("info", Some(true))?;
    let colors = p.bool("colors", Some(false))?;
    let offset = p.int("offset", Some(0), 0, i64::MAX)? as usize;
    let limit = limit(p)?;
    let limits = Limits::default();
    let listing = scan::list(&dir, &limits).map_err(|e| lib_err(p, e))?;
    let mut outside = 0;
    let mut folders = Vec::new();
    for f in &listing.folders {
        match vetted(s, f, Access::Read) {
            Some(_) => folders.push(json!({
                "path": text(f),
                "name": f.file_name().map(|n| n.to_string_lossy().into_owned()),
            })),
            None => outside += 1,
        }
    }
    let files: Vec<PathBuf> = listing
        .designs
        .iter()
        .filter(|f| {
            let ok = vetted(s, f, Access::Read).is_some();
            outside += usize::from(!ok);
            ok
        })
        .cloned()
        .collect();
    let mut index = Index::default();
    let designs: Vec<Value> = files
        .iter()
        .skip(offset)
        .take(limit)
        .map(|f| match decode.then(|| index.info(f, &limits)) {
            Some(Ok(i)) => info_json(&i, colors),
            Some(Err(e)) => {
                let mut v = plain_json(f);
                v["readable"] = json!(false);
                v["error"] = json!(e.to_string());
                v
            }
            None => plain_json(f),
        })
        .collect();
    Ok(json!({
        "path": text(&dir),
        "folders": folders,
        "designs": designs,
        "designCount": files.len(),
        "more": offset.saturating_add(limit) < files.len(),
        "truncated": listing.truncated,
        "unreadable": listing.unreadable,
        "outsideRoots": outside,
    }))
}

fn info_one(s: &mut EngineState, p: &Params) -> CommandResult {
    let path = s.resolve(&p.string("path", None)?, Access::Read, &p.command)?;
    let (i, _) = info::inspect(&path, &Limits::default()).map_err(|e| lib_err(p, e))?;
    Ok(info_json(&i, true))
}

fn range<T: PartialOrd + Copy>(
    p: &Params,
    key: &str,
    conv: impl Fn(f64) -> Option<T>,
) -> Result<Range<T>, CommandError> {
    let Some(v) = p.get(key) else {
        return Ok(Range::default());
    };
    let obj = v
        .as_object()
        .ok_or_else(|| p.invalid(key, "must be {\"min\"?, \"max\"?}"))?;
    let end = |name: &str| -> Result<Option<T>, CommandError> {
        match obj.get(name).filter(|v| !v.is_null()) {
            None => Ok(None),
            Some(v) => v
                .as_f64()
                .filter(|n| n.is_finite())
                .and_then(&conv)
                .map(Some)
                .ok_or_else(|| p.invalid(key, format!("{name} must be a non-negative number"))),
        }
    };
    Ok(Range::new(end("min")?, end("max")?))
}

fn count(n: f64) -> Option<usize> {
    (n >= 0.0 && n <= usize::MAX as f64).then_some(n as usize)
}

fn query(p: &Params) -> Result<Query, CommandError> {
    let mut q = Query {
        name: p.get("name").map(|_| p.string("name", None)).transpose()?,
        stitches: range(p, "stitches", count)?,
        color_changes: range(p, "colorChanges", count)?,
        total_colors: range(p, "totalColors", count)?,
        width_mm: range(p, "widthMm", |n| (n >= 0.0).then_some(n))?,
        height_mm: range(p, "heightMm", |n| (n >= 0.0).then_some(n))?,
        modified: range(p, "modified", |n| {
            (0.0..=1e12)
                .contains(&n)
                .then(|| UNIX_EPOCH + Duration::from_secs_f64(n))
        })?,
        ..Query::default()
    };
    if p.has("formats") {
        for ext in strings(p, "formats", 64)? {
            let ext = ext.trim_start_matches('.');
            let kind = Kind::of(Path::new(&format!("x.{ext}")))
                .ok_or_else(|| p.invalid("formats", format!("unknown format '{ext}'")))?;
            q.kinds.push(kind);
        }
    }
    if let Some(v) = p.get("properties") {
        let obj = v
            .as_object()
            .ok_or_else(|| p.invalid("properties", "must be an object"))?;
        for (k, v) in obj {
            if !info::PROPERTY_KEYS.iter().any(|(key, _)| key == k) {
                return Err(p.invalid("properties", format!("unknown property '{k}'")));
            }
            let words = v
                .as_str()
                .ok_or_else(|| p.invalid("properties", "values must be strings"))?;
            q.properties.insert(k.clone(), words.to_string());
        }
    }
    Ok(q)
}

fn search(s: &mut EngineState, p: &Params) -> CommandResult {
    let dir = folder(s, p, "path", Access::Read)?;
    let subfolders = p.bool("subfolders", Some(false))?;
    let colors = p.bool("colors", Some(false))?;
    let limit = limit(p)?;
    let q = query(p)?;
    let limits = Limits::default();
    let (paths, truncated) = if subfolders {
        let w = scan::walk(&dir, &limits);
        (w.designs, w.truncated)
    } else {
        let l = scan::list(&dir, &limits).map_err(|e| lib_err(p, e))?;
        (l.designs, l.truncated)
    };
    let mut outside = 0;
    let paths: Vec<PathBuf> = paths
        .iter()
        .filter_map(|f| {
            let real = vetted(s, f, Access::Read);
            outside += usize::from(real.is_none());
            real.map(|_| f.clone())
        })
        .collect();
    let found = openstitch_library::search::search_in(
        &paths,
        truncated,
        &q,
        &limits,
        &mut Index::default(),
    );
    Ok(json!({
        "path": text(&dir),
        "designs": found.designs.iter().take(limit).map(|i| info_json(i, colors)).collect::<Vec<_>>(),
        "matches": found.designs.len(),
        "searched": found.searched,
        "more": found.designs.len() > limit,
        "truncated": found.truncated,
        "outsideRoots": outside,
    }))
}

// MARK: convert and catalogue

fn conflict(p: &Params) -> Result<Conflict, CommandError> {
    match p.string("conflict", Some("skip"))?.as_str() {
        "skip" => Ok(Conflict::Skip),
        "keepBoth" => Ok(Conflict::KeepBoth),
        _ => Err(p.invalid("conflict", "must be \"skip\" or \"keepBoth\"")),
    }
}

fn convert_files(s: &mut EngineState, p: &Params) -> CommandResult {
    let ext = p.string("format", None)?;
    let target = FileType::from_extension(ext.trim_start_matches('.'))
        .ok_or_else(|| p.invalid("format", format!("unknown format '{ext}'")))?;
    if !target.can_write() {
        return Err(p.invalid(
            "format",
            format!(
                "OpenStitch cannot write {} files",
                target.extension().to_uppercase()
            ),
        ));
    }
    let output = if p.has("output") {
        Some(folder(s, p, "output", Access::Write)?)
    } else {
        None
    };
    let mut options = ConvertOptions::new(target, Output::SameFolder);
    options.conflict = conflict(p)?;
    if p.has("dstTrimJumps") {
        options.dst_trim_jumps = p.int("dstTrimJumps", None, 0, 8)? as usize;
    }
    let limits = Limits::default();
    let src = sources(s, p, MAX_CONVERT, &limits)?;
    let mut results = Vec::new();
    let (mut written, mut failed) = (0, 0);
    for f in &src.files {
        let dir = match &output {
            Some(d) => Some(d.clone()),
            None => f
                .parent()
                .and_then(|d| vetted(s, d, Access::Write))
                .filter(|d| d.is_dir()),
        };
        let Some(dir) = dir else {
            failed += 1;
            results.push(json!({
                "from": text(f), "status": "failed",
                "reason": "the source's folder is outside the write roots; give an output folder",
            }));
            continue;
        };
        options.output = Output::Folder(dir);
        for r in convert::convert(std::slice::from_ref(f), &options, &limits) {
            results.push(match r {
                Converted::Written { from, to } | Converted::Copied { from, to } => {
                    written += 1;
                    json!({"from": text(&from), "status": "written", "to": text(&to)})
                }
                Converted::Skipped { from, reason } => {
                    json!({"from": text(&from), "status": "skipped", "reason": reason})
                }
                Converted::Failed { from, error } => {
                    failed += 1;
                    json!({"from": text(&from), "status": "failed", "reason": error.to_string()})
                }
            });
        }
    }
    Ok(json!({
        "results": results,
        "written": written,
        "failed": failed,
        "outsideRoots": src.outside,
        "truncated": src.truncated,
    }))
}

fn setup(p: &Params) -> Result<Setup, CommandError> {
    let style = match p.get("style") {
        None => Style::Four,
        Some(v) => match v
            .as_i64()
            .map(|n| n.to_string())
            .or(v.as_str().map(str::to_string))
            .as_deref()
        {
            Some("1") => Style::One,
            Some("4") => Style::Four,
            Some("12") => Style::Twelve,
            Some("instructions") => Style::Instructions,
            _ => return Err(p.invalid("style", "must be 1, 4, 12 or \"instructions\"")),
        },
    };
    let paper = match p.string("paper", Some("a4"))?.to_lowercase().as_str() {
        "a4" => Paper::A4,
        "letter" => Paper::Letter,
        _ => return Err(p.invalid("paper", "must be \"a4\" or \"letter\"")),
    };
    let color_list = match p.string("colorList", Some("changes"))?.as_str() {
        "changes" => ColorList::Changes,
        "totals" => ColorList::Totals,
        _ => return Err(p.invalid("colorList", "must be \"changes\" or \"totals\"")),
    };
    let units = match p.string("units", Some("mm"))?.as_str() {
        "mm" => Units::Mm,
        "inch" => Units::Inch,
        _ => return Err(p.invalid("units", "must be \"mm\" or \"inch\"")),
    };
    Ok(Setup {
        paper,
        landscape: p.bool("landscape", Some(false))?,
        style,
        color_list,
        brand_numbers: p.bool("brandNumbers", Some(false))?,
        units,
        title: p.string("title", Some(""))?,
    })
}

fn catalog_out(s: &mut EngineState, p: &Params) -> CommandResult {
    let raw = p.string("output", None)?;
    let format = match p.get("format") {
        Some(_) => p.string("format", None)?.to_lowercase(),
        None => Path::new(&raw)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .ok_or_else(|| p.invalid("format", "give a format or an output extension"))?,
    };
    if !matches!(format.as_str(), "pdf" | "png" | "html" | "csv") {
        return Err(p.invalid("format", "must be pdf, png, html or csv"));
    }
    let output = s.resolve(&raw, Access::Write, &p.command)?;
    let setup = setup(p)?;
    let overwrite = p.bool("overwrite", Some(false))?;
    let dpi = p.opt("dpi", 150.0, 36.0, 300.0)?;
    let limits = Limits::default();
    let src = sources(s, p, MAX_CATALOG, &limits)?;
    let items = catalog::load_items(&src.files, &limits);
    if items.is_empty() {
        return Err(p.failed("no design files to put in the catalogue"));
    }
    let write = |path: &Path, bytes: &[u8]| {
        convert::write_file(path, bytes, overwrite).map_err(|e| lib_err(p, e))
    };
    let written: Vec<PathBuf> = match format.as_str() {
        "pdf" => {
            write(&output, &catalog::pdf(&items, &setup))?;
            vec![output]
        }
        "csv" => {
            write(&output, catalog::csv(&items, &setup).as_bytes())?;
            vec![output]
        }
        "png" => {
            let pages = catalog::png_pages(&items, &setup, dpi);
            let targets = if pages.len() == 1 {
                vec![output]
            } else {
                let stem = output
                    .file_stem()
                    .map_or_else(|| "catalog".into(), |s| s.to_string_lossy().into_owned());
                let dir = output.parent().unwrap_or(Path::new(""));
                (1..=pages.len())
                    .map(|n| {
                        let t = dir.join(format!("{stem}-{n}.png"));
                        let t_text = t.to_str().ok_or_else(|| p.failed("invalid output path"))?;
                        s.resolve(t_text, Access::Write, &p.command)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            if !overwrite
                && let Some(t) = targets
                    .iter()
                    .find(|t| std::fs::symlink_metadata(t).is_ok())
            {
                return Err(p.failed(format!("{} already exists", t.display())));
            }
            for (t, page) in targets.iter().zip(&pages) {
                write(t, page)?;
            }
            targets
        }
        _ => {
            // The image folder sits next to the page; vet it too (it may be a link).
            let stem = output
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| p.invalid("output", "needs a file name"))?;
            let images = output.parent().unwrap_or(Path::new("")).join(stem);
            let images_text = images
                .to_str()
                .ok_or_else(|| p.failed("invalid output path"))?;
            s.resolve(images_text, Access::Write, &p.command)?;
            catalog::write_html(&items, &setup, &output, overwrite).map_err(|e| lib_err(p, e))?
        }
    };
    Ok(json!({
        "written": written.iter().map(|w| text(w)).collect::<Vec<_>>(),
        "designs": items.len(),
        "unreadable": items.iter().filter(|i| i.info.summary.is_err()).count(),
        "pages": catalog::page_count(items.len(), &setup),
        "outsideRoots": src.outside,
        "truncated": src.truncated,
    }))
}

// MARK: organise

fn create_folder(s: &mut EngineState, p: &Params) -> CommandResult {
    let parent = folder(s, p, "path", Access::Write)?;
    let made = ops::create_folder(&parent, &p.string("name", None)?).map_err(|e| lib_err(p, e))?;
    Ok(json!({ "path": text(&made) }))
}

fn rename(s: &mut EngineState, p: &Params) -> CommandResult {
    let path = entry(s, p, &p.string("path", None)?, Access::Write)?;
    let renamed = ops::rename(&path, &p.string("name", None)?).map_err(|e| lib_err(p, e))?;
    Ok(json!({ "path": text(&renamed) }))
}

fn transfer(s: &mut EngineState, p: &Params, mode: Transfer) -> CommandResult {
    let dest = folder(s, p, "destination", Access::Write)?;
    let conflict = conflict(p)?;
    let sources = strings(p, "paths", MAX_TRANSFER)?
        .iter()
        .map(|raw| match mode {
            // A copy reads what the path names; a move takes the entry out of its folder.
            Transfer::Copy => s.resolve(raw, Access::Read, &p.command),
            Transfer::Move => entry(s, p, raw, Access::Write),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outcomes = ops::transfer(&sources, &dest, mode, conflict, &Limits::default());
    let done = outcomes.iter().filter(|o| o.is_done()).count();
    let results: Vec<Value> = outcomes
        .into_iter()
        .map(|o| match o {
            Outcome::Done { from, to } => {
                json!({"from": text(&from), "status": "done", "to": text(&to)})
            }
            Outcome::Skipped { from, reason } => {
                json!({"from": text(&from), "status": "skipped", "reason": reason})
            }
            Outcome::Failed { from, error } => {
                json!({"from": text(&from), "status": "failed", "reason": error.to_string()})
            }
        })
        .collect();
    Ok(json!({ "results": results, "done": done }))
}
