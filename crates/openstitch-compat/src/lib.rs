//! OpenStitch's reference compatibility harness (docs/COMPATIBILITY_TESTS.md).
//!
//! - [`spec`]: declarative, versioned test specs (`compat/specs/<area>/<id>.json`): what
//!   they cover, the evidence, a `stitchtool exec` script, the exports and the measurements.
//! - [`compare`]: semantic comparison of two embroidery files or stitch patterns.
//! - [`measure`]: the measurements specs use (stitch counts, lengths, angles, edges, travel,
//!   area, PES objects, threads, metadata…).
//! - [`runner`]: runs specs headlessly and checks each measurement at each evidence tier.
//! - [`provenance`]: PE-DESIGN reference files and sew-out records, with their provenance.
//! - [`report`]: the JSON report, the markdown summary and the generated status table.
//! - [`cli`]: `stitchtool compat`.

#![forbid(unsafe_code)]

pub mod cli;
pub mod compare;
pub mod measure;
pub mod provenance;
pub mod report;
pub mod runner;
pub mod sha256;
pub mod spec;
pub mod tolerance;
pub mod trace;

pub use compare::{CompareOptions, Comparison, Difference, compare_files, compare_patterns};
pub use report::{Report, Status};
pub use runner::{Config, run};
pub use spec::{Spec, Tier};
