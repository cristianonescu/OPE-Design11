use crate::preview;
use crate::tools::{self, Tool, ToolKind};
use openstitch_design::Compiler;
use openstitch_engine::{Capabilities, Params, Sandbox, SandboxError, Session};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::PathBuf;

/// Newest first; `initialize` gets the client's version if listed here, else the newest.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
pub const SERVER_NAME: &str = "openstitch";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Deeper JSON is refused before decoding.
pub const MAX_NESTING: usize = 64;

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

const INSTRUCTIONS: &str = "OpenStitch embroidery engine. Each tool is one engine command on a single open design \
(lengths in mm, angles in degrees; object ids come from object_list). Start with design_new, \
add shapes, set sewing with object_setRegion/object_setLine, check with preview, then \
design_export (pes, dst, jef, ...) or project_save (.osd). Edits are undoable with \
history_undo. File paths must lie inside the server's configured roots.";

#[derive(Clone, Debug)]
pub struct Config {
    /// Directories whose files commands may read.
    pub read_roots: Vec<PathBuf>,
    /// Directories whose files commands may read and write.
    pub write_roots: Vec<PathBuf>,
    /// Largest accepted request line, in bytes.
    pub max_request_bytes: usize,
    /// Largest reply line, in bytes; a bigger reply is replaced by an error.
    pub max_response_bytes: usize,
    pub max_batch_steps: usize,
    pub max_preview_pixels: u32,
    /// What the client may do; tools needing anything else are not listed and are refused.
    pub capabilities: Capabilities,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            read_roots: Vec::new(),
            write_roots: Vec::new(),
            max_request_bytes: 1 << 20,
            max_response_bytes: 8 << 20,
            max_batch_steps: 256,
            max_preview_pixels: 2048,
            capabilities: Capabilities::FULL,
        }
    }
}

/// Model Context Protocol server over newline-delimited JSON-RPC 2.0 (the stdio transport).
/// It owns one [`Session`]; every tool call goes through `Session::execute`, so the MCP surface
/// is exactly the command registry plus `run_batch` and `preview`.
pub struct Server {
    config: Config,
    session: Session,
    tools: Vec<Tool>,
    by_name: HashMap<String, usize>,
    protocol_version: Option<&'static str>,
}

