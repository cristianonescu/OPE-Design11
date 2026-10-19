//! MCP server tests: drive the server the way a client does, one
//! serialized line in, one line out.

use base64::Engine;
use openstitch_engine::Session;
#[cfg(unix)]
use openstitch_engine::{Access, Sandbox};
use openstitch_mcp::{Config, Line, LineReader, PROTOCOL_VERSIONS, Server, tool_name};
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

fn temp_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-mcp-tests-{}-{t}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

struct Client {
    server: Server,
    next_id: i64,
}

impl Client {
    fn new(config: Config) -> Self {
        let mut c = Client::uninitialized(config);
        c.request(
            "initialize",
            Some(json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "tests", "version": "1"}})),
        );
        assert_eq!(
            c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})),
            None
        );
        c
    }

    fn uninitialized(config: Config) -> Self {
        Client {
            server: Server::new(config).expect("server"),
            next_id: 1,
        }
    }

    fn send(&mut self, message: &Value) -> Option<Value> {
        self.server
            .handle_line(&message.to_string())
            .map(|l| serde_json::from_str(&l).expect("reply is JSON"))
    }

    fn line(&mut self, line: &str) -> String {
        self.server.handle_line(line).expect("reply")
    }

    fn request(&mut self, method: &str, params: Option<Value>) -> Value {
        let mut m = json!({"jsonrpc": "2.0", "id": self.next_id, "method": method});
        if let Some(p) = params {
            m["params"] = p;
        }
        self.next_id += 1;
        self.send(&m).expect("reply")
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let reply = self.request(
            "tools/call",
            Some(json!({"name": tool, "arguments": arguments})),
        );
        reply
            .get("result")
            .cloned()
            .unwrap_or_else(|| panic!("no result in {reply}"))
    }

    fn session(&self) -> &Session {
        self.server.session()
    }

    fn objects(&self) -> usize {
        self.session().design().objects.len()
    }
}

fn text(result: &Value) -> String {
    result["content"]
        .as_array()
        .and_then(|items| items.iter().find(|i| i["type"] == "text"))
        .and_then(|i| i["text"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn rect(x: f64) -> Value {
    json!({"x": x, "y": 0, "width": 10, "height": 10})
}

// Handshake

#[test]
fn initialize_negotiates_version_and_advertises_tools() {
    let mut c = Client::uninitialized(Config::default());
    let reply = c.request(
        "initialize",
        Some(json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}})),
    );
    assert_eq!(reply["jsonrpc"], "2.0");
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
    assert!(reply["result"]["capabilities"]["tools"].is_object());
    assert_eq!(reply["result"]["serverInfo"]["name"], "openstitch");
    // The id is echoed as an integer, and keys are sorted.
    assert_eq!(
        c.line(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#),
        r#"{"id":7,"jsonrpc":"2.0","result":{}}"#
    );
    assert_eq!(
        c.line(r#"{"jsonrpc":"2.0","id":"a","method":"ping"}"#),
        r#"{"id":"a","jsonrpc":"2.0","result":{}}"#
    );
}

#[test]
fn unknown_version_gets_the_newest() {
    let mut c = Client::uninitialized(Config::default());
    let reply = c.request(
        "initialize",
        Some(json!({"protocolVersion": "1999-01-01", "capabilities": {}})),
    );
    assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
}

#[test]
fn tools_need_initialize() {
    let mut c = Client::uninitialized(Config::default());
    assert_eq!(c.request("tools/list", None)["error"]["code"], -32600);
    assert_eq!(
        c.request("tools/call", Some(json!({"name": "design_info"})))["error"]["code"],
        -32600
    );
    assert_eq!(c.request("ping", None)["result"], json!({}));
}

#[test]
fn protocol_errors() {
    let mut c = Client::new(Config::default());
    assert!(c.line("{not json").contains("-32700"));
    assert_eq!(c.request("no/such/method", None)["error"]["code"], -32601);
    let code = |c: &mut Client, m: Value| c.send(&m).map(|r| r["error"]["code"].clone());
    assert_eq!(
        code(&mut c, json!({"jsonrpc": "1.0", "id": 1, "method": "ping"})),
        Some(json!(-32600))
    );
    assert_eq!(
        code(
            &mut c,
            json!([{"jsonrpc": "2.0", "id": 1, "method": "ping"}])
        ),
        Some(json!(-32600))
    );
    assert_eq!(
        code(
            &mut c,
            json!({"jsonrpc": "2.0", "id": 1.5, "method": "ping"})
        ),
        Some(json!(-32600))
    );
    assert_eq!(
        code(
            &mut c,
            json!({"jsonrpc": "2.0", "id": null, "method": "ping"})
        ),
        Some(json!(-32600))
    );
    assert_eq!(
        code(
            &mut c,
            json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": [1]})
        ),
        Some(json!(-32602))
    );
    assert_eq!(
        code(&mut c, json!({"jsonrpc": "2.0", "id": 1})),
        Some(json!(-32600))
    );
    // Notifications and client responses get no reply.
    assert_eq!(c.send(&json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 1}})), None);
    assert_eq!(
        c.send(&json!({"jsonrpc": "2.0", "id": 4, "result": {}})),
        None
    );
    assert_eq!(c.server.handle_line("   "), None);
    assert_eq!(
        c.request("tools/call", Some(json!({"name": "no_such_tool"})))["error"]["code"],
        -32602
    );
    assert_eq!(
        c.request(
            "tools/call",
            Some(json!({"name": "design_info", "arguments": [1]}))
        )["error"]["code"],
        -32602
    );
    assert_eq!(
        c.request("tools/call", Some(json!({"arguments": {}})))["error"]["code"],
        -32602
    );
}

