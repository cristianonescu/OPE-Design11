//! Model Context Protocol server over the OpenStitch command engine: newline-delimited
//! JSON-RPC 2.0 on stdio. Tools are generated from the engine's command registry, plus
//! `run_batch` and `preview`; file access is confined by the engine's [`Sandbox`](openstitch_engine::Sandbox).

pub mod line_reader;
pub mod server;
pub mod tools;

/// The PNG renderer, shared with the thumbnailer (`openstitch-render`).
pub use openstitch_render::preview;

pub use line_reader::{Line, LineReader};
pub use server::{Config, PROTOCOL_VERSIONS, Server};
pub use tools::{Tool, ToolKind, tool_name};
