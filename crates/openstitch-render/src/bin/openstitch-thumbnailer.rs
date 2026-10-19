//! `openstitch-thumbnailer`: writes a PNG thumbnail of an OpenStitch project or a machine
//! embroidery file, in the freedesktop thumbnailer calling convention.
//!
//! ```text
//! openstitch-thumbnailer [-s SIZE] INPUT OUTPUT.png
//! ```
//! Installed with `share/thumbnailers/openstitch.thumbnailer`
//! (`Exec=openstitch-thumbnailer -s %s %i %o`), so GNOME, KDE and other freedesktop file
//! managers show stitched previews of `.osd` and `.pes` files.

use openstitch_render::thumbnail;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: openstitch-thumbnailer [-s SIZE] INPUT OUTPUT.png";

fn parse(args: &[String]) -> Result<(u32, PathBuf, PathBuf), String> {
    let mut size = 256;
    let mut files = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-s" | "--size" => {
                size = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|n| (1..=4096).contains(n))
                    .ok_or("-s takes a size in pixels")?;
            }
            "-h" | "--help" => return Err(String::new()),
            _ => files.push(PathBuf::from(a)),
        }
    }
    match <[PathBuf; 2]>::try_from(files) {
        Ok([input, output]) => Ok((size, input, output)),
        Err(_) => Err("expected INPUT and OUTPUT".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (size, input, output) = match parse(&args) {
        Ok(v) => v,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("openstitch-thumbnailer: {e}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = thumbnail::load(&input)
        .and_then(|p| thumbnail::png(&p, size))
        .and_then(|png| {
            std::fs::write(&output, png).map_err(|e| format!("{}: {e}", output.display()))
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("openstitch-thumbnailer: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parses_freedesktop_arguments() {
        let (size, i, o) = parse(&args("-s 128 in.pes out.png")).expect("ok");
        assert_eq!(
            (size, i.to_str(), o.to_str()),
            (128, Some("in.pes"), Some("out.png"))
        );
        assert_eq!(parse(&args("a.osd b.png")).expect("ok").0, 256);
        assert!(parse(&args("-s 0 a b")).is_err());
        assert!(parse(&args("-s x a b")).is_err());
        assert!(parse(&args("only-one")).is_err());
        assert!(parse(&args("a b c")).is_err());
    }
}
