//! How this copy of the editor is installed, which decides the asset to download and how it is
//! swapped in (`docs/research/updater.md`, "Platform mechanics").

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
    Other,
}

impl Os {
    pub const fn this() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            Os::Other
        }
    }
}

/// The install kind of the running editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Install {
    /// `openstitch.exe` unzipped anywhere outside Program Files.
    WindowsPortable { exe: PathBuf },
    /// Installed by the MSI under `%ProgramFiles%\OpenStitch`.
    WindowsMsi { exe: PathBuf },
    /// `OpenStitch.app` (the bundle directory).
    MacBundle { app: PathBuf },
    /// Running from an AppImage; `image` is `$APPIMAGE`.
    AppImage { image: PathBuf },
    /// The tarball's `bin/openstitch`.
    LinuxBinary { exe: PathBuf },
    /// Inside a Flatpak sandbox: updates come from the Flatpak remote.
    Flatpak,
    /// A debug build or a binary under a cargo `target/` directory: never checks.
    Development,
    /// Something this crate has no asset for (another OS or architecture, a macOS binary
    /// outside a bundle): the dialog can show notes, but not apply.
    Unsupported(String),
}

/// What [`classify`] looks at; [`detect`] fills it from the process.
#[derive(Clone, Debug, Default)]
pub struct Context {
    pub os: Option<Os>,
    pub arch: String,
    pub exe: Option<PathBuf>,
    /// `$APPIMAGE` (set by the AppImage runtime).
    pub appimage: Option<PathBuf>,
    /// `/.flatpak-info` exists.
    pub flatpak: bool,
    /// `%ProgramFiles%` and `%ProgramW6432%`.
    pub program_files: Vec<PathBuf>,
    pub debug: bool,
}

impl Install {
    /// The release asset for this install at `tag`, or `None` when nothing can be applied.
    pub fn asset_name(&self, tag: &str) -> Option<String> {
        let suffix = match self {
            Install::WindowsPortable { .. } => "windows-x86_64.zip",
            Install::WindowsMsi { .. } => "windows-x86_64.msi",
            Install::MacBundle { .. } => "macos-universal.zip",
            Install::AppImage { .. } => "linux-x86_64.AppImage",
            Install::LinuxBinary { .. } => "linux-x86_64.tar.gz",
            Install::Flatpak | Install::Development | Install::Unsupported(_) => return None,
        };
        Some(format!("OpenStitch-{tag}-{suffix}"))
    }

    /// Whether an update check makes sense at all for this install.
    pub fn checks(&self) -> bool {
        !matches!(self, Install::Flatpak | Install::Development)
    }

    /// Whether an update can be downloaded and applied (as opposed to only shown).
    pub fn can_apply(&self) -> bool {
        !matches!(
            self,
            Install::Flatpak | Install::Development | Install::Unsupported(_)
        )
    }
}

/// The install kind of this process.
pub fn detect() -> Install {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    classify(&Context {
        os: Some(Os::this()),
        arch: std::env::consts::ARCH.to_string(),
        exe: std::env::current_exe().ok(),
        appimage: env("APPIMAGE"),
        flatpak: Path::new("/.flatpak-info").exists(),
        program_files: ["ProgramFiles", "ProgramW6432"]
            .iter()
            .filter_map(|k| env(k))
            .collect(),
        debug: cfg!(debug_assertions),
    })
}

/// Pure classification, so every branch is testable on every OS.
pub fn classify(c: &Context) -> Install {
    if c.debug {
        return Install::Development;
    }
    let Some(exe) = c.exe.as_deref() else {
        return Install::Unsupported("the executable's path is unknown".into());
    };
    if exe.components().any(|p| p.as_os_str() == "target") {
        return Install::Development;
    }
    if c.flatpak {
        return Install::Flatpak;
    }
    match c.os {
        Some(Os::Windows) => {
            if c.arch != "x86_64" {
                return Install::Unsupported(format!("Windows on {}", c.arch));
            }
            let under_program_files = c.program_files.iter().any(|pf| {
                let pf = normalise_windows(&pf.join("OpenStitch")) + "/";
                normalise_windows(exe).starts_with(&pf)
            });
            if under_program_files {
                Install::WindowsMsi { exe: exe.into() }
            } else {
                Install::WindowsPortable { exe: exe.into() }
            }
        }
        Some(Os::MacOs) => match bundle_of(exe) {
            Some(app) => Install::MacBundle { app },
            None => Install::Unsupported("macOS binary outside an .app bundle".into()),
        },
        Some(Os::Linux) => {
            if c.arch != "x86_64" {
                return Install::Unsupported(format!("Linux on {}", c.arch));
            }
            match &c.appimage {
                Some(image) => Install::AppImage {
                    image: image.clone(),
                },
                None => Install::LinuxBinary { exe: exe.into() },
            }
        }
        Some(Os::Other) | None => Install::Unsupported(std::env::consts::OS.to_string()),
    }
}

