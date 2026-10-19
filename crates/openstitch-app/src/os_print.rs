//! Printing through the operating system: a PDF that OpenStitch wrote is handed to the system's
//! print dialog, or, where there is no way to reach one, opened in the system's PDF viewer to be
//! printed from there.
//!
//! Only the system's own programs are started, with the file path passed as a separate argument
//! (or, on Windows, in an environment variable), never inside a shell or script string:
//! - macOS: `osascript` asks Preview to print the file with its print dialog; the fallback is
//!   `open`, which shows it in the default PDF viewer.
//! - Windows: PowerShell `Start-Process -Verb Print`, the default PDF app's Print verb; the
//!   fallback is `Start-Process` without a verb, which opens the file.
//! - Linux and other systems: no print dialog can be reached from the command line, so
//!   `xdg-open` shows the file in the desktop's PDF viewer.
//!
//! [`plan`] builds the launches and is tested without starting anything; [`start`] runs them on
//! a background thread and [`Job::poll`] reports the outcome to the status bar.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// The systems that hand files over differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Windows,
    /// Linux and the BSDs (freedesktop `xdg-open`).
    Unix,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Unix
        }
    }
}

/// What a launch shows when it works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shows {
    PrintDialog,
    Viewer,
}

/// One program to start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub program: &'static str,
    pub args: Vec<OsString>,
    pub env: Vec<(&'static str, OsString)>,
    pub shows: Shows,
}

impl Launch {
    pub fn command(&self) -> std::process::Command {
        let mut c = std::process::Command::new(self.program);
        c.args(&self.args);
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console window flashes up behind the dialog.
            c.creation_flags(0x0800_0000);
        }
        c
    }
}

/// Environment variable that carries the path to PowerShell (read as data, never parsed).
pub const PATH_VAR: &str = "OPENSTITCH_PRINT_FILE";

/// AppleScript run by `osascript`; the file arrives as `item 1 of argv`. Preview is told to
/// print without waiting for the dialog to close.
const MAC_SCRIPT: [&str; 9] = [
    "on run argv",
    "set f to POSIX file (item 1 of argv)",
    "tell application \"Preview\"",
    "activate",
    "ignoring application responses",
    "print f with print dialog",
    "end ignoring",
    "end tell",
    "end run",
];

const WIN_PRINT: &str = "$ErrorActionPreference = 'Stop'; \
     Start-Process -FilePath $env:OPENSTITCH_PRINT_FILE -Verb Print";
const WIN_OPEN: &str = "$ErrorActionPreference = 'Stop'; \
     Start-Process -FilePath $env:OPENSTITCH_PRINT_FILE";

/// The launches to try, in order, until one starts: a print dialog first where the system has
/// one, then the PDF viewer. `pdf` is made absolute so it can never read as an option.
pub fn plan(os: Os, pdf: &Path) -> Vec<Launch> {
    let file: OsString = std::path::absolute(pdf)
        .unwrap_or_else(|_| pdf.to_path_buf())
        .into_os_string();
    match os {
        Os::MacOs => {
            let mut args = Vec::new();
            for line in MAC_SCRIPT {
                args.push("-e".into());
                args.push(line.into());
            }
            args.push(file.clone());
            vec![
                Launch {
                    program: "osascript",
                    args,
                    env: Vec::new(),
                    shows: Shows::PrintDialog,
                },
                Launch {
                    program: "open",
                    args: vec![file],
                    env: Vec::new(),
                    shows: Shows::Viewer,
                },
            ]
        }
        Os::Windows => {
            let ps = |script: &str, shows| Launch {
                program: "powershell.exe",
                args: ["-NoProfile", "-NonInteractive", "-Command", script]
                    .map(OsString::from)
                    .to_vec(),
                env: vec![(PATH_VAR, file.clone())],
                shows,
            };
            vec![
                ps(WIN_PRINT, Shows::PrintDialog),
                ps(WIN_OPEN, Shows::Viewer),
            ]
        }
        Os::Unix => vec![Launch {
            program: "xdg-open",
            args: vec![file],
            env: Vec::new(),
            shows: Shows::Viewer,
        }],
    }
}

/// How the hand-over went, for the status bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Shown(Shows, PathBuf),
    Failed(PathBuf, String),
}

impl Outcome {
    pub fn message(&self) -> String {
        match self {
            Outcome::Shown(Shows::PrintDialog, p) => {
                format!("Sent {} to the system print dialog", file_name(p))
            }
            Outcome::Shown(Shows::Viewer, p) => format!(
                "Opened {} in the PDF viewer; print it from there at 100 %",
                file_name(p)
            ),
            Outcome::Failed(p, e) => format!(
                "Could not open the print dialog or a PDF viewer ({e}); the PDF is at {}",
                p.display()
            ),
        }
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into())
}

/// Runs the launches in order and stops at the first that exits successfully.
pub fn run(pdf: &Path, launches: &[Launch]) -> Outcome {
    let mut last = String::from("nothing to start");
    for l in launches {
        match l.command().output() {
            Ok(o) if o.status.success() => return Outcome::Shown(l.shows, pdf.to_path_buf()),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                last = format!(
                    "{} {}",
                    l.program,
                    err.lines().next().unwrap_or("failed").trim()
                );
            }
            Err(e) => last = format!("{}: {e}", l.program),
        }
    }
    Outcome::Failed(pdf.to_path_buf(), last)
}

/// A hand-over running on a background thread.
pub struct Job {
    pdf: PathBuf,
    rx: mpsc::Receiver<Outcome>,
}

