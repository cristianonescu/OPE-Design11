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
pub mod perceptual;
pub mod photo;
pub mod raster;
pub mod regions;
pub mod screen;
pub mod trace;

pub use raster::{Frame, Raster};

/// How a wizard converts: as PE-DESIGN 11.50 does (the default), or OpenStitch's improved
/// method (docs/research/beyond/image-digitizing.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    PeDesign,
    OpenStitch,
}

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

#[cfg(test)]
mod tests {
    /// The crate's own code calls no platform transcendental function: every such call goes
    /// through `openstitch_design::fmath::Det`, so an image converts to the same stitches on
    /// macOS, Windows and Linux (the platforms' C math libraries differ in the last bit, see
    /// `openstitch_design::fmath`). Tests may compare against the platform's functions.
    #[test]
    fn the_crate_uses_no_platform_transcendental_functions() {
        let names = [
            "sin", "cos", "sin_cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh",
            "tanh", "exp", "exp2", "exp_m1", "ln", "log", "log2", "log10", "ln_1p", "powf",
            "hypot", "cbrt",
        ];
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("src") {
            let path = entry.expect("entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.ends_with(".rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            // Code before the file's own test module only.
            let code = text.split("#[cfg(test)]").next().unwrap_or("");
            for (n, line) in code.lines().enumerate() {
                let line = line.split("//").next().unwrap_or("");
                for f in names {
                    if line.contains(&format!(".{f}(")) {
                        found.push(format!("{name}:{}: .{f}(", n + 1));
                    }
                }
            }
        }
        assert!(found.is_empty(), "platform math calls: {found:?}");
    }
}
