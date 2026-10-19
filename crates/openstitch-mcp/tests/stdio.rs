//! End to end over the built binary's stdin/stdout, as an MCP client launches it.

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn binary_speaks_json_rpc_over_stdio() {
    let root = std::env::temp_dir().join(format!("openstitch-mcp-stdio-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("root");
    let mut child = Command::new(env!("CARGO_BIN_EXE_openstitch-mcp"))
        .args([
            "--root",
            &root.display().to_string(),
            "--max-request-bytes",
            "4096",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "shape_rectangle", "arguments": {"x": 0, "y": 0, "width": 20, "height": 20}}}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "design_export", "arguments": {"path": "a.pes"}}}),
    ];
    let mut input: String = requests.iter().map(|r| format!("{r}\n")).collect();
    input.push_str(&format!("{}\n", "x".repeat(10_000)));
    input.push_str(r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#);
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    let replies: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("JSON line"))
        .collect();
    assert_eq!(replies.len(), 5, "{replies:?}");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(replies[1]["result"]["isError"], false);
    assert_eq!(replies[2]["result"]["isError"], false, "{}", replies[2]);
    assert!(root.join("a.pes").exists());
    assert_eq!(replies[3]["error"]["code"], -32600);
    assert_eq!(replies[4]["id"], 4);
    assert!(String::from_utf8_lossy(&out.stderr).contains("ready"));
}

#[test]
fn binary_refuses_bad_arguments() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_openstitch-mcp"))
            .args(args)
            .output()
            .expect("run")
    };
    assert_eq!(run(&["--bogus"]).status.code(), Some(64));
    assert_eq!(run(&["--root"]).status.code(), Some(64));
    assert_eq!(run(&["--max-request-bytes", "10"]).status.code(), Some(64));
    assert_eq!(
        run(&["--root", "/definitely/not/here"]).status.code(),
        Some(66)
    );
    assert!(run(&["--help"]).status.success());
}
