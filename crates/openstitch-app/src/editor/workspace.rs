//! Workspace aids that never change the design: grid settings, ruler ticks, the measure tool,
//! the Reference window's mapping and the realistic thread shading. Model units are 0.1 mm.

use super::viewport::{ScreenRect, Viewport};
use openstitch_design::{Rect, Vec2, v2};

/// Grid drawn on the Design Page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSettings {
    pub visible: bool,
    /// Line spacing, mm.
    pub spacing_mm: f64,
    /// Every n-th line is darker.
    pub major_every: u32,
}

impl Default for GridSettings {
    fn default() -> Self {
        GridSettings {
            visible: true,
            spacing_mm: 10.0,
            major_every: 5,
        }
    }
}

impl GridSettings {
    pub const SPACINGS_MM: [f64; 6] = [1.0, 2.0, 5.0, 10.0, 20.0, 25.4];

    /// Grid line positions along one axis of a page `length` model units long, with whether
    /// each is a major line. Empty when lines would be closer than `min_points` on screen.
    pub fn lines(&self, length: f64, scale: f64, min_points: f64) -> Vec<(f64, bool)> {
        let step = self.spacing_mm * 10.0;
        if !self.visible || step <= 0.0 || step * scale < min_points {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut k = 1u32;
        while f64::from(k) * step < length {
            out.push((
                f64::from(k) * step,
                self.major_every > 0 && k.is_multiple_of(self.major_every),
            ));
            k += 1;
        }
        out
    }
}

/// A ruler tick at model coordinate `at`; major ticks carry their label in mm.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub at: f64,
    pub major: bool,
    pub label: Option<String>,
}

/// Ticks for a ruler showing model coordinates `from..to` at `scale` points per model unit.
/// The minor step is the smallest of 1, 2, 5 × 10ⁿ mm at least `min_points` apart; major,
/// labelled ticks fall on multiples of 10 × 10ⁿ mm.
pub fn ruler_ticks(from: f64, to: f64, scale: f64, min_points: f64) -> Vec<Tick> {
    if !(scale > 0.0 && from.is_finite() && to.is_finite() && to > from) {
        return Vec::new();
    }
    let Some((step_mm, major_every)) = (-1..5)
        .flat_map(|e| [1, 2, 5].map(|m| (f64::from(m) * 10f64.powi(e), 10 / m)))
        .find(|&(mm, _)| mm * 10.0 * scale >= min_points)
    else {
        return Vec::new();
    };
    let step = step_mm * 10.0;
    let first = (from / step).ceil() as i64;
    let last = (to / step).floor() as i64;
    if last - first > 10_000 {
        return Vec::new();
    }
    (first..=last)
        .map(|k| {
            let at = k as f64 * step;
            let major = k.rem_euclid(i64::from(major_every)) == 0;
            Tick {
                at,
                major,
                label: major.then(|| format_mm(at / 10.0)),
            }
        })
        .collect()
}

fn format_mm(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.1}")
    }
}

/// Measure tool readout: the same numbers as the engine's `design.measure`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    pub from: Vec2,
    pub to: Vec2,
}

impl Measurement {
    pub fn distance_mm(&self) -> f64 {
        self.from.distance(self.to) / 10.0
    }

    pub fn dx_mm(&self) -> f64 {
        (self.to.x - self.from.x) / 10.0
    }

    pub fn dy_mm(&self) -> f64 {
        (self.to.y - self.from.y) / 10.0
    }

    /// Degrees counter-clockwise from +x as seen on screen (model y points down).
    pub fn angle_degrees(&self) -> f64 {
        let (dx, dy) = (self.dx_mm(), self.dy_mm());
        if dx == 0.0 && dy == 0.0 {
            0.0
        } else {
            (-dy).atan2(dx).to_degrees()
        }
    }

    pub fn label(&self) -> String {
        format!(
            "{:.1} mm   Δx {:.1}  Δy {:.1}   {:.1}°",
            self.distance_mm(),
            self.dx_mm(),
            self.dy_mm(),
            self.angle_degrees()
        )
    }
}

