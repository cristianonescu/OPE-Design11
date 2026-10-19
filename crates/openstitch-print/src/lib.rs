//! Printable output as PDF: actual-size and reduced prints with sewing information,
//! positioning sheets for split, multi-position and jumbo designs, and Print and Stitch.
//! Pure Rust and deterministic; see [`pdf`] for why the PDF writer is our own.

pub mod pdf;
pub mod sheets;

pub use pdf::{Image, Page, write};
pub use sheets::{Options, PAPERS, actual_size, positioning_sheets, print_and_stitch, reduced};