impl Job {
    /// The outcome once the launches are done (`None` while they run).
    pub fn poll(&self) -> Option<Outcome> {
        match self.rx.try_recv() {
            Ok(o) => Some(o),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Outcome::Failed(
                self.pdf.clone(),
                "the launch stopped".into(),
            )),
        }
    }
}

/// Hands `pdf` to this system's print dialog or viewer without blocking the window.
pub fn start(pdf: &Path) -> Job {
    let (tx, rx) = mpsc::channel();
    let file = pdf.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(run(&file, &plan(Os::current(), &file)));
    });
    Job {
        pdf: pdf.to_path_buf(),
        rx,
    }
}

/// A fresh path in the temporary folder for a PDF to print, named after `name` (letters,
/// digits, `-` and `_` only).
pub fn temp_pdf(name: &str) -> PathBuf {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let stem = if stem.trim_matches('_').is_empty() {
        "print".to_string()
    } else {
        stem
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    std::env::temp_dir()
        .join("openstitch-print")
        .join(format!("{stem}-{stamp}.pdf"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    /// A path a shell or script would misread if it were spliced into code.
    fn hostile() -> PathBuf {
        std::env::temp_dir().join("a'b\" $(rm -rf x); `y` & del z.pdf")
    }

    #[test]
    fn macos_asks_preview_for_its_print_dialog_then_falls_back_to_open() {
        let p = hostile();
        let l = plan(Os::MacOs, &p);
        assert_eq!(l.len(), 2);
        assert_eq!(
            (l[0].program, l[0].shows),
            ("osascript", Shows::PrintDialog)
        );
        // The path is the last argument on its own; no script line contains it.
        assert_eq!(l[0].args.last(), Some(&p.clone().into_os_string()));
        let script: Vec<_> = l[0].args[..l[0].args.len() - 1].to_vec();
        assert!(script.iter().step_by(2).all(|a| a == "-e"));
        assert!(
            script
                .iter()
                .all(|a| !a.to_string_lossy().contains("rm -rf"))
        );
        assert!(script.iter().any(|a| a == "print f with print dialog"));
        assert_eq!(l[1].program, "open");
        assert_eq!(l[1].args, vec![p.into_os_string()]);
        assert_eq!(l[1].shows, Shows::Viewer);
    }

    #[test]
    fn windows_uses_the_print_verb_with_the_path_in_the_environment() {
        let p = hostile();
        let l = plan(Os::Windows, &p);
        assert_eq!(l.len(), 2);
        for launch in &l {
            assert_eq!(launch.program, "powershell.exe");
            assert_eq!(launch.env, vec![(PATH_VAR, p.clone().into_os_string())]);
            assert!(
                launch
                    .args
                    .iter()
                    .all(|a| !a.to_string_lossy().contains("rm -rf"))
            );
            assert!(launch.args.iter().any(|a| a == "-NoProfile"));
        }
        assert!(
            l[0].args
                .last()
                .unwrap()
                .to_string_lossy()
                .contains("-Verb Print")
        );
        assert_eq!(l[0].shows, Shows::PrintDialog);
        assert!(
            !l[1]
                .args
                .last()
                .unwrap()
                .to_string_lossy()
                .contains("-Verb")
        );
        assert_eq!(l[1].shows, Shows::Viewer);
    }

    #[test]
    fn other_systems_open_the_pdf_viewer() {
        let p = hostile();
        let l = plan(Os::Unix, &p);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].program, "xdg-open");
        assert_eq!(l[0].args, vec![p.into_os_string()]);
        assert_eq!(l[0].shows, Shows::Viewer);
    }

    #[test]
    fn relative_paths_are_made_absolute_so_they_never_read_as_options() {
        for os in [Os::MacOs, Os::Windows, Os::Unix] {
            for l in plan(os, Path::new("-n.pdf")) {
                let file = match l.env.first() {
                    Some((_, v)) => v.clone(),
                    None => l.args.last().unwrap().clone(),
                };
                assert!(Path::new(&file).is_absolute(), "{os:?}");
            }
        }
    }

    #[test]
    fn a_launch_that_fails_falls_through_and_the_last_error_is_reported() {
        let p = Path::new("/tmp/x.pdf");
        let missing = Launch {
            program: "openstitch-no-such-program",
            args: vec![],
            env: vec![],
            shows: Shows::PrintDialog,
        };
        let out = run(p, &[missing.clone(), missing]);
        let Outcome::Failed(path, e) = &out else {
            panic!("{out:?}")
        };
        assert_eq!(path, p);
        assert!(e.contains("openstitch-no-such-program"), "{e}");
        assert!(out.message().contains("/tmp/x.pdf"));
        assert!(run(p, &[]).message().contains("nothing to start"));
    }

    #[test]
    fn outcomes_read_clearly() {
        let p = PathBuf::from("/tmp/catalogue.pdf");
        let d = Outcome::Shown(Shows::PrintDialog, p.clone()).message();
        assert!(d.contains("print dialog") && d.contains("catalogue.pdf"));
        let v = Outcome::Shown(Shows::Viewer, p).message();
        assert!(v.contains("PDF viewer") && v.contains("100 %"));
    }

    #[test]
    fn temp_pdfs_get_plain_unique_names() {
        let p = temp_pdf("../My design: 1/2");
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("___My_design__1_2-") && name.ends_with(".pdf"),
            "{name}"
        );
        assert_eq!(p.parent().unwrap().file_name().unwrap(), "openstitch-print");
        assert!(
            temp_pdf("...")
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("print-")
        );
    }
}
