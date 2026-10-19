//! Transfer via USB media: find removable volumes (USB sticks, SD cards) and write the
//! machine file to the root of one, as PE-DESIGN's "Send to USB Media" does.
//!
//! Detection uses only the operating system's own tools and files, no FFI:
//! - macOS: `/Volumes/*`, each checked with `diskutil info -plist` (ejectable, on a USB or SD
//!   bus; disk images and network shares are left out);
//! - Linux: `/proc/mounts`, keeping filesystems on block devices that sysfs marks removable or
//!   that sit on a USB bus;
//! - Windows: PowerShell `Get-CimInstance Win32_LogicalDisk` with drive type 2 (removable).
//!
//! `transfer.send` only writes to a volume detection reports at that moment, to its root, under
//! a plain file name, and never overwrites unless asked.

use crate::commands::{CommandSpec, Effect, ParamDoc, RunFn};
use crate::params::{CommandError, CommandResult, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_core::Command;
use openstitch_design::Compiler;
use openstitch_formats::FileType;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", windows))]
use std::process::Command as Process;

/// A mounted removable volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub path: PathBuf,
    pub name: String,
}

/// Removable volumes mounted now.
pub fn detect() -> Vec<Volume> {
    #[cfg(target_os = "macos")]
    return macos();
    #[cfg(target_os = "linux")]
    return linux();
    #[cfg(windows)]
    return windows();
    #[allow(unreachable_code)]
    Vec::new()
}

// MARK: macOS

/// The value after `<key>key</key>` in `diskutil -plist` output: `true`, `false` or a string.
pub fn plist_value(plist: &str, key: &str) -> Option<String> {
    let at = plist.find(&format!("<key>{key}</key>"))? + key.len() + 11;
    let rest = plist[at..].trim_start();
    if rest.starts_with("<true/>") {
        return Some("true".into());
    }
    if rest.starts_with("<false/>") {
        return Some("false".into());
    }
    let s = rest.strip_prefix("<string>")?;
    Some(s[..s.find("</string>")?].to_string())
}

/// Whether `diskutil info -plist` describes a removable USB or SD volume.
pub fn macos_is_removable(plist: &str) -> bool {
    let yes = |k| plist_value(plist, k).as_deref() == Some("true");
    let bus = plist_value(plist, "BusProtocol").unwrap_or_default();
    let usb_or_sd = ["USB", "Secure Digital", "SD"]
        .iter()
        .any(|b| bus.eq_ignore_ascii_case(b));
    (yes("Ejectable") || yes("RemovableMedia") || yes("Removable")) && usb_or_sd
        || (usb_or_sd && plist_value(plist, "Internal").as_deref() == Some("false"))
}

#[cfg(target_os = "macos")]
fn macos() -> Vec<Volume> {
    let Ok(entries) = std::fs::read_dir("/Volumes") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        // The boot volume appears as a link to "/".
        if std::fs::canonicalize(&path).is_ok_and(|p| p == Path::new("/")) {
            continue;
        }
        let Ok(o) = Process::new("/usr/sbin/diskutil")
            .args(["info", "-plist"])
            .arg(&path)
            .output()
        else {
            continue;
        };
        if o.status.success() && macos_is_removable(&String::from_utf8_lossy(&o.stdout)) {
            out.push(Volume {
                name: e.file_name().to_string_lossy().into_owned(),
                path,
            });
        }
    }
    out
}

// MARK: Linux

