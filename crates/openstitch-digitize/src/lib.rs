//! Image-to-embroidery conversions, written from PE-DESIGN 11's public user's guide only
//! (Auto Punch, Cross Stitch, Photo Stitch 1/2, Design Center). The algorithms are this
//! project's own and are approximations of PE-DESIGN's behaviour, not reproductions of it.
//!
//! Every conversion is deterministic for a given image and parameters, treats the image as
//! hostile (dimensions, pixel counts and output sizes are capped) and returns editable
//! [`DesignObject`](openstitch_design::DesignObject)s for the engine to add to a design.

pub mod autopunch;
pub mod center;
pub mod coverage;
pub mod cross;
pub mod palette;
pub mod photo;
pub mod raster;
pub mod regions;
pub mod screen;
pub mod trace;

pub use raster::{Frame, Raster};

/// Why a conversion failed. Messages are user-facing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DigitizeError {
    /// The bytes are not a supported image or are corrupt.
    Decode(String),
    /// The image or the requested output exceeds a limit.
    TooLarge(String),
    /// The image has nothing to convert (empty, fully transparent, all background).
    Empty(String),
}

impl std::fmt::Display for DigitizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(m) => write!(f, "cannot decode image: {m}"),
            Self::TooLarge(m) => write!(f, "too large: {m}"),
            Self::Empty(m) => write!(f, "nothing to convert: {m}"),
        }
    }
}

impl std::error::Error for DigitizeError {}

/// Most needle points one conversion may generate.
pub const MAX_STITCHES: usize = 500_000;