// tools/list

fn tool_list(c: &mut Client) -> Vec<Value> {
    c.request("tools/list", None)["result"]["tools"]
        .as_array()
        .expect("tools")
        .clone()
}

#[test]
fn one_tool_per_command_plus_batch_and_preview() {
    let mut c = Client::new(Config::default());
    let tools = tool_list(&mut c);
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    let commands = Session::new(".")
        .commands()
        .iter()
        .map(|s| tool_name(s.id))
        .collect::<Vec<_>>();
    assert_eq!(names.len(), commands.len() + 2);
    let expected: HashSet<String> = commands
        .into_iter()
        .chain(["run_batch".into(), "preview".into()])
        .collect();
    assert_eq!(
        names.iter().map(|n| n.to_string()).collect::<HashSet<_>>(),
        expected
    );
    for name in &names {
        assert!(
            (1..=64).contains(&name.len())
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "{name}"
        );
    }
    for tool in &tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert!(tool["description"].is_string());
        assert!(tool["inputSchema"].get("anyOf").is_none());
    }
    let info = tools
        .iter()
        .find(|t| t["name"] == "design_info")
        .expect("design_info");
    assert_eq!(info["annotations"]["readOnlyHint"], true);
    let export = tools
        .iter()
        .find(|t| t["name"] == "design_export")
        .expect("design_export");
    assert_eq!(export["annotations"]["destructiveHint"], true);
}

#[test]
fn schemas_come_from_param_docs() {
    let mut c = Client::new(Config::default());
    let tools = tool_list(&mut c);
    let schema = |name: &str| {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .map(|t| t["inputSchema"].clone())
            .expect(name)
    };

    let rect = schema("shape_rectangle");
    assert_eq!(rect["properties"]["width"]["type"], "number");
    assert_eq!(
        rect["properties"]["fill"]["type"],
        json!(["string", "integer"])
    );
    assert_eq!(rect["required"], json!(["x", "y", "width", "height"]));
    assert_eq!(rect["additionalProperties"], false);

    let line = schema("object_setLine");
    assert_eq!(
        line["properties"]["type"]["enum"],
        json!([
            "running",
            "triple",
            "zigzag",
            "motif",
            "stem",
            "candlewick",
            "eStitch",
            "vStitch",
            "chain",
            "cutting",
            "none"
        ])
    );
    assert_eq!(line["properties"]["runTimes"]["maximum"], 6);
    assert_eq!(
        line["properties"]["arrange"]["enum"],
        json!(["normal", "mirror", "alternate"])
    );
    assert_eq!(line["properties"]["ids"]["type"], "array");
    assert_eq!(line["properties"]["id"]["type"], "integer");
    assert!(line.get("required").is_none());
    assert_eq!(line["properties"]["density"]["type"], "number");

    assert_eq!(
        schema("shape_polygon")["properties"]["points"]["items"]["maxItems"],
        2
    );
    assert_eq!(schema("design_export")["required"], json!(["path"]));
    // "pes|pec|dst|..." is open-ended, so it is a described string, not an enum.
    assert!(
        schema("design_export")["properties"]["format"]
            .get("enum")
            .is_none()
    );
}

