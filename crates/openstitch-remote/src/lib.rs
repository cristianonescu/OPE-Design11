//! Loopback socket transport for the running editor: external tools (`stitchtool connect`,
//! `openstitch-mcp --live`) send the same JSON requests as `stitchtool serve` to the app's
//! [`Session`](openstitch_engine::Session).
//!
//! Security model:
//! - the listener binds `127.0.0.1` only (OS-assigned or configured port) and drops peers that
//!   are not loopback; the client refuses non-loopback addresses, so the token never leaves the
//!   machine;
//! - every request carries a random 256-bit per-launch token, compared in constant time; a
//!   wrong or missing token closes the connection. The token is written with the address to an
//!   endpoint file in the OS config directory that only the user can read (mode 0600 on Unix;
//!   on Windows `%APPDATA%` is private to the user), and removed when the server stops;
//! - each client gets the configured [`Capabilities`](openstitch_engine::Capabilities)
//!   (read-only, file access off) and a [`Sandbox`](openstitch_engine::Sandbox) of read/write
//!   roots (none by default, so every file command is refused);
//! - requests are limited in size, idle connections time out, and so does waiting for the UI;
//! - requests are executed on the UI thread: [`Server::try_next`] hands them over one at a time,
//!   so undo/redo stays coherent with the user's own edits.
//!
//! Wire protocol: one JSON object per line each way.
//! `{"token", "id"?, "command", "params"?}` → `{"id"?, "ok": true, "result"}` or
//! `{"id"?, "ok": false, "error"}`.

mod client;
mod endpoint;
mod server;

pub use client::Client;
pub use endpoint::{Endpoint, default_endpoint_file};
pub use server::{Job, Policy, Saved, Server, ServerConfig};

/// Largest request line the server accepts by default, in bytes.
pub const DEFAULT_MAX_REQUEST_BYTES: usize = 1 << 20;
/// Largest reply line a client accepts, in bytes.
pub const MAX_REPLY_BYTES: usize = 64 << 20;
