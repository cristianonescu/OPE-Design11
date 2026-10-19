//! Editable embroidery designs: geometry, design objects with sewing attributes,
//! stitch generators and the compiler from [`Design`] to a machine `StitchPattern`.
//! Pure computation: no I/O, deterministic output.

pub mod boolean;
pub mod column;
pub mod compile;
pub mod direction;
pub mod fonts;
pub mod generate;
pub mod geometry;
pub mod lines;
pub mod machine;
pub mod model;
pub mod monogram;
pub mod ornament;
pub mod paths;
pub mod patterns;
pub mod project;
mod provenance;
pub mod region;
pub mod regions;
pub mod route;
pub mod sections;
pub mod sewing;
pub mod shapes;
pub(crate) mod shaping;
pub mod skeleton;
pub mod stamp;
pub mod stitch_edit;
pub mod text;

pub use compile::Compiler;
pub use fonts::{DesignFont, FontBytes, FontData};
pub use geometry::{Bezier, Contour, Rect, Transform, Vec2, v2};
pub use machine::{
    BROTHER_HOOPS, Estimate, Hoop, MachineSettings, MachineType, SewingArea, UserHoop, estimate,
};
pub use model::{
    Arrange, BackgroundImage, ChainShape, Design, DesignObject, DesignPage, Feather, FeatherSide,
    Guides, ImageAdjust, LineSewing, LineType, ObjectKind, RegionSewing, RegionType, StitchBlock, UnderlayStyle,
};
pub use text::{CharEdit, FONTS, FontInfo, Layout, TextAlign, TextDirection, TextError, TextSpec};

#[cfg(test)]
mod stitch_type_tests;
#[cfg(test)]
mod tests;
