use crate::capabilities::Capabilities;
use crate::commands::{self, CommandSpec, Effect};
use crate::params::{CommandError, CommandResult, Params};
use crate::sandbox::{Access, Sandbox};
use openstitch_design::Design;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// State a command runs against.
#[derive(Clone, Debug)]
pub struct EngineState {
    pub design: Design,
    /// Relative paths resolve against this directory when there is no sandbox.
    pub base_dir: PathBuf,
    /// When set (automation transports), every file path must resolve inside its roots.
    pub sandbox: Option<Sandbox>,
    /// What the client may do; file paths are refused here when file access is off.
    pub capabilities: Capabilities,
    /// Attribute sets and user thread charts (not part of the design).
    pub user: crate::user::UserSettings,
}

impl EngineState {
    /// The path a file command may open for `access`. Every command that touches a file must
    /// go through here so the sandbox sees it.
    pub fn resolve(
        &self,
        path: &str,
        access: Access,
        command: &str,
    ) -> Result<PathBuf, CommandError> {
        if !self.capabilities.allows_access(access) {
            return Err(CommandError::Failed {
                command: command.into(),
                reason: format!(
                    "file {} access is not permitted for this client",
                    access.as_str()
                ),
            });
        }
        match &self.sandbox {
            Some(sandbox) => sandbox
                .resolve(path, access)
                .map_err(|e| CommandError::Failed {
                    command: command.into(),
                    reason: e.0,
                }),
            None => {
                let p = Path::new(path);
                Ok(if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    self.base_dir.join(p)
                })
            }
        }
    }
}

/// Owns the open design, its history and command dispatch. Every front end — app, CLI,
/// JSON control channel, MCP, tests — goes through [`Session::execute`].
pub struct Session {
    state: EngineState,
    /// Where settings commands save the user settings (`None`: kept in memory only).
    user_path: Option<PathBuf>,
    undo: Vec<Design>,
    redo: Vec<Design>,
    registry: HashMap<&'static str, CommandSpec>,
}

pub const HISTORY_LIMIT: usize = 200;

