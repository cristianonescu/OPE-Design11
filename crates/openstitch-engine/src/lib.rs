//! OpenStitch engine: a [`Session`] owns the open design and its undo history, and every
//! operation is a registered command taking JSON parameters. The desktop app, CLI, JSON
//! control channel, MCP server and tests all go through [`Session::execute`].

pub mod capabilities;
pub mod commands;
mod convert;
#[cfg(feature = "digitize")]
mod digitize;
mod drawing;
pub mod fonts;
#[cfg(feature = "formats")]
pub mod imports;
mod lettering;
#[cfg(feature = "library")]
mod library;
pub mod machine;
mod order;
mod ornament;
pub mod params;
mod pattern_cmds;
pub mod print;
pub mod project;
mod punch;
pub mod sandbox;
pub mod sections;
pub mod session;
mod sewing;
mod sewing_cmds;
mod shape_ops;
mod stamps;
mod stitch_edit;
pub mod templates;
#[cfg(feature = "formats")]
pub mod transfer;
pub mod user;
mod wizards;

pub use capabilities::{Capabilities, Capability};
pub use commands::{CommandSpec, Effect, ParamDoc};
pub use params::{CommandError, CommandResult, Params};
pub use sandbox::{Access, Sandbox, SandboxError};
pub use session::{EngineState, Session};
