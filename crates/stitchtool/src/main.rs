//! `stitchtool`: run OpenStitch commands from scripts or a JSON-lines control channel.
//!
//! ```text
//! stitchtool commands              # command catalogue (JSON)
//! stitchtool exec script.json      # JSON array of {"command", "params"}; stops at first failure
//! stitchtool serve [--read-only] [--no-files]
//!                                  # one JSON request per stdin line, one reply per stdout line;
//!                                  # --read-only refuses edits, undo/redo and file writes,
//!                                  # --no-files refuses every file read and write
//! stitchtool connect [--endpoint FILE]
//!                                  # same JSON lines, sent to the running editor (started with
//!                                  # `openstitch --listen PORT`); FILE defaults to automation.json
//!                                  # in the OpenStitch config directory
//! ```

use openstitch_engine::{Capabilities, Session};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

const MAX_LINE: usize = 1 << 20;
const USAGE: &str = "usage: stitchtool commands | exec <script.json> | serve [--read-only] [--no-files] | connect [--endpoint FILE]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["commands"] => {
            let mut s = Session::new(cwd);
            match s.execute("commands.list", None) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
                Err(e) => return fail(&e.to_string()),
            }
        }
        ["exec", script] => return exec(Path::new(script)),
        ["serve", flags @ ..] => {
            let mut caps = Capabilities::FULL;
            for f in flags {
                caps = match *f {
                    "--read-only" => caps.read_only(),
                    "--no-files" => caps.without_files(),
                    _ => return fail(USAGE),
                };
            }
            let mut session = Session::new(cwd);
            session.set_capabilities(caps);
            serve(session)
        }
        ["connect"] => return connect(None),
        ["connect", "--endpoint", file] => return connect(Some(Path::new(file))),
        _ => return fail(USAGE),
    }
    ExitCode::SUCCESS
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("{msg}");
    ExitCode::FAILURE
}

fn exec(script: &Path) -> ExitCode {
    let text = match std::fs::read_to_string(script) {
        Ok(t) => t,
        Err(e) => return fail(&format!("cannot read {}: {e}", script.display())),
    };
    let Ok(Value::Array(steps)) = serde_json::from_str::<Value>(&text) else {
        return fail("script must be a JSON array of requests");
    };
    let base = script
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut session = Session::new(base);
    for step in &steps {
        let reply = session.handle(step);
        println!("{reply}");
        if reply["ok"] != json!(true) {
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

/// Forwards stdin request lines to the running editor and prints its replies.
fn connect(endpoint: Option<&Path>) -> ExitCode {
    let Some(file) = endpoint
        .map(Path::to_path_buf)
        .or_else(openstitch_remote::default_endpoint_file)
    else {
        return fail("no OpenStitch config directory; pass --endpoint FILE");
    };
    let mut client = match openstitch_remote::Client::from_file(&file) {
        Ok(c) => c,
        Err(e) => {
            return fail(&format!(
                "cannot read {} ({e}); is the editor running with --listen?",
                file.display()
            ));
        }
    };
    lines(|req| client.handle(req));
    ExitCode::SUCCESS
}

fn serve(mut session: Session) {
    lines(|req| session.handle(req));
}

/// One JSON request per stdin line, one reply per stdout line.
fn lines(mut handle: impl FnMut(&Value) -> Value) {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(req) if line.len() <= MAX_LINE => handle(&req),
            _ => json!({ "ok": false, "error": "invalid JSON request" }),
        };
        if writeln!(out, "{reply}").and_then(|()| out.flush()).is_err() {
            break;
        }
    }
}