/// Lower-cased with `/` as the separator, so `C:\Program Files` and `c:/program files` compare
/// equal; the result is only ever compared, never used as a path.
fn normalise_windows(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// `…/OpenStitch.app` for `…/OpenStitch.app/Contents/MacOS/openstitch`.
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app")
        .then(|| app.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(os: Os, exe: &str) -> Context {
        Context {
            os: Some(os),
            arch: "x86_64".into(),
            exe: Some(PathBuf::from(exe)),
            program_files: vec![
                PathBuf::from(r"C:\Program Files"),
                PathBuf::from(r"C:\Program Files"),
            ],
            ..Context::default()
        }
    }

    #[test]
    fn windows_portable_or_msi() {
        let c = ctx(Os::Windows, r"D:\Tools\OpenStitch\openstitch.exe");
        assert_eq!(
            classify(&c),
            Install::WindowsPortable {
                exe: c.exe.clone().expect("exe")
            }
        );
        let msi = ctx(Os::Windows, r"C:\PROGRAM FILES\OpenStitch\openstitch.exe");
        assert!(matches!(classify(&msi), Install::WindowsMsi { .. }));
        let other = ctx(
            Os::Windows,
            r"C:\Program Files\OpenStitchFork\openstitch.exe",
        );
        assert!(matches!(classify(&other), Install::WindowsPortable { .. }));
        let mut arm = ctx(Os::Windows, r"D:\x\openstitch.exe");
        arm.arch = "aarch64".into();
        assert!(matches!(classify(&arm), Install::Unsupported(_)));
    }

    #[test]
    fn mac_bundle_only() {
        let c = ctx(
            Os::MacOs,
            "/Applications/OpenStitch.app/Contents/MacOS/openstitch",
        );
        assert_eq!(
            classify(&c),
            Install::MacBundle {
                app: PathBuf::from("/Applications/OpenStitch.app")
            }
        );
        let mut arm = c.clone();
        arm.arch = "aarch64".into();
        assert!(
            matches!(classify(&arm), Install::MacBundle { .. }),
            "universal binary"
        );
        let loose = ctx(Os::MacOs, "/usr/local/bin/openstitch");
        assert!(matches!(classify(&loose), Install::Unsupported(_)));
    }

    #[test]
    fn linux_appimage_tarball_flatpak() {
        let mut c = ctx(Os::Linux, "/tmp/.mount_OpenStXYZ/usr/bin/openstitch");
        c.appimage = Some("/home/me/Apps/OpenStitch.AppImage".into());
        assert_eq!(
            classify(&c),
            Install::AppImage {
                image: PathBuf::from("/home/me/Apps/OpenStitch.AppImage")
            }
        );
        let t = ctx(Os::Linux, "/home/me/.local/bin/openstitch");
        assert!(matches!(classify(&t), Install::LinuxBinary { .. }));
        let mut f = t.clone();
        f.flatpak = true;
        assert_eq!(classify(&f), Install::Flatpak);
        assert!(!Install::Flatpak.checks());
    }

    #[test]
    fn development_builds_never_check() {
        let mut c = ctx(
            Os::Windows,
            r"D:\src\OpenStitch\target\release\openstitch.exe",
        );
        assert_eq!(classify(&c), Install::Development);
        c.exe = Some(r"D:\x\openstitch.exe".into());
        c.debug = true;
        assert_eq!(classify(&c), Install::Development);
        assert!(!Install::Development.checks() && !Install::Development.can_apply());
        let none = Context::default();
        assert!(matches!(classify(&none), Install::Unsupported(_)));
    }

    #[test]
    fn asset_names_match_the_release_pipeline() {
        let t = "v0.1.0-alpha.4";
        let p = |i: Install| i.asset_name(t);
        assert_eq!(
            p(Install::WindowsPortable { exe: "x".into() }).as_deref(),
            Some("OpenStitch-v0.1.0-alpha.4-windows-x86_64.zip")
        );
        assert_eq!(
            p(Install::WindowsMsi { exe: "x".into() }).as_deref(),
            Some("OpenStitch-v0.1.0-alpha.4-windows-x86_64.msi")
        );
        assert_eq!(
            p(Install::MacBundle { app: "x".into() }).as_deref(),
            Some("OpenStitch-v0.1.0-alpha.4-macos-universal.zip")
        );
        assert_eq!(
            p(Install::AppImage { image: "x".into() }).as_deref(),
            Some("OpenStitch-v0.1.0-alpha.4-linux-x86_64.AppImage")
        );
        assert_eq!(
            p(Install::LinuxBinary { exe: "x".into() }).as_deref(),
            Some("OpenStitch-v0.1.0-alpha.4-linux-x86_64.tar.gz")
        );
        assert_eq!(p(Install::Flatpak), None);
        assert_eq!(p(Install::Unsupported("x".into())), None);
        assert!(!Install::Unsupported("x".into()).can_apply());
        assert!(
            Install::Unsupported("x".into()).checks(),
            "notes still show"
        );
    }
}