// tools/call

#[test]
fn command_success_returns_text_and_structured_content() {
    let mut c = Client::new(Config::default());
    c.call("design_new", json!({"name": "badge"}));
    let r = c.call(
        "shape_rectangle",
        json!({"x": 10, "y": 10, "width": 30, "height": 20, "fill": "Red"}),
    );
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["id"], 1);
    assert_eq!(
        serde_json::from_str::<Value>(&text(&r)).expect("json")["id"],
        1
    );
    assert_eq!(c.objects(), 1);
    // A non-object result has no structuredContent.
    let list = c.call("object_list", json!({}));
    assert!(list.get("structuredContent").is_none());
}

#[test]
fn command_failure_is_a_tool_error() {
    let mut c = Client::new(Config::default());
    let missing = c.call("shape_rectangle", json!({"x": 10}));
    assert_eq!(missing["isError"], true);
    assert!(text(&missing).contains("missing parameter"), "{missing}");
    let not_found = c.call("object_delete", json!({"id": 99}));
    assert_eq!(not_found["isError"], true);
    assert_eq!(c.objects(), 0);
}

#[test]
fn preview_returns_png() {
    let mut c = Client::new(Config::default());
    c.call(
        "shape_ellipse",
        json!({"cx": 50, "cy": 50, "rx": 20, "fill": "#cc0000"}),
    );
    let r = c.call("preview", json!({"size": 128, "showTravel": true}));
    assert_eq!(r["isError"], false, "{r}");
    let image = &r["content"][0];
    assert_eq!(image["mimeType"], "image/png");
    let png = base64::engine::general_purpose::STANDARD
        .decode(image["data"].as_str().expect("data"))
        .expect("base64");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    assert!(text(&r).contains("stitches"));

    // Decode it: longest side is the requested size, and the fill colour is drawn.
    let decoder = png::Decoder::new(std::io::Cursor::new(png));
    let mut reader = decoder.read_info().expect("png header");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    assert_eq!(info.width.max(info.height), 128);
    let pixels = &buf[..info.buffer_size()];
    assert!(
        pixels
            .chunks_exact(3)
            .any(|p| p[0] > 150 && p[1] < 80 && p[2] < 80),
        "no red pixels"
    );

    assert_eq!(c.call("preview", json!({"size": 100_000}))["isError"], true);
    assert_eq!(c.call("preview", json!({"size": 10}))["isError"], true);
}

#[test]
fn responses_over_the_cap_become_errors() {
    let mut c = Client::new(Config {
        max_response_bytes: 2048,
        ..Config::default()
    });
    c.call("shape_ellipse", json!({"cx": 50, "cy": 50, "rx": 20}));
    let reply = c.request(
        "tools/call",
        Some(json!({"name": "preview", "arguments": {"size": 512}})),
    );
    assert_eq!(reply["error"]["code"], -32603);
    assert!(!reply["id"].is_null());
}

#[test]
fn oversized_and_deep_requests_are_refused() {
    let mut c = Client::new(Config {
        max_request_bytes: 1024,
        ..Config::default()
    });
    let big = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{"pad":"{}"}}}}"#,
        "x".repeat(2000)
    );
    assert!(c.line(&big).contains("exceeds 1024 bytes"));
    let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
    assert!(c.line(&deep).contains("-32600"));
    // Brackets inside strings do not count.
    let quoted = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"ping","params":{{"s":"{}"}}}}"#,
        "[".repeat(100)
    );
    assert!(c.line(&quoted).contains(r#""result":{}"#));
}

/// Hands out one chunk per `read`, like a pipe.
struct Chunks(VecDeque<Vec<u8>>);

impl std::io::Read for Chunks {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let Some(chunk) = self.0.pop_front() else {
            return Ok(0);
        };
        buf[..chunk.len()].copy_from_slice(&chunk);
        Ok(chunk.len())
    }
}

#[test]
fn line_reader_drops_oversized_lines_without_buffering() {
    let chunks = Chunks(VecDeque::from([
        b"ok\r\n".to_vec(),
        vec![b'y'; 40],
        vec![b'y'; 40],
        b"y\nnext\nlast".to_vec(),
    ]));
    let lines: Vec<Line> = LineReader::new(chunks, 16).collect();
    assert_eq!(
        lines,
        [
            Line::Text("ok".into()),
            Line::Oversized,
            Line::Text("next".into()),
            Line::Text("last".into())
        ]
    );
    // An over-long final line without a newline is reported once.
    let tail: Vec<Line> = LineReader::new(Chunks(VecDeque::from([vec![b'z'; 40]])), 16).collect();
    assert_eq!(tail, [Line::Oversized]);
}

