use openstitch_design::{Rect, Vec2, v2};

/// The eight scale handles around a selection plus the rotate handle above it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Rotate,
}

impl Handle {
    pub const SCALE: [Handle; 8] = [
        Handle::TopLeft,
        Handle::Top,
        Handle::TopRight,
        Handle::Right,
        Handle::BottomRight,
        Handle::Bottom,
        Handle::BottomLeft,
        Handle::Left,
    ];

    /// Position on the unit box: (0, 0) top-left … (1, 1) bottom-right.
    fn unit(self) -> (f64, f64) {
        match self {
            Handle::TopLeft => (0.0, 0.0),
            Handle::Top | Handle::Rotate => (0.5, 0.0),
            Handle::TopRight => (1.0, 0.0),
            Handle::Right => (1.0, 0.5),
            Handle::BottomRight => (1.0, 1.0),
            Handle::Bottom => (0.5, 1.0),
            Handle::BottomLeft => (0.0, 1.0),
            Handle::Left => (0.0, 0.5),
        }
    }

    pub fn scales_x(self) -> bool {
        !matches!(self, Handle::Top | Handle::Bottom | Handle::Rotate)
    }

    pub fn scales_y(self) -> bool {
        !matches!(self, Handle::Left | Handle::Right | Handle::Rotate)
    }

    pub fn is_corner(self) -> bool {
        self.scales_x() && self.scales_y()
    }

    /// The handle that stays fixed while this one is dragged.
    pub fn opposite(self) -> Handle {
        match self {
            Handle::TopLeft => Handle::BottomRight,
            Handle::Top => Handle::Bottom,
            Handle::TopRight => Handle::BottomLeft,
            Handle::Right => Handle::Left,
            Handle::BottomRight => Handle::TopLeft,
            Handle::Bottom => Handle::Top,
            Handle::BottomLeft => Handle::TopRight,
            Handle::Left => Handle::Right,
            Handle::Rotate => Handle::Rotate,
        }
    }
}

/// Scale factors and the point they keep fixed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub sx: f64,
    pub sy: f64,
    pub origin: Vec2,
}

/// Handle layout for a selection box. Handles have a fixed size on screen, so the layout takes
/// the current model units per screen point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionHandles {
    pub bounds: Rect,
    pub units_per_point: f64,
}

impl SelectionHandles {
    /// Handle radius and rotate-handle distance above the box, in points.
    pub const RADIUS: f64 = 5.0;
    pub const ROTATE_OFFSET: f64 = 22.0;

    pub fn position(&self, h: Handle) -> Vec2 {
        let b = &self.bounds;
        if h == Handle::Rotate {
            return v2(
                b.center().x,
                b.min_y - Self::ROTATE_OFFSET * self.units_per_point,
            );
        }
        let (ux, uy) = h.unit();
        v2(b.min_x + ux * b.width(), b.min_y + uy * b.height())
    }

    /// The handle under `p`; the rotate handle wins over overlapping scale handles.
    pub fn handle_at(&self, p: Vec2) -> Option<Handle> {
        let r = Self::RADIUS * self.units_per_point * 1.4;
        std::iter::once(Handle::Rotate)
            .chain(Handle::SCALE)
            .find(|&h| self.position(h).distance(p) <= r)
    }

    /// Scale for dragging `handle` from `start` to `p`. `proportional` keeps the aspect ratio
    /// (corner handles). Factors are clamped to what `object.scale` accepts; dragging past the
    /// fixed point does not mirror.
    pub fn scale(&self, handle: Handle, start: Vec2, p: Vec2, proportional: bool) -> Scale {
        let o = self.position(handle.opposite());
        let factor = |now: f64, was: f64, origin: f64| {
            let d = was - origin;
            if d.abs() < 1e-9 {
                1.0
            } else {
                ((now - origin) / d).clamp(0.01, 100.0)
            }
        };
        let mut sx = if handle.scales_x() {
            factor(p.x, start.x, o.x)
        } else {
            1.0
        };
        let mut sy = if handle.scales_y() {
            factor(p.y, start.y, o.y)
        } else {
            1.0
        };
        if proportional && handle.is_corner() {
            let s = sx.max(sy);
            sx = s;
            sy = s;
        }
        Scale { sx, sy, origin: o }
    }

    /// Clockwise degrees for dragging from `start` to `p` around the box centre; `snap` rounds
    /// to 15° steps.
    pub fn rotation(&self, start: Vec2, p: Vec2, snap: bool) -> f64 {
        let c = self.bounds.center();
        let a0 = (start.y - c.y).atan2(start.x - c.x);
        let a1 = (p.y - c.y).atan2(p.x - c.x);
        let mut deg = (a1 - a0).to_degrees();
        if deg > 180.0 {
            deg -= 360.0;
        } else if deg < -180.0 {
            deg += 360.0;
        }
        if snap {
            (deg / 15.0).round() * 15.0
        } else {
            deg
        }
    }
}
