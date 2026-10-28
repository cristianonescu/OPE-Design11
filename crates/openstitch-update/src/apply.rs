//! Putting a verified download in place and starting the new version
//! (`docs/research/updater.md`, "Platform mechanics").
//!
//! Three phases, so the running program is only ever touched after everything else succeeded:
//! 1. [`stage`]: unpack the asset *beside* the install (same directory, so the final rename is
//!    a same-volume rename) as `<target>.new`; nothing in use is changed.
//! 2. [`swap`]: rename the running `<target>` to `<target>.old` (allowed while it runs on every
//!    OS), rename `<target>.new` into place; on failure, rename the old one back.
//! 3. [`restart`]: start the new program and let the caller exit. The new process calls
//!    [`cleanup_previous`] at its start to remove `<target>.old`.
//!
//! The MSI install is the exception: the installer does the replacing, so `stage` keeps the
//! downloaded `.msi` and `restart` runs `msiexec` after this process has exited.

use crate::install::Install;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ApplyError {
    Io(io::Error),
    /// A damaged or unexpected archive.
    Archive(String),
    /// The archive has no program in it.
    Missing(String),
    /// This install kind cannot be replaced.
    Unsupported,
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplyError::Io(e) => write!(f, "file error: {e}"),
            ApplyError::Archive(e) => write!(f, "damaged download: {e}"),
            ApplyError::Missing(e) => write!(f, "the download has no {e}"),
            ApplyError::Unsupported => write!(f, "this install cannot update itself"),
        }
    }
}

impl std::error::Error for ApplyError {}

impl From<io::Error> for ApplyError {
    fn from(e: io::Error) -> Self {
        ApplyError::Io(e)
    }
}

impl From<zip::result::ZipError> for ApplyError {
    fn from(e: zip::result::ZipError) -> Self {
        match e {
            zip::result::ZipError::Io(e) => ApplyError::Io(e),
            other => ApplyError::Archive(other.to_string()),
        }
    }
}

/// What [`stage`] prepared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Staged {
    /// `target` (a file, or the `.app` directory) is replaced by `new`; the previous one is
    /// kept as `old` until the next start.
    Replace {
        target: PathBuf,
        new: PathBuf,
        old: PathBuf,
        bundle: bool,
    },
    /// Windows Installer replaces the files; `exe` is started afterwards.
    Msi { msi: PathBuf, exe: PathBuf },
}

/// The process [`restart`] starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub program: OsString,
    pub args: Vec<OsString>,
    /// Windows only: `args` holds one pre-quoted command line for `cmd /c`.
    pub raw: bool,
}

/// Where downloads go before they are staged.
pub fn download_dir() -> PathBuf {
    std::env::temp_dir().join("openstitch-update")
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut name = p.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(suffix);
    p.with_file_name(name)
}

/// Unpacks `downloaded` (the asset for `install`) beside the install.
pub fn stage(install: &Install, downloaded: &Path) -> Result<Staged, ApplyError> {
    match install {
        Install::WindowsPortable { exe } => {
            let dir = exe.parent().ok_or(ApplyError::Unsupported)?;
            let new = with_suffix(exe, ".new");
            unzip_flat(downloaded, dir, "openstitch.exe", &new)?;
            Ok(replace(exe, new, false))
        }
        Install::WindowsMsi { exe } => Ok(Staged::Msi {
            msi: downloaded.to_path_buf(),
            exe: exe.clone(),
        }),
        Install::MacBundle { app } => {
            let parent = app.parent().ok_or(ApplyError::Unsupported)?;
            let new = with_suffix(app, ".new");
            let scratch = parent.join(format!(".openstitch-update-{}", std::process::id()));
            let _ = fs::remove_dir_all(&scratch);
            let _ = fs::remove_dir_all(&new);
            let result = (|| {
                fs::create_dir_all(&scratch)?;
                let file = fs::File::open(downloaded)?;
                let mut archive = zip::ZipArchive::new(file)?;
                archive.extract(&scratch)?;
                let unpacked = scratch.join("OpenStitch.app");
                if !unpacked
                    .join("Contents")
                    .join("MacOS")
                    .join("openstitch")
                    .is_file()
                {
                    return Err(ApplyError::Missing("OpenStitch.app".into()));
                }
                fs::rename(&unpacked, &new)?;
                Ok(())
            })();
            let _ = fs::remove_dir_all(&scratch);
            result?;
            Ok(replace(app, new, true))
        }
        Install::AppImage { image } => {
            let new = with_suffix(image, ".new");
            fs::copy(downloaded, &new)?;
            set_executable(&new)?;
            Ok(replace(image, new, false))
        }
        Install::LinuxBinary { exe } => {
            let new = with_suffix(exe, ".new");
            untar_one(downloaded, &["bin", "openstitch"], &new)?;
            set_executable(&new)?;
            Ok(replace(exe, new, false))
        }
        Install::Flatpak | Install::Development | Install::Unsupported(_) => {
            Err(ApplyError::Unsupported)
        }
    }
}