/// `/proc/mounts` escapes spaces and a few other bytes as `\ooo`.
fn unescape_mount(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c))
        {
            out.push(((b[i + 1] - b'0') << 6) | ((b[i + 2] - b'0') << 3) | (b[i + 3] - b'0'));
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The whole-disk name of a partition device: `sdb1` → `sdb`, `mmcblk0p1` → `mmcblk0`,
/// `nvme0n1p2` → `nvme0n1`.
pub fn disk_of(device: &str) -> String {
    let name = device.rsplit('/').next().unwrap_or(device);
    if name.starts_with("mmcblk") || name.starts_with("nvme") {
        // Partitions end in `p<digits>`; the disk name itself ends in a digit.
        match name.rsplit_once('p') {
            Some((disk, part))
                if !part.is_empty()
                    && part.chars().all(|c| c.is_ascii_digit())
                    && disk.len() > 4 =>
            {
                disk.to_string()
            }
            _ => name.to_string(),
        }
    } else {
        name.trim_end_matches(|c: char| c.is_ascii_digit())
            .to_string()
    }
}

/// Removable mounts in `/proc/mounts` text; `removable(disk)` answers from sysfs.
pub fn linux_volumes(mounts: &str, removable: impl Fn(&str) -> bool) -> Vec<Volume> {
    let mut out = Vec::new();
    for line in mounts.lines() {
        let mut f = line.split_whitespace();
        let (Some(dev), Some(mnt)) = (f.next(), f.next()) else {
            continue;
        };
        if !dev.starts_with("/dev/") {
            continue;
        }
        let path = PathBuf::from(unescape_mount(mnt));
        if path == Path::new("/") || out.iter().any(|v: &Volume| v.path == path) {
            continue;
        }
        if removable(&disk_of(dev)) {
            out.push(Volume {
                name: path
                    .file_name()
                    .map_or_else(|| dev.to_string(), |n| n.to_string_lossy().into_owned()),
                path,
            });
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn linux() -> Vec<Volume> {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    linux_volumes(&mounts, |disk| {
        let sys = Path::new("/sys/block").join(disk);
        let flagged = std::fs::read_to_string(sys.join("removable")).is_ok_and(|s| s.trim() == "1");
        // USB disks often report removable = 0; their sysfs path runs through the USB bus.
        let on_usb = std::fs::canonicalize(&sys).is_ok_and(|p| {
            p.components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with("usb"))
        });
        flagged || on_usb
    })
}

// MARK: Windows

/// Lines `E:|LABEL` from the PowerShell query.
pub fn windows_volumes(output: &str) -> Vec<Volume> {
    output
        .lines()
        .filter_map(|l| {
            let (drive, label) = l.trim().split_once('|')?;
            let ok = drive.len() == 2
                && drive.as_bytes()[0].is_ascii_alphabetic()
                && drive.as_bytes()[1] == b':';
            ok.then(|| Volume {
                path: PathBuf::from(format!("{drive}\\")),
                name: if label.is_empty() {
                    drive.to_string()
                } else {
                    label.to_string()
                },
            })
        })
        .collect()
}

#[cfg(windows)]
fn windows() -> Vec<Volume> {
    let script = "Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=2' | ForEach-Object { $_.DeviceID + '|' + $_.VolumeName }";
    match Process::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
    {
        Ok(o) if o.status.success() => windows_volumes(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
}

// MARK: writing

/// A file name safe on FAT32 volumes and machine displays: no path parts, no reserved
/// characters, at most 64 characters before the extension.
pub fn safe_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control() && !"/\\:*?\"<>|".contains(*c))
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    (!cleaned.is_empty()).then(|| cleaned.chars().take(64).collect())
}

/// What happened when writing to a volume.
#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    Exists(PathBuf),
    Failed(String),
}

/// Writes `bytes` as `name.ext` in the root of `volume`.
pub fn write_to_volume(
    volume: &Path,
    name: &str,
    ext: &str,
    bytes: &[u8],
    overwrite: bool,
) -> Result<PathBuf, SendError> {
    let file = safe_name(name).ok_or_else(|| SendError::Failed("empty file name".into()))?;
    let path = volume.join(format!("{file}.{ext}"));
    if path.exists() && !overwrite {
        return Err(SendError::Exists(path));
    }
    std::fs::write(&path, bytes)
        .map_err(|e| SendError::Failed(format!("{}: {e}", path.display())))?;
    Ok(path)
}

// MARK: commands

fn spec(
    id: &'static str,
    summary: &'static str,
    effect: Effect,
    params: Vec<ParamDoc>,
    run: RunFn,
) -> CommandSpec {
    CommandSpec {
        id,
        summary,
        effect,
        params,
        run,
    }
}

const fn opt(name: &'static str, kind: &'static str, summary: &'static str) -> ParamDoc {
    ParamDoc {
        name,
        kind,
        required: false,
        summary,
    }
}

fn volumes_json(v: &[Volume]) -> Value {
    Value::Array(
        v.iter()
            .map(|v| json!({ "path": v.path.display().to_string(), "name": v.name }))
            .collect(),
    )
}

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "transfer.volumes",
            "Removable volumes (USB sticks, SD cards) mounted now",
            Effect::Read,
            vec![],
            |s, p| {
                refuse_without(s, p, Access::Read)?;
                Ok(volumes_json(&detect()))
            },
        ),
        spec(
            "transfer.send",
            "Write the design as a machine file to the root of a removable volume (Send to USB Media)",
            Effect::File,
            vec![
                opt(
                    "volume",
                    "string",
                    "Volume path from transfer.volumes (default: the only one mounted)",
                ),
                opt("format", "pes|dst|jef|...", "Machine format (default pes)"),
                opt(
                    "name",
                    "string",
                    "File name without extension (default: the design name)",
                ),
                opt(
                    "overwrite",
                    "bool",
                    "Replace a file of the same name (default false)",
                ),
            ],
            send,
        ),
    ]
}

fn refuse_without(s: &EngineState, p: &Params, access: Access) -> Result<(), CommandError> {
    if s.capabilities.allows_access(access) {
        Ok(())
    } else {
        Err(p.failed("file access is not permitted for this client"))
    }
}

