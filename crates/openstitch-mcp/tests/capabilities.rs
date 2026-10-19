//! Capability flags: read-only mode and file access off hide tools from `tools/list` and refuse
//! them in `tools/call` and `run_batch`.

use openstitch_engine::Capabilities;
use openstitch_mcp::{Config, Server};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-mcp-caps-{}-{t}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

struct Client {
    server: Server,
    next: i64,
}

impl Client {
    fn new(config: Config) -> (Self, Value) {
        let mut c = Client {
            server: Server::new(config).expect("server"),
            next: 1,
        };
        let init = c.request("initialize", json!({"protocolVersion": "2025-11-25"}));
        (c, init)
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let m = json!({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params});
        self.next += 1;
        let line = self.server.handle_line(&m.to_string()).expect("reply");
        serde_json::from_str(&line).expect("json")
    }

    fn tool_names(&mut self) -> Vec<String> {
        let r = self.request("tools/list", json!({}));
        r["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .filter_map(|t| t["name"].as_str().map(String::from))
            .collect()
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }
}

#[test]
fn read_only_lists_and_runs_only_reads() {
    let root = temp_dir();
    let (mut c, init) = Client::new(Config {
        write_roots: vec![root.clone()],
        capabilities: Capabilities::FULL.read_only(),
        ..Config::default()
    });
    assert!(
        init["result"]["instructions"]
            .as_str()
            .is_some_and(|s| s.contains("read-only"))
    );
    let names = c.tool_names();
    for present in [
        "design_info",
        "object_list",
        "machine_hoops",
        "preview",
        "run_batch",
    ] {
        assert!(names.iter().any(|n| n == present), "{present} missing");
    }
    for absent in [
        "shape_rectangle",
        "history_undo",
        "design_export",
        "project_save",
        "project_open",
        "machine_set",
    ] {
        assert!(!names.iter().any(|n| n == absent), "{absent} listed");
    }

    let r = c.call(
        "shape_rectangle",
        json!({"x": 0, "y": 0, "width": 5, "height": 5}),
    );
    assert_eq!(r["error"]["code"], -32602);
    assert!(
        r["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("not available"))
    );

    let r = c.call(
        "run_batch",
        json!({"steps": [{"tool": "design_info"}, {"tool": "design_export", "arguments": {"path": "x.pes"}}]}),
    );
    assert_eq!(r["result"]["isError"], true, "{r}");
    assert!(r.to_string().contains("not available"));
    assert!(!root.join("x.pes").exists());
    assert!(c.server.session().design().objects.is_empty());

    let r = c.call("preview", json!({"size": 128}));
    assert_ne!(r["result"]["isError"], true, "{r}");
    let r = c.call("design_info", json!({}));
    assert_ne!(r["result"]["isError"], true, "{r}");
}

#[test]
fn no_files_hides_file_tools_but_allows_edits() {
    let (mut c, init) = Client::new(Config {
        capabilities: Capabilities::FULL.without_files(),
        ..Config::default()
    });
    assert!(
        init["result"]["instructions"]
            .as_str()
            .is_some_and(|s| s.contains("File access is off"))
    );
    let names = c.tool_names();
    for absent in [
        "design_export",
        "project_save",
        "project_open",
        "stitches_import",
    ] {
        assert!(!names.iter().any(|n| n == absent), "{absent} listed");
    }
    assert!(names.iter().any(|n| n == "shape_rectangle"));
    let r = c.call(
        "shape_rectangle",
        json!({"x": 0, "y": 0, "width": 5, "height": 5}),
    );
    assert_ne!(r["result"]["isError"], true, "{r}");
    let r = c.call("project_save", json!({"path": "a.osd"}));
    assert_eq!(r["error"]["code"], -32602);
}

#[test]
fn full_capabilities_list_every_command() {
    let (mut c, _) = Client::new(Config::default());
    let names = c.tool_names();
    assert_eq!(
        names.len(),
        c.server.session().commands().len() + 2,
        "every command plus run_batch and preview"
    );
}

#[test]
fn binary_refuses_no_files_with_roots() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_openstitch-mcp"))
        .args(["--no-files", "--root", "."])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--no-files"));
}
