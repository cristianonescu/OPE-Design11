//! `openstitch-mcp`: MCP server on stdio. One JSON-RPC message per line in, one per line out;
//! logs go to stderr.

use openstitch_mcp::{Config, Line, LineReader, Server};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str =
    "usage: openstitch-mcp [--root DIR]... [--read-root DIR]... [--write-root DIR]...
                      [--read-only] [--no-files]
                      [--max-request-bytes N] [--max-response-bytes N]
  --root DIR         commands may read and write files under DIR (repeatable)
  --read-root DIR    commands may only read files under DIR (repeatable)
  --write-root DIR   same as --root
  --read-only        inspect and preview only: no edits, undo/redo or file writes
  --no-files         no file access at all (cannot be combined with roots)
With no roots, every file command is refused. Relative paths resolve against the first
write root, else the first read root.";

fn log(message: &str) {
    eprintln!("openstitch-mcp: {message}");
}

fn dir(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Option<Config>, String> {
    let mut config = Config::default();
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--root" | "--write-root" => config.write_roots.push(dir(&value()?)),
            "--read-root" => config.read_roots.push(dir(&value()?)),
            "--max-request-bytes" | "--max-response-bytes" => {
                let n: usize = value()?
                    .parse()
                    .ok()
                    .filter(|n| *n >= 1024)
                    .ok_or_else(|| format!("{flag} must be an integer >= 1024"))?;
                if flag == "--max-request-bytes" {
                    config.max_request_bytes = n;
                } else {
                    config.max_response_bytes = n;
                }
            }
            "--read-only" => config.capabilities = config.capabilities.read_only(),
            "--no-files" => config.capabilities = config.capabilities.without_files(),
            "-h" | "--help" => return Ok(None),
            _ => return Err(format!("unknown argument {flag}")),
        }
    }
    let c = config.capabilities;
    if !c.read_files
        && !c.write_files
        && !(config.read_roots.is_empty() && config.write_roots.is_empty())
    {
        return Err("--no-files cannot be combined with --root/--read-root/--write-root".into());
    }
    Ok(Some(config))
}

fn main() -> ExitCode {
    let config = match parse(std::env::args().skip(1)) {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            log(&format!("{e}\n{USAGE}"));
            return ExitCode::from(64);
        }
    };
    let max_request_bytes = config.max_request_bytes;
    let mut server = match Server::new(config) {
        Ok(server) => server,
        Err(e) => {
            log(&e.to_string());
            return ExitCode::from(66);
        }
    };
    if let Some(sandbox) = server.sandbox() {
        log(&format!(
            "ready; write roots: {:?}, read roots: {:?}, capabilities: {:?}",
            sandbox.write_roots(),
            sandbox.read_roots(),
            server.config().capabilities.names()
        ));
    }

    let mut stdout = std::io::stdout().lock();
    for line in LineReader::new(std::io::stdin().lock(), max_request_bytes) {
        let reply = match line {
            Line::Text(text) => server.handle_line(&text),
            Line::Oversized => Some(server.oversized_request_reply()),
        };
        if let Some(reply) = reply {
            if writeln!(stdout, "{reply}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                break; // client went away
            }
        }
    }
    ExitCode::SUCCESS
}