// run_batch

#[test]
fn batch_runs_steps_in_order() {
    let mut c = Client::new(Config::default());
    let r = c.call(
        "run_batch",
        json!({"steps": [
            {"tool": "design_new", "arguments": {"name": "b"}},
            {"tool": "shape.rectangle", "arguments": rect(0.0)},
            {"tool": "object_move", "arguments": {"id": 1, "dx": 5}},
        ]}),
    );
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["ran"], 3);
    assert_eq!(c.objects(), 1);
}

#[test]
fn batch_stops_at_first_failure_unless_asked() {
    let mut c = Client::new(Config::default());
    let steps = json!([
        {"tool": "shape_rectangle", "arguments": rect(0.0)},
        {"tool": "object_delete", "arguments": {"id": 42}},
        {"tool": "shape_rectangle", "arguments": rect(20.0)},
    ]);
    let stopped = c.call("run_batch", json!({"steps": steps}));
    assert_eq!(stopped["isError"], true);
    assert_eq!(stopped["structuredContent"]["ran"], 2);
    assert_eq!(stopped["structuredContent"]["results"][1]["ok"], false);
    assert_eq!(c.objects(), 1);
    let kept = c.call(
        "run_batch",
        json!({"steps": steps, "continueOnError": true}),
    );
    assert_eq!(kept["structuredContent"]["ran"], 3);
    assert_eq!(kept["structuredContent"]["failed"], 1);
    assert_eq!(c.objects(), 3);
}

#[test]
fn batch_enforces_step_limit_and_validates_before_running() {
    let mut c = Client::new(Config::default());
    let step =
        json!({"tool": "shape_rectangle", "arguments": {"x": 0, "y": 0, "width": 1, "height": 1}});
    let too_many = c.call("run_batch", json!({"steps": vec![step.clone(); 257]}));
    assert_eq!(too_many["isError"], true);
    assert!(text(&too_many).contains("limit of 256"), "{too_many}");
    assert_eq!(c.objects(), 0);

    assert_eq!(
        c.call("run_batch", json!({"steps": vec![step.clone(); 256]}))["isError"],
        false
    );
    assert_eq!(c.objects(), 256);

    for bad in [
        json!([step, {"tool": "preview"}]),
        json!([step, {"tool": "run_batch", "arguments": {"steps": []}}]),
        json!([step, {"tool": "nope"}]),
        json!([step, {"arguments": {}}]),
        json!([step, {"tool": "design_info", "arguments": [1]}]),
        json!([step, 7]),
        json!([]),
        json!("steps"),
    ] {
        assert_eq!(
            c.call("run_batch", json!({"steps": bad}))["isError"],
            true,
            "{bad}"
        );
    }
    assert_eq!(c.objects(), 256);
    // Smaller configured limit.
    let mut small = Client::new(Config {
        max_batch_steps: 2,
        ..Config::default()
    });
    assert!(text(&small.call("run_batch", json!({"steps": vec![step; 3]}))).contains("limit of 2"));
}

// File roots

fn sandboxed(root: &Path, read: &[PathBuf]) -> Client {
    Client::new(Config {
        write_roots: vec![root.to_path_buf()],
        read_roots: read.to_vec(),
        ..Config::default()
    })
}

#[test]
fn reads_and_writes_inside_the_root() {
    let root = temp_dir();
    std::fs::create_dir_all(root.join("out")).expect("out");
    let mut c = sandboxed(&root, &[]);
    c.call(
        "shape_rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    assert_eq!(
        c.call("design_export", json!({"path": "out/a.dst"}))["isError"],
        false
    );
    assert!(root.join("out/a.dst").exists());
    let absolute = root.join("b.osd").display().to_string();
    assert_eq!(
        c.call("project_save", json!({ "path": absolute }))["isError"],
        false
    );
    assert_eq!(
        c.call("project_open", json!({"path": "b.osd"}))["isError"],
        false
    );
    assert_eq!(
        c.call("stitches_import", json!({"path": "out/a.dst"}))["isError"],
        false
    );
}

