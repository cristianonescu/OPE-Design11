use openstitch_design::{Rect, Vec2, v2};

/// Screen-space rectangle in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ScreenRect {
    pub fn center(&self) -> Vec2 {
        v2(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// Maps model units (0.1 mm, +y down) to screen points: `screen = offset + model × scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Screen points per model unit.
    pub scale: f64,
    /// Screen position of the model origin.
    pub offset: Vec2,
}

impl Default for Viewport {
    fn default() -> Self {
        Viewport {
            scale: 0.5,
            offset: v2(0.0, 0.0),
        }
    }
}

impl Viewport {
    /// Zoom limits: 0.02 pt per 0.1 mm (a 2 m page in ~400 pt) to 40 pt per 0.1 mm.
    pub const MIN_SCALE: f64 = 0.02;
    pub const MAX_SCALE: f64 = 40.0;

    pub fn to_screen(&self, p: Vec2) -> Vec2 {
        self.offset + p * self.scale
    }

    pub fn to_model(&self, s: Vec2) -> Vec2 {
        (s - self.offset) * (1.0 / self.scale)
    }

    /// Model units per screen point; pointer tolerances are defined in points.
    pub fn units_per_point(&self) -> f64 {
        1.0 / self.scale
    }

    /// Zooms by `factor` keeping the model point under `anchor` (screen) in place.
    pub fn zoom_about(&mut self, anchor: Vec2, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let model = self.to_model(anchor);
        self.scale = (self.scale * factor).clamp(Self::MIN_SCALE, Self::MAX_SCALE);
        self.offset = anchor - model * self.scale;
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.offset = self.offset + v2(dx, dy);
    }

    /// Fits `content` (model) centred into `screen`, leaving `margin` points on every side.
    pub fn fit(content: Rect, screen: ScreenRect, margin: f64) -> Viewport {
        let w = (screen.width - 2.0 * margin).max(1.0);
        let h = (screen.height - 2.0 * margin).max(1.0);
        let scale = (w / content.width().max(1.0))
            .min(h / content.height().max(1.0))
            .clamp(Self::MIN_SCALE, Self::MAX_SCALE);
        let offset = screen.center() - content.center() * scale;
        Viewport { scale, offset }
    }

    /// Zoom as a percentage where 100 % shows 1 mm as 1 point × `points_per_mm`.
    pub fn zoom_percent(&self, points_per_mm: f64) -> f64 {
        self.scale * 10.0 / points_per_mm * 100.0
    }
}