impl Server {
    /// Fails if a root is not an existing directory.
    pub fn new(config: Config) -> Result<Self, SandboxError> {
        let sandbox = Sandbox::new(&config.read_roots, &config.write_roots)?;
        let mut session = Session::with_sandbox(sandbox);
        session.set_capabilities(config.capabilities);
        let tools = tools::tools(
            &session.permitted_commands(),
            config.max_batch_steps,
            config.max_preview_pixels,
        );
        let by_name = tools
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name.clone(), i))
            .collect();
        Ok(Server {
            config,
            session,
            tools,
            by_name,
            protocol_version: None,
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn sandbox(&self) -> Option<&Sandbox> {
        self.session.sandbox()
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    pub fn protocol_version(&self) -> Option<&'static str> {
        self.protocol_version
    }

    // Transport

    /// Handles one line from the client; returns the reply line, or `None` for notifications.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        if line.len() > self.config.max_request_bytes {
            return Some(self.oversized_request_reply());
        }
        if line.trim().is_empty() {
            return None;
        }
        if nesting_depth(line) > MAX_NESTING {
            return Some(self.encode(&error(
                None,
                INVALID_REQUEST,
                &format!("JSON nested deeper than {MAX_NESTING} levels"),
            )));
        }
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return Some(self.encode(&error(None, PARSE_ERROR, "Parse error")));
        };
        self.handle_message(&message)
            .map(|reply| self.encode(&reply))
    }

    pub fn oversized_request_reply(&self) -> String {
        self.encode(&error(
            None,
            INVALID_REQUEST,
            &format!("request exceeds {} bytes", self.config.max_request_bytes),
        ))
    }

    /// Serializes a reply, replacing it with an error if it is over the response cap.
    pub fn encode(&self, reply: &Value) -> String {
        let text = reply.to_string();
        if text.len() <= self.config.max_response_bytes {
            return text;
        }
        let message = format!(
            "response of {} bytes exceeds the {}-byte limit",
            text.len(),
            self.config.max_response_bytes
        );
        error(reply.get("id").cloned(), INTERNAL_ERROR, &message).to_string()
    }

    // JSON-RPC

    /// Handles one decoded message; `None` when no reply is due (notifications, client responses).
    pub fn handle_message(&mut self, message: &Value) -> Option<Value> {
        let Some(m) = message.as_object() else {
            // JSON-RPC batches were removed from MCP in 2025-06-18.
            return Some(error(
                None,
                INVALID_REQUEST,
                "expected a single JSON-RPC object",
            ));
        };
        if m.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Some(error(
                valid_id(m.get("id")),
                INVALID_REQUEST,
                "jsonrpc must be \"2.0\"",
            ));
        }
        let Some(method) = m.get("method").and_then(Value::as_str) else {
            // A response to a server request; this server sends none, so ignore it.
            if m.contains_key("result") || m.contains_key("error") {
                return None;
            }
            return Some(error(
                valid_id(m.get("id")),
                INVALID_REQUEST,
                "missing method",
            ));
        };
        // Notification: notifications/initialized, notifications/cancelled, ...
        let raw_id = m.get("id")?;
        let Some(id) = valid_id(Some(raw_id)) else {
            return Some(error(
                None,
                INVALID_REQUEST,
                "id must be a string or integer",
            ));
        };
        let empty = Value::Object(Map::new());
        let params = m.get("params").unwrap_or(&empty);
        if !params.is_object() {
            return Some(error(Some(id), INVALID_PARAMS, "params must be an object"));
        }

        Some(match method {
            "initialize" => {
                let result = self.initialize(params);
                success(id, result)
            }
            "ping" => success(id, json!({})),
            "tools/list" | "tools/call" if self.protocol_version.is_none() => {
                error(Some(id), INVALID_REQUEST, "call initialize first")
            }
            "tools/list" => {
                let list: Vec<Value> = self.tools.iter().map(|t| t.definition.clone()).collect();
                success(id, json!({ "tools": list }))
            }
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error(
                        Some(id),
                        INVALID_PARAMS,
                        "tools/call needs a string 'name'",
                    ));
                };
                let Some(&index) = self.by_name.get(name) else {
                    return Some(error(Some(id), INVALID_PARAMS, &self.unknown_tool(name)));
                };
                let arguments = params.get("arguments").unwrap_or(&empty);
                if !arguments.is_object() {
                    return Some(error(
                        Some(id),
                        INVALID_PARAMS,
                        "arguments must be an object",
                    ));
                }
                let kind = self.tools[index].kind.clone();
                let result = self.call(&kind, arguments);
                success(id, result)
            }
            _ => error(
                Some(id),
                METHOD_NOT_FOUND,
                &format!("method not found: {method}"),
            ),
        })
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let version = PROTOCOL_VERSIONS
            .iter()
            .copied()
            .find(|v| Some(*v) == requested)
            .unwrap_or(PROTOCOL_VERSIONS[0]);
        self.protocol_version = Some(version);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": SERVER_NAME, "title": "OpenStitch", "version": SERVER_VERSION },
            "instructions": self.instructions(),
        })
    }

    /// An engine command this server does not offer because of its capabilities.
    fn withheld(&self, name: &str) -> bool {
        self.session
            .commands()
            .iter()
            .any(|c| c.id == name || tools::tool_name(c.id) == name)
    }

    fn unknown_tool(&self, name: &str) -> String {
        if self.withheld(name) {
            format!(
                "tool '{name}' is not available: this server allows only {:?}",
                self.config.capabilities.names()
            )
        } else {
            format!("unknown tool '{name}'")
        }
    }

    /// The usage notes, plus what this client may not do.
    fn instructions(&self) -> String {
        let caps = self.config.capabilities;
        let mut text = INSTRUCTIONS.to_string();
        if !caps.edit {
            text.push_str(
                " This server is read-only: the design can be inspected and previewed, not edited.",
            );
        }
        if !caps.read_files && !caps.write_files {
            text.push_str(" File access is off: no tool opens, imports, saves or exports files.");
        } else if !caps.write_files {
            text.push_str(" Files cannot be written.");
        }
        text
    }

    // Tools

    /// A `CallToolResult`. Command failures are tool errors (`isError`), so the model sees them.
    pub fn call(&mut self, kind: &ToolKind, arguments: &Value) -> Value {
        let outcome = match kind {
            ToolKind::Command(id) => self
                .session
                .execute(id, Some(arguments))
                .map(|v| tool_success(&v))
                .map_err(|e| e.to_string()),
            ToolKind::Batch => self.run_batch(arguments),
            ToolKind::Preview => self.preview(arguments),
        };
        outcome.unwrap_or_else(|message| tool_failure(&message))
    }

    fn run_batch(&mut self, arguments: &Value) -> Result<Value, String> {
        let p = Params::new("run_batch", Some(arguments)).map_err(|e| e.to_string())?;
        let steps = p.array("steps").map_err(|e| e.to_string())?;
        let continue_on_error = p
            .bool("continueOnError", Some(false))
            .map_err(|e| e.to_string())?;
        if steps.is_empty() {
            return Err("run_batch: steps must not be empty".into());
        }
        if steps.len() > self.config.max_batch_steps {
            return Err(format!(
                "run_batch: {} steps exceeds the limit of {}",
                steps.len(),
                self.config.max_batch_steps
            ));
        }
        // Check every step before running any.
        let empty = Value::Object(Map::new());
        let mut plan: Vec<(String, &'static str, Value)> = Vec::with_capacity(steps.len());
        for (i, step) in steps.iter().enumerate() {
            let name = step
                .as_object()
                .and_then(|o| o.get("tool").or_else(|| o.get("command")))
                .and_then(Value::as_str)
                .ok_or_else(|| format!("run_batch: step {i} needs a string 'tool'"))?;
            let args = step
                .get("arguments")
                .or_else(|| step.get("params"))
                .unwrap_or(&empty);
            if !args.is_object() {
                return Err(format!("run_batch: step {i} arguments must be an object"));
            }
            let index = self
                .by_name
                .get(name)
                .or_else(|| self.by_name.get(&tools::tool_name(name)));
            let Some(ToolKind::Command(id)) = index.map(|&i| &self.tools[i].kind) else {
                return Err(if self.withheld(name) {
                    format!("run_batch: step {i}: {}", self.unknown_tool(name))
                } else {
                    format!("run_batch: step {i}: '{name}' is not an engine command")
                });
            };
            plan.push((name.to_string(), id, args.clone()));
        }
        let mut results = Vec::with_capacity(plan.len());
        let mut failed = 0;
        for (i, (name, id, args)) in plan.iter().enumerate() {
            match self.session.execute(id, Some(args)) {
                Ok(value) => {
                    results.push(json!({ "step": i, "tool": name, "ok": true, "result": value }))
                }
                Err(e) => {
                    failed += 1;
                    results.push(
                        json!({ "step": i, "tool": name, "ok": false, "error": e.to_string() }),
                    );
                    if !continue_on_error {
                        break;
                    }
                }
            }
        }
        let summary = json!({ "steps": plan.len(), "ran": results.len(), "failed": failed, "results": results });
        let mut r = tool_success(&summary);
        if failed > 0 {
            r["isError"] = json!(true);
        }
        Ok(r)
    }

    fn preview(&mut self, arguments: &Value) -> Result<Value, String> {
        let p = Params::new("preview", Some(arguments)).map_err(|e| e.to_string())?;
        let size = p
            .int(
                "size",
                Some(512),
                64,
                i64::from(self.config.max_preview_pixels),
            )
            .map_err(|e| e.to_string())?;
        let show_travel = p
            .bool("showTravel", Some(false))
            .map_err(|e| e.to_string())?;
        let design = self.session.design();
        let pattern = Compiler::for_design(design).compile(design);
        let png = preview::png(design, &pattern, size as u32, show_travel)?;
        let stats = self
            .session
            .execute("design.generate", None)
            .map_err(|e| e.to_string())?;
        Ok(json!({
            "content": [
                { "type": "image", "data": base64_encode(&png), "mimeType": "image/png" },
                { "type": "text", "text": stats.to_string() },
            ],
            "isError": false,
        }))
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A JSON-RPC id this server echoes: a string or an integer.
fn valid_id(id: Option<&Value>) -> Option<Value> {
    match id? {
        v @ Value::String(_) => Some(v.clone()),
        v @ Value::Number(n) if n.is_i64() || n.is_u64() => Some(v.clone()),
        Value::Number(n) => n
            .as_f64()
            .filter(|f| f.fract() == 0.0 && f.abs() < 9e15)
            .map(|f| json!(f as i64)),
        _ => None,
    }
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}

/// Text content is the serialized result; `structuredContent` only when it is an object.
pub fn tool_success(value: &Value) -> Value {
    let mut r =
        json!({ "content": [{ "type": "text", "text": value.to_string() }], "isError": false });
    if value.is_object() {
        r["structuredContent"] = value.clone();
    }
    r
}

pub fn tool_failure(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

/// Bracket depth outside strings, so hostile nesting is refused before the decoder sees it.
pub fn nesting_depth(text: &str) -> usize {
    let (mut depth, mut deepest, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for b in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'{' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}