/// Measure tool: press to anchor, drag to measure; the last measurement stays until the next.
/// Shift snaps the angle to 45° steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeasureTool {
    pub active: bool,
    pub current: Option<Measurement>,
    dragging: bool,
}

impl MeasureTool {
    pub fn press(&mut self, at: Vec2) {
        self.current = Some(Measurement { from: at, to: at });
        self.dragging = true;
    }

    pub fn drag(&mut self, at: Vec2, snap: bool) {
        if !self.dragging {
            return;
        }
        if let Some(m) = &mut self.current {
            m.to = if snap { snap_45(m.from, at) } else { at };
        }
    }

    pub fn release(&mut self, at: Vec2, snap: bool) {
        self.drag(at, snap);
        self.dragging = false;
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub fn clear(&mut self) {
        *self = MeasureTool {
            active: self.active,
            ..Default::default()
        };
    }
}

fn snap_45(from: Vec2, to: Vec2) -> Vec2 {
    let d = to - from;
    let len = d.length();
    let step = std::f64::consts::FRAC_PI_4;
    let a = (d.y.atan2(d.x) / step).round() * step;
    from + v2(a.cos(), a.sin()) * len
}

/// The Reference window: a thumbnail of the whole Design Page showing which part the canvas
/// shows; clicking or dragging in it centres the canvas there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceMap {
    /// Thumbnail of the page, fitted inside the window area.
    pub thumb: Viewport,
}

impl ReferenceMap {
    pub fn new(page: Rect, area: ScreenRect) -> Self {
        ReferenceMap {
            thumb: Viewport::fit(page, area, 4.0),
        }
    }

    /// The part of the model the canvas viewport `vp` shows on `screen`, as a rectangle in
    /// thumbnail coordinates.
    pub fn visible(&self, vp: &Viewport, screen: ScreenRect) -> ScreenRect {
        let a = self.thumb.to_screen(vp.to_model(v2(screen.x, screen.y)));
        let b = self
            .thumb
            .to_screen(vp.to_model(v2(screen.x + screen.width, screen.y + screen.height)));
        ScreenRect {
            x: a.x,
            y: a.y,
            width: b.x - a.x,
            height: b.y - a.y,
        }
    }

    /// Pans `vp` so the model point under thumbnail point `at` is in the middle of `screen`.
    pub fn center_on(&self, vp: &mut Viewport, screen: ScreenRect, at: Vec2) {
        let target = self.thumb.to_model(at);
        let now = vp.to_screen(target);
        let c = screen.center();
        vp.pan(c.x - now.x, c.y - now.y);
    }
}

/// Shades for drawing a thread realistically: a darker edge, the body, and a highlight along
/// the middle, as on a twisted, slightly glossy thread.
pub fn thread_shades(rgb: [u8; 3]) -> [[u8; 3]; 3] {
    let scale = |k: f64| rgb.map(|c| (f64::from(c) * k).round().clamp(0.0, 255.0) as u8);
    let lift = |k: f64| rgb.map(|c| (f64::from(c) + (255.0 - f64::from(c)) * k).round() as u8);
    [scale(0.55), rgb, lift(0.45)]
}