fn replace(target: &Path, new: PathBuf, bundle: bool) -> Staged {
    Staged::Replace {
        target: target.to_path_buf(),
        old: with_suffix(target, ".old"),
        new,
        bundle,
    }
}

/// Puts the staged program in place of the running one.
pub fn swap(staged: &Staged) -> Result<(), ApplyError> {
    let Staged::Replace {
        target,
        new,
        old,
        bundle,
    } = staged
    else {
        return Ok(());
    };
    if *bundle {
        let _ = fs::remove_dir_all(old);
    } else {
        let _ = fs::remove_file(old);
    }
    fs::rename(target, old)?;
    if let Err(e) = fs::rename(new, target) {
        let _ = fs::rename(old, target);
        return Err(e.into());
    }
    Ok(())
}

/// The command that starts the new version with `args` (the original command line).
pub fn launch(staged: &Staged, args: &[OsString]) -> Launch {
    match staged {
        Staged::Replace {
            target,
            bundle: true,
            ..
        } => {
            // Launch Services starts the new bundle even while the old process is still exiting.
            let mut a: Vec<OsString> = vec!["-n".into(), "-a".into(), target.clone().into()];
            if !args.is_empty() {
                a.push("--args".into());
                a.extend(args.iter().cloned());
            }
            Launch {
                program: "open".into(),
                args: a,
                raw: false,
            }
        }
        Staged::Replace { target, .. } => Launch {
            program: target.clone().into(),
            args: args.to_vec(),
            raw: false,
        },
        Staged::Msi { msi, exe } => Launch {
            program: "cmd".into(),
            args: vec![msi_command_line(msi, exe).into()],
            raw: true,
        },
    }
}

/// `cmd /c` line: wait about three seconds for this process to exit (so the installer finds
/// no file in use), run the installer with its progress bar only, then start the installed
/// editor. `msiexec` asks for elevation itself (the MSI installs per machine).
fn msi_command_line(msi: &Path, exe: &Path) -> String {
    format!(
        "/c ping -n 4 127.0.0.1 >nul & start \"\" /wait msiexec /i \"{}\" /passive /norestart & start \"\" \"{}\"",
        msi.display(),
        exe.display()
    )
}

/// Starts `launch` detached, with `env` added to its environment, and returns at once; the
/// caller then exits.
pub fn restart(launch: &Launch, env: &[(&str, &str)]) -> io::Result<()> {
    let mut cmd = std::process::Command::new(&launch.program);
    if launch.raw {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            for a in &launch.args {
                cmd.raw_arg(a);
            }
            // No console window for cmd.
            cmd.creation_flags(0x0800_0000);
        }
        #[cfg(not(windows))]
        cmd.args(&launch.args);
    } else {
        cmd.args(&launch.args);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn().map(drop)
}

