//! Pure-Rust rendering of OpenStitch designs with tiny-skia: PNG previews of a compiled design
//! (used by the MCP `preview` tool) and file-manager thumbnails of `.osd` projects and machine
//! embroidery files (the `openstitch-thumbnailer` binary).

pub mod preview;
pub mod thumbnail;