/// Real thread width on screen: 40-weight embroidery thread lies about 0.4 mm wide.
pub const THREAD_WIDTH_MM: f64 = 0.4;

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_engine::Session;
    use serde_json::json;

    fn page(w: f64, h: f64) -> Rect {
        Rect {
            min_x: 0.0,
            min_y: 0.0,
            max_x: w,
            max_y: h,
        }
    }

    #[test]
    fn grid_lines_follow_spacing_and_hide_when_dense() {
        let g = GridSettings::default();
        let lines = g.lines(1000.0, 1.0, 6.0);
        assert_eq!(lines.len(), 9);
        assert_eq!(lines[4], (500.0, true));
        assert!(!lines[0].1);
        let fine = GridSettings {
            spacing_mm: 1.0,
            ..g
        };
        assert!(fine.lines(1000.0, 0.2, 6.0).is_empty(), "2 points apart");
        assert_eq!(fine.lines(1000.0, 1.0, 6.0).len(), 99);
        let hidden = GridSettings {
            visible: false,
            ..g
        };
        assert!(hidden.lines(1000.0, 1.0, 6.0).is_empty());
    }

    #[test]
    fn ruler_ticks_pick_readable_steps_with_labels() {
        // 1 point per 0.1 mm: 1 mm minor ticks would be 10 points apart.
        let t = ruler_ticks(0.0, 500.0, 1.0, 6.0);
        assert_eq!(t[1].at, 10.0);
        let majors: Vec<_> = t.iter().filter(|t| t.major).collect();
        assert_eq!(majors[1].at, 100.0);
        assert_eq!(majors[1].label.as_deref(), Some("10"));
        // Zoomed out: 0.05 points per 0.1 mm → 2 mm would be 1 point; 20 mm is 10 points.
        let t = ruler_ticks(-1000.0, 5000.0, 0.05, 6.0);
        assert_eq!(t[1].at - t[0].at, 200.0);
        assert!(
            t.iter()
                .any(|t| t.major && t.label.as_deref() == Some("-100"))
        );
        assert!(ruler_ticks(10.0, 0.0, 1.0, 6.0).is_empty());
        assert!(ruler_ticks(0.0, 10.0, f64::NAN, 6.0).is_empty());
    }

    #[test]
    fn measurement_matches_the_engine() {
        let m = Measurement {
            from: v2(100.0, 400.0),
            to: v2(400.0, 0.0),
        };
        let mut s = Session::new(std::env::temp_dir());
        let r = s
            .execute(
                "design.measure",
                Some(&json!({"from": [10, 40], "to": [40, 0]})),
            )
            .expect("measure");
        assert_eq!(r["distanceMM"].as_f64(), Some(m.distance_mm()));
        assert_eq!(r["dyMM"].as_f64(), Some(m.dy_mm()));
        assert_eq!(
            r["angleDegrees"].as_f64(),
            Some((m.angle_degrees() * 100.0).round() / 100.0)
        );
        assert!(m.label().starts_with("50.0 mm"));
    }

    #[test]
    fn measure_tool_drags_and_snaps() {
        let mut t = MeasureTool {
            active: true,
            ..Default::default()
        };
        t.drag(v2(5.0, 5.0), false);
        assert_eq!(t.current, None, "no drag before a press");
        t.press(v2(0.0, 0.0));
        t.drag(v2(100.0, 10.0), false);
        assert!(t.is_dragging());
        t.release(v2(100.0, 90.0), true);
        let m = t.current.expect("measurement");
        assert!(
            (m.angle_degrees() + 45.0).abs() < 1e-9,
            "{}",
            m.angle_degrees()
        );
        t.drag(v2(1.0, 1.0), false);
        assert_eq!(t.current, Some(m), "released: no more changes");
        t.clear();
        assert!(t.active && t.current.is_none());
    }

    #[test]
    fn reference_map_shows_and_moves_the_visible_area() {
        let screen = ScreenRect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let mut vp = Viewport::fit(page(1000.0, 1000.0), screen, 0.0);
        vp.zoom_about(screen.center(), 4.0);
        let thumb_area = ScreenRect {
            x: 0.0,
            y: 0.0,
            width: 108.0,
            height: 108.0,
        };
        let map = ReferenceMap::new(page(1000.0, 1000.0), thumb_area);
        let v = map.visible(&vp, screen);
        // A quarter of the fitted view, around the page centre.
        assert!((v.x + v.width / 2.0 - 54.0).abs() < 1e-6);
        assert!(v.width < 50.0 && v.height < 40.0);
        map.center_on(&mut vp, screen, v2(4.0 + 10.0, 4.0 + 10.0));
        let c = vp.to_model(screen.center());
        assert!(
            (c.x - 100.0).abs() < 1e-6 && (c.y - 100.0).abs() < 1e-6,
            "{c:?}"
        );
    }

    #[test]
    fn thread_shades_are_dark_body_light() {
        let [d, b, l] = thread_shades([200, 40, 40]);
        assert_eq!(b, [200, 40, 40]);
        assert!(d.iter().zip(b).all(|(x, y)| *x <= y));
        assert!(l.iter().zip(b).all(|(x, y)| *x >= y));
    }
}