impl Session {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Session {
            state: EngineState {
                design: Design::default(),
                base_dir: base_dir.into(),
                sandbox: None,
                capabilities: Capabilities::FULL,
                user: Default::default(),
            },
            user_path: None,
            undo: Vec::new(),
            redo: Vec::new(),
            registry: commands::all().into_iter().map(|c| (c.id, c)).collect(),
        }
    }

    /// Session whose file commands may read and write only under `root`. Fails closed: if
    /// `root` is not an existing directory, every file command is refused.
    pub fn sandboxed(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self::with_sandbox(
            Sandbox::new::<PathBuf, _>(&[], &[root]).unwrap_or_else(|_| Sandbox::deny_all()),
        )
    }

    /// Session whose file commands are confined to `sandbox`'s roots.
    pub fn with_sandbox(sandbox: Sandbox) -> Self {
        let mut s = Self::new(
            sandbox
                .base_dir()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        );
        s.state.sandbox = Some(sandbox);
        s
    }

    /// Loads the user settings from `path` and saves every settings change there. A missing
    /// file starts empty; an unreadable one is an error and leaves the session in memory only.
    pub fn with_user_settings(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        self.state.user = crate::user::UserSettings::load(&path)?;
        self.user_path = Some(path);
        Ok(())
    }

    pub fn user_settings(&self) -> &crate::user::UserSettings {
        &self.state.user
    }

    /// Limits what this session's client may do (read-only mode, file access off).
    pub fn set_capabilities(&mut self, capabilities: Capabilities) {
        self.state.capabilities = capabilities;
    }

    pub fn capabilities(&self) -> Capabilities {
        self.state.capabilities
    }

    /// The commands this session's client may run.
    pub fn permitted_commands(&self) -> Vec<&CommandSpec> {
        let caps = self.state.capabilities;
        self.commands()
            .into_iter()
            .filter(|c| caps.allows(c))
            .collect()
    }

    /// Confines file commands to `sandbox` (`None`: paths resolve against the base directory).
    /// Returns the previous sandbox, so a transport can scope one client's roots to its requests.
    pub fn set_sandbox(&mut self, sandbox: Option<Sandbox>) -> Option<Sandbox> {
        std::mem::replace(&mut self.state.sandbox, sandbox)
    }

    pub fn sandbox(&self) -> Option<&Sandbox> {
        self.state.sandbox.as_ref()
    }

    pub fn design(&self) -> &Design {
        &self.state.design
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn commands(&self) -> Vec<&CommandSpec> {
        let mut v: Vec<_> = self.registry.values().collect();
        v.sort_by_key(|c| c.id);
        v
    }

    /// The command `id` if it exists and this session's capabilities allow it.
    pub fn permits(&self, id: &str) -> Result<&CommandSpec, CommandError> {
        let spec = self
            .registry
            .get(id)
            .ok_or_else(|| CommandError::UnknownCommand(id.into()))?;
        if let Some(missing) = self.state.capabilities.missing(spec) {
            return Err(CommandError::Failed {
                command: id.into(),
                reason: format!(
                    "not permitted for this client (needs the '{}' capability)",
                    missing.as_str()
                ),
            });
        }
        Ok(spec)
    }

    /// Runs one command. Failures leave the document untouched; a successful edit that changes
    /// the design is one undo step.
    pub fn execute(&mut self, id: &str, params: Option<&Value>) -> CommandResult {
        let spec = self.permits(id)?;
        let p = Params::new(id, params)?;
        let (run, effect) = (spec.run, spec.effect);
        match effect {
            Effect::History => self.history(id),
            Effect::Edit => {
                let mut scratch = self.state.clone();
                let result = run(&mut scratch, &p)?;
                // Text on a path follows any edit of its path.
                scratch.design.relayout_text();
                scratch.design.dissolve_single_groups();
                if scratch.design != self.state.design {
                    self.undo
                        .push(std::mem::replace(&mut self.state.design, scratch.design));
                    if self.undo.len() > HISTORY_LIMIT {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                }
                Ok(result)
            }
            Effect::Settings => {
                let mut scratch = self.state.clone();
                let result = run(&mut scratch, &p)?;
                if scratch.user != self.state.user {
                    if let Some(path) = &self.user_path {
                        scratch.user.save(path).map_err(|e| CommandError::Failed {
                            command: id.into(),
                            reason: format!("could not save the user settings: {e}"),
                        })?;
                    }
                    self.state.user = scratch.user;
                }
                Ok(result)
            }
            Effect::Read | Effect::File => {
                let mut scratch = self.state.clone();
                run(&mut scratch, &p)
            }
        }
    }

    /// `{"id"?, "command", "params"?}` → `{"id"?, "ok": true, "result"}` or `{"id"?, "ok": false, "error"}`.
    pub fn handle(&mut self, request: &Value) -> Value {
        let mut reply = serde_json::Map::new();
        if let Some(id) = request.get("id") {
            reply.insert("id".into(), id.clone());
        }
        let Some(command) = request.get("command").and_then(Value::as_str) else {
            reply.insert("ok".into(), json!(false));
            reply.insert("error".into(), json!("request needs a string 'command'"));
            return Value::Object(reply);
        };
        match self.execute(command, request.get("params")) {
            Ok(result) => {
                reply.insert("ok".into(), json!(true));
                reply.insert("result".into(), result);
            }
            Err(e) => {
                reply.insert("ok".into(), json!(false));
                reply.insert("error".into(), json!(e.to_string()));
            }
        }
        Value::Object(reply)
    }

    fn history(&mut self, id: &str) -> CommandResult {
        let fail = |r: &str| CommandError::Failed {
            command: id.into(),
            reason: r.into(),
        };
        match id {
            "history.undo" => {
                let prev = self.undo.pop().ok_or_else(|| fail("nothing to undo"))?;
                self.redo
                    .push(std::mem::replace(&mut self.state.design, prev));
            }
            "history.redo" => {
                let next = self.redo.pop().ok_or_else(|| fail("nothing to redo"))?;
                self.undo
                    .push(std::mem::replace(&mut self.state.design, next));
            }
            _ => return Err(CommandError::UnknownCommand(id.into())),
        }
        Ok(json!({ "canUndo": self.can_undo(), "canRedo": self.can_redo() }))
    }
}