fn send(s: &mut EngineState, p: &Params) -> CommandResult {
    refuse_without(s, p, Access::Write)?;
    let volumes = detect();
    let volume = if p.has("volume") {
        let want = PathBuf::from(p.string("volume", None)?);
        volumes
            .iter()
            .find(|v| v.path == want)
            .ok_or_else(|| p.not_found(format!("removable volume {}", want.display())))?
            .clone()
    } else {
        match volumes.as_slice() {
            [one] => one.clone(),
            [] => return Err(p.failed("no removable volume is mounted")),
            _ => return Err(p.invalid("volume", "several volumes are mounted; choose one")),
        }
    };
    // Sandboxed clients may only write where their roots allow.
    s.resolve(
        &volume.path.display().to_string(),
        Access::Write,
        &p.command,
    )?;
    let format = p.string("format", Some("pes"))?;
    let ty = FileType::from_extension(&format)
        .filter(|t| t.can_write())
        .ok_or_else(|| p.invalid("format", "not a writable machine format"))?;
    let name = p.string("name", Some(&s.design.name))?;
    let overwrite = p.bool("overwrite", Some(false))?;
    let pattern = Compiler::for_design(&s.design).compile(&s.design);
    if pattern.count(Command::Stitch) == 0 {
        return Err(p.failed("design has no stitches"));
    }
    let bytes = ty.write(&pattern).map_err(|e| p.failed(e.to_string()))?;
    match write_to_volume(&volume.path, &name, ty.extension(), &bytes, overwrite) {
        Ok(path) => Ok(json!({
            "path": path.display().to_string(),
            "volume": volume.name,
            "bytes": bytes.len(),
        })),
        Err(SendError::Exists(path)) => Err(p.failed(format!(
            "{} exists; send again with overwrite: true or another name",
            path.display()
        ))),
        Err(SendError::Failed(e)) => Err(p.failed(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diskutil_plists_tell_usb_sticks_from_internal_disks_and_images() {
        let stick = "<dict><key>BusProtocol</key><string>USB</string><key>Ejectable</key><true/><key>Internal</key><false/></dict>";
        let internal = "<dict><key>BusProtocol</key><string>Apple Fabric</string><key>Ejectable</key><false/><key>Internal</key><true/></dict>";
        let dmg = "<dict><key>BusProtocol</key><string>Disk Image</string><key>Ejectable</key><true/><key>Internal</key><false/></dict>";
        let sd = "<dict><key>BusProtocol</key>\n\t<string>Secure Digital</string><key>RemovableMedia</key>\n\t<true/></dict>";
        assert!(macos_is_removable(stick));
        assert!(macos_is_removable(sd));
        assert!(!macos_is_removable(internal));
        assert!(!macos_is_removable(dmg));
        assert_eq!(plist_value(stick, "BusProtocol").as_deref(), Some("USB"));
        assert_eq!(plist_value(stick, "Missing"), None);
    }

    #[test]
    fn linux_mounts_keep_removable_block_devices() {
        let mounts = "/dev/nvme0n1p2 / ext4 rw 0 0\n\
                      proc /proc proc rw 0 0\n\
                      /dev/sdb1 /media/me/MY\\040STICK vfat rw 0 0\n\
                      /dev/mmcblk0p1 /run/media/me/SD vfat rw 0 0\n\
                      /dev/sda1 /home ext4 rw 0 0\n";
        let v = linux_volumes(mounts, |d| d == "sdb" || d == "mmcblk0");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].path, PathBuf::from("/media/me/MY STICK"));
        assert_eq!(v[0].name, "MY STICK");
        assert_eq!(v[1].name, "SD");
        assert_eq!(disk_of("/dev/sdb1"), "sdb");
        assert_eq!(disk_of("/dev/nvme0n1p2"), "nvme0n1");
        assert_eq!(disk_of("/dev/mmcblk0p1"), "mmcblk0");
    }

    #[test]
    fn windows_query_lines_become_drive_roots() {
        let v = windows_volumes("E:|STICK\r\nF:|\r\nnonsense\r\nGG:|x\r\n");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].path, PathBuf::from("E:\\"));
        assert_eq!(v[1].name, "F:");
    }

    #[test]
    fn writes_plain_names_to_the_volume_root_without_overwriting() {
        let dir = std::env::temp_dir().join(format!("openstitch-usb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let p = write_to_volume(&dir, "../../etc/badge", "pes", b"x", false).expect("write");
        assert_eq!(p, dir.join("etcbadge.pes"));
        assert_eq!(
            write_to_volume(&dir, "etcbadge", "pes", b"y", false),
            Err(SendError::Exists(p.clone()))
        );
        assert!(write_to_volume(&dir, "etcbadge", "pes", b"y", true).is_ok());
        assert_eq!(std::fs::read(&p).expect("read"), b"y");
        assert!(write_to_volume(&dir, " .. ", "pes", b"x", false).is_err());
        assert_eq!(safe_name("a:b*c?").as_deref(), Some("abc"));
        assert_eq!(safe_name(&"x".repeat(100)).map(|s| s.len()), Some(64));
    }
}