/// Removes what the previous version left behind (`<target>.old`). Called at every start; a
/// file still held by the exiting old process is simply tried again next time.
pub fn cleanup_previous(install: &Install) {
    let (target, bundle) = match install {
        Install::WindowsPortable { exe } | Install::WindowsMsi { exe } => (exe, false),
        Install::MacBundle { app } => (app, true),
        Install::AppImage { image } => (image, false),
        Install::LinuxBinary { exe } => (exe, false),
        Install::Flatpak | Install::Development | Install::Unsupported(_) => return,
    };
    let old = with_suffix(target, ".old");
    if bundle {
        let _ = fs::remove_dir_all(&old);
    } else {
        let _ = fs::remove_file(&old);
    }
    let new = with_suffix(target, ".new");
    if bundle {
        let _ = fs::remove_dir_all(&new);
    } else {
        let _ = fs::remove_file(&new);
    }
    let _ = fs::remove_dir_all(download_dir());
}

/// Extracts the regular files of a zip whose entries all sit in one top-level folder into
/// `dir`, writing the entry named `program` to `program_dest` instead and everything else under
/// its own name (the README, notices and licences the portable zip ships).
fn unzip_flat(
    archive_path: &Path,
    dir: &Path,
    program: &str,
    program_dest: &Path,
) -> Result<(), ApplyError> {
    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        // Only files directly inside the top folder (or at the root) are ours.
        let mut parts = path.components();
        let (first, second) = (parts.next(), parts.next());
        let name = match (first, second, parts.next()) {
            (Some(a), None, None) => a.as_os_str().to_os_string(),
            (Some(_), Some(b), None) => b.as_os_str().to_os_string(),
            _ => continue,
        };
        let dest = if name == program {
            found = true;
            program_dest.to_path_buf()
        } else {
            dir.join(&name)
        };
        let mut out = fs::File::create(&dest)?;
        io::copy(&mut entry, &mut out)?;
    }
    if !found {
        let _ = fs::remove_file(program_dest);
        return Err(ApplyError::Missing(program.into()));
    }
    Ok(())
}

/// Extracts the one entry of a `.tar.gz` whose path ends in `tail` (after the top folder) to
/// `dest`.
fn untar_one(archive_path: &Path, tail: &[&str], dest: &Path) -> Result<(), ApplyError> {
    let file = fs::File::open(archive_path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let matches = parts.len() == tail.len() + 1 && parts[1..] == *tail;
        if !matches || !entry.header().entry_type().is_file() {
            continue;
        }
        let mut out = fs::File::create(dest)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        io::Write::write_all(&mut out, &buf)?;
        return Ok(());
    }
    Err(ApplyError::Missing(tail.join("/")))
}