#[cfg(unix)]
#[test]
fn rejects_escapes() {
    let root = temp_dir();
    let outside = temp_dir();
    std::os::unix::fs::symlink(&outside, root.join("link")).expect("symlink");
    std::os::unix::fs::symlink(outside.join("target.osd"), root.join("file.osd")).expect("symlink");
    let mut c = sandboxed(&root, &[]);
    c.call(
        "shape_rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    let escaped = outside.join("x.osd").display().to_string();
    for path in [
        escaped.as_str(),
        "../x.osd",
        "sub/../../x.osd",
        "./../x.osd",
        "link/x.osd",
        "file.osd",
        "/etc/passwd",
        "missing/dir/x.osd",
        "",
    ] {
        let r = c.call("project_save", json!({ "path": path }));
        assert_eq!(r["isError"], true, "{path:?} was accepted");
    }
    assert!(std::fs::read_dir(&outside).expect("dir").next().is_none());
    assert_eq!(
        c.call("project_open", json!({"path": "/etc/hosts"}))["isError"],
        true
    );
    assert_eq!(
        c.call("stitches_import", json!({"path": "link/../../x.dst"}))["isError"],
        true
    );
    assert_eq!(
        c.call("design_export", json!({"path": "link/x.dst"}))["isError"],
        true
    );
}

#[test]
fn read_roots_are_read_only_and_no_roots_means_no_files() {
    let root = temp_dir();
    let library = temp_dir();
    let mut c = sandboxed(&root, std::slice::from_ref(&library));
    c.call(
        "shape_rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    let in_library = library.join("a.osd");
    let in_library_str = in_library.display().to_string();
    assert_eq!(
        c.call("project_save", json!({ "path": in_library_str }))["isError"],
        true
    );
    c.call("project_save", json!({"path": "a.osd"}));
    std::fs::copy(root.join("a.osd"), &in_library).expect("copy");
    assert_eq!(
        c.call("project_open", json!({ "path": in_library_str }))["isError"],
        false
    );

    let mut none = Client::new(Config::default());
    none.call(
        "shape_rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    let r = none.call("project_save", json!({"path": "a.osd"}));
    assert_eq!(r["isError"], true);
    assert!(text(&r).contains("no write root"), "{r}");
    assert!(text(&none.call("project_open", json!({"path": "a.osd"}))).contains("no read root"));

    // Read-only server: relative paths resolve against the read root.
    let mut reader = Client::new(Config {
        read_roots: vec![library.clone()],
        ..Config::default()
    });
    assert_eq!(
        reader.call("project_open", json!({"path": "a.osd"}))["isError"],
        false
    );
    assert_eq!(
        reader.call("project_save", json!({"path": "b.osd"}))["isError"],
        true
    );
}

#[cfg(unix)]
#[test]
fn sandbox_resolves_symlinked_roots() {
    let real = temp_dir();
    let alias = temp_dir().join("alias");
    std::os::unix::fs::symlink(&real, &alias).expect("symlink");
    let sandbox = Sandbox::new::<PathBuf, _>(&[], &[&alias]).expect("sandbox");
    let p = sandbox
        .resolve(&alias.join("x.pes").display().to_string(), Access::Write)
        .expect("inside");
    assert!(p.starts_with(real.canonicalize().expect("real")));
    assert!(sandbox.resolve("../x.pes", Access::Read).is_err());
    assert!(Sandbox::new(&[real.join("nope")], &[] as &[PathBuf]).is_err());
    assert!(
        Server::new(Config {
            write_roots: vec![real.join("nope")],
            ..Config::default()
        })
        .is_err()
    );
}

#[test]
fn curve_parameters_get_array_schemas() {
    let mut c = Client::new(Config::default());
    let tools = tool_list(&mut c);
    let path = tools
        .iter()
        .find(|t| t["name"] == "shape_path")
        .expect("shape_path tool");
    let curves = &path["inputSchema"]["properties"]["curves"];
    assert_eq!(curves["type"], "array");
    assert_eq!(curves["items"]["anyOf"][0]["type"], "null");
    assert_eq!(curves["items"]["anyOf"][1]["minItems"], 2);
    let seg = tools
        .iter()
        .find(|t| t["name"] == "object_setSegment")
        .expect("object_setSegment tool");
    let curve = &seg["inputSchema"]["properties"]["curve"];
    assert_eq!(curve["type"], "array");
    assert_eq!(curve["items"]["items"]["type"], "number");
}