fn set_executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openstitch-apply-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("zip");
        let mut w = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        for (name, body) in entries {
            if name.ends_with('/') {
                w.add_directory(*name, opts).expect("dir");
            } else {
                w.start_file(*name, opts).expect("start");
                w.write_all(body).expect("write");
            }
        }
        w.finish().expect("finish");
    }

    fn make_tgz(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("tgz");
        let gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut b = tar::Builder::new(gz);
        for (name, body) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            b.append_data(&mut h, name, *body).expect("append");
        }
        b.into_inner().expect("finish").finish().expect("gz");
    }

    fn read(p: &Path) -> String {
        fs::read_to_string(p).expect("read")
    }

    #[test]
    fn portable_zip_is_staged_swapped_and_cleaned_up() {
        let dir = temp("portable");
        let exe = dir.join("openstitch.exe");
        fs::write(&exe, "old exe").expect("exe");
        fs::write(dir.join("README.md"), "old readme").expect("readme");
        let zip = dir.join("dl.zip");
        make_zip(
            &zip,
            &[
                ("OpenStitch-v0.1.0-alpha.4-windows-x86_64/", b""),
                (
                    "OpenStitch-v0.1.0-alpha.4-windows-x86_64/openstitch.exe",
                    b"new exe",
                ),
                (
                    "OpenStitch-v0.1.0-alpha.4-windows-x86_64/README.md",
                    b"new readme",
                ),
                (
                    "OpenStitch-v0.1.0-alpha.4-windows-x86_64/deeper/x.txt",
                    b"ignored",
                ),
            ],
        );
        let install = Install::WindowsPortable { exe: exe.clone() };
        let staged = stage(&install, &zip).expect("stage");
        assert_eq!(
            staged,
            Staged::Replace {
                target: exe.clone(),
                new: dir.join("openstitch.exe.new"),
                old: dir.join("openstitch.exe.old"),
                bundle: false,
            }
        );
        assert_eq!(
            read(&exe),
            "old exe",
            "staging never touches the running program"
        );
        assert_eq!(read(&dir.join("openstitch.exe.new")), "new exe");
        assert_eq!(read(&dir.join("README.md")), "new readme");
        assert!(!dir.join("deeper").exists() && !dir.join("x.txt").exists());
        swap(&staged).expect("swap");
        assert_eq!(read(&exe), "new exe");
        assert_eq!(read(&dir.join("openstitch.exe.old")), "old exe");
        assert!(!dir.join("openstitch.exe.new").exists());
        let l = launch(&staged, &["design.osd".into()]);
        assert_eq!(l.program, exe.as_os_str());
        assert_eq!(l.args, [OsString::from("design.osd")]);
        assert!(!l.raw);
        cleanup_previous(&install);
        assert!(!dir.join("openstitch.exe.old").exists());
        assert_eq!(read(&exe), "new exe");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn zip_without_the_program_is_refused_and_leaves_nothing() {
        let dir = temp("noexe");
        let exe = dir.join("openstitch.exe");
        fs::write(&exe, "old").expect("exe");
        let zip = dir.join("dl.zip");
        make_zip(&zip, &[("top/README.md", b"r"), ("../evil.exe", b"x")]);
        let e = stage(&Install::WindowsPortable { exe: exe.clone() }, &zip).expect_err("missing");
        assert!(matches!(e, ApplyError::Missing(_)), "{e}");
        assert!(!dir.join("openstitch.exe.new").exists());
        assert!(!dir.parent().expect("parent").join("evil.exe").exists());
        assert_eq!(read(&exe), "old");
        let bad = dir.join("bad.zip");
        fs::write(&bad, "not a zip").expect("bad");
        let e = stage(&Install::WindowsPortable { exe }, &bad).expect_err("damaged");
        assert!(matches!(e, ApplyError::Archive(_)), "{e}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mac_bundle_is_swapped_as_a_whole() {
        let dir = temp("mac");
        let app = dir.join("OpenStitch.app");
        let bin = app.join("Contents").join("MacOS");
        fs::create_dir_all(&bin).expect("bundle");
        fs::write(bin.join("openstitch"), "old").expect("bin");
        let zip = dir.join("dl.zip");
        make_zip(
            &zip,
            &[
                ("OpenStitch.app/", b""),
                ("OpenStitch.app/Contents/", b""),
                ("OpenStitch.app/Contents/Info.plist", b"<plist/>"),
                ("OpenStitch.app/Contents/MacOS/openstitch", b"new"),
            ],
        );
        let install = Install::MacBundle { app: app.clone() };
        let staged = stage(&install, &zip).expect("stage");
        assert!(matches!(&staged, Staged::Replace { bundle: true, .. }));
        assert_eq!(read(&bin.join("openstitch")), "old");
        assert_eq!(
            read(&dir.join("OpenStitch.app.new/Contents/MacOS/openstitch")),
            "new"
        );
        assert!(
            !dir.join(format!(".openstitch-update-{}", std::process::id()))
                .exists()
        );
        swap(&staged).expect("swap");
        assert_eq!(read(&bin.join("openstitch")), "new");
        assert_eq!(read(&app.join("Contents/Info.plist")), "<plist/>");
        assert_eq!(
            read(&dir.join("OpenStitch.app.old/Contents/MacOS/openstitch")),
            "old"
        );
        let l = launch(&staged, &[]);
        assert_eq!(l.program, "open");
        assert_eq!(
            l.args,
            [OsString::from("-n"), "-a".into(), app.clone().into()]
        );
        let l = launch(&staged, &["a.osd".into()]);
        assert_eq!(l.args.len(), 5);
        assert_eq!(l.args[3], "--args");
        cleanup_previous(&install);
        assert!(!dir.join("OpenStitch.app.old").exists());
        let zip2 = dir.join("empty.zip");
        make_zip(&zip2, &[("Other.app/Contents/MacOS/x", b"x")]);
        let e = stage(&install, &zip2).expect_err("no bundle");
        assert!(matches!(e, ApplyError::Missing(_)), "{e}");
        assert!(!dir.join("OpenStitch.app.new").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn appimage_and_tarball_replace_the_file() {
        let dir = temp("linux");
        let image = dir.join("OpenStitch.AppImage");
        fs::write(&image, "old image").expect("image");
        let dl = dir.join("dl.AppImage");
        fs::write(&dl, "new image").expect("dl");
        let install = Install::AppImage {
            image: image.clone(),
        };
        let staged = stage(&install, &dl).expect("stage");
        assert_eq!(read(&dir.join("OpenStitch.AppImage.new")), "new image");
        swap(&staged).expect("swap");
        assert_eq!(read(&image), "new image");
        assert_eq!(read(&dir.join("OpenStitch.AppImage.old")), "old image");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&image).expect("meta").permissions().mode() & 0o777,
                0o755
            );
        }
        cleanup_previous(&install);
        assert!(!dir.join("OpenStitch.AppImage.old").exists());

        let bin = dir.join("bin");
        fs::create_dir_all(&bin).expect("bin");
        let exe = bin.join("openstitch");
        fs::write(&exe, "old bin").expect("exe");
        let tgz = dir.join("dl.tar.gz");
        make_tgz(
            &tgz,
            &[
                ("openstitch-0.1.0-linux-x86_64/README.md", b"r"),
                (
                    "openstitch-0.1.0-linux-x86_64/bin/openstitch-thumbnailer",
                    b"thumb",
                ),
                ("openstitch-0.1.0-linux-x86_64/bin/openstitch", b"new bin"),
            ],
        );
        let install = Install::LinuxBinary { exe: exe.clone() };
        let staged = stage(&install, &tgz).expect("stage");
        assert_eq!(read(&bin.join("openstitch.new")), "new bin");
        swap(&staged).expect("swap");
        assert_eq!(read(&exe), "new bin");
        assert_eq!(read(&bin.join("openstitch.old")), "old bin");
        assert!(!bin.join("openstitch-thumbnailer").exists());
        let empty = dir.join("empty.tar.gz");
        make_tgz(&empty, &[("x/README.md", b"r")]);
        let e = stage(&install, &empty).expect_err("no binary");
        assert!(matches!(e, ApplyError::Missing(_)), "{e}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn msi_runs_the_installer_after_this_process_and_relaunches() {
        let msi = PathBuf::from(
            r"C:\Users\me\AppData\Local\Temp\openstitch-update\OpenStitch-v0.1.0-alpha.4-windows-x86_64.msi",
        );
        let exe = PathBuf::from(r"C:\Program Files\OpenStitch\openstitch.exe");
        let staged = stage(&Install::WindowsMsi { exe: exe.clone() }, &msi).expect("stage");
        assert_eq!(
            staged,
            Staged::Msi {
                msi: msi.clone(),
                exe: exe.clone()
            }
        );
        swap(&staged).expect("nothing to swap");
        let l = launch(&staged, &["ignored.osd".into()]);
        assert_eq!(l.program, "cmd");
        assert!(l.raw);
        let line = l.args[0].to_string_lossy().into_owned();
        assert!(line.starts_with("/c ping -n 4 127.0.0.1 >nul & start \"\" /wait msiexec /i \""));
        assert!(line.contains(&format!("\"{}\" /passive /norestart", msi.display())));
        assert!(line.ends_with(&format!("& start \"\" \"{}\"", exe.display())));
    }

    #[test]
    fn kinds_that_cannot_update_are_refused() {
        for i in [
            Install::Flatpak,
            Install::Development,
            Install::Unsupported("x".into()),
        ] {
            assert!(matches!(
                stage(&i, Path::new("x")),
                Err(ApplyError::Unsupported)
            ));
            cleanup_previous(&i);
        }
    }
}
