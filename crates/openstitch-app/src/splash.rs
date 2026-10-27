//! The startup splash: the OpenStitch wordmark over the window for about a second, then a fade.
//! A click or a key skips it. It only appears on an interactive start
//! ([`crate::app::Launch::splash_allowed`]), and Options ▸ "Show splash at startup" or
//! `--no-splash` turn it off.

use eframe::egui;

/// Seconds the splash stays fully visible, then fades out.
const HOLD: f64 = 0.7;
const FADE: f64 = 0.3;

#[derive(Debug, Default)]
pub struct Splash {
    /// Frames drawn so far; the clock starts at [`START_FRAME`].
    frames: u32,
    /// `egui` time the clock started.
    start: Option<f64>,
}

/// The first frames come before the window is on screen and are slow (fonts, textures), so
/// the splash stays fully visible until this frame and only then starts its second.
const START_FRAME: u32 = 3;

impl Splash {
    /// Opacity `t` seconds after the splash appeared; `None` once it is over.
    fn opacity(t: f64) -> Option<f32> {
        if t < HOLD {
            Some(1.0)
        } else if t < HOLD + FADE {
            Some((1.0 - (t - HOLD) / FADE) as f32)
        } else {
            None
        }
    }

    /// Draws the splash over everything; `false` once it is over or skipped. A key press that
    /// skips it is taken out of the frame's input, so it doesn't also run a shortcut.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let now = ctx.input(|i| i.time);
        self.frames += 1;
        let start = if self.frames < START_FRAME {
            now
        } else {
            *self.start.get_or_insert(now)
        };
        let skipped = ctx.input_mut(|i| {
            let pressed = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. }
                        | egui::Event::PointerButton { pressed: true, .. }
                )
            });
            if pressed {
                i.events
                    .retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_)));
            }
            pressed
        });
        let Some(alpha) = Self::opacity(now - start).filter(|_| !skipped) else {
            return false;
        };
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("openstitch-splash"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                // Swallows the pointer, so the click that skips the splash doesn't reach the page.
                ui.allocate_rect(screen, egui::Sense::click());
                let dark = ui.visuals().dark_mode;
                let painter = ui.painter();
                painter.rect_filled(screen, 0, ui.visuals().panel_fill.gamma_multiply(alpha));
                let w = (screen.width() * 0.4).clamp(240.0, 560.0);
                let (name, src) = if dark {
                    ("wordmark-dark", crate::icons::WORDMARK_DARK)
                } else {
                    ("wordmark", crate::icons::WORDMARK)
                };
                let px = (w * ctx.pixels_per_point()).round() as u32;
                if let Some(t) = crate::icons::logo_texture(ctx, name, src, px) {
                    let size = t.size_vec2() * (w / t.size_vec2().x);
                    let r = egui::Rect::from_center_size(screen.center(), size);
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    painter.image(t.id(), r, uv, egui::Color32::WHITE.gamma_multiply(alpha));
                    painter.text(
                        egui::pos2(screen.center().x, r.max.y + 18.0),
                        egui::Align2::CENTER_TOP,
                        format!("Version {}", env!("CARGO_PKG_VERSION")),
                        egui::FontId::proportional(13.0),
                        ui.visuals().weak_text_color().gamma_multiply(alpha),
                    );
                }
            });
        ctx.request_repaint();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_then_fades_out_within_a_second() {
        assert_eq!(Splash::opacity(0.0), Some(1.0));
        assert_eq!(Splash::opacity(0.69), Some(1.0));
        let mid = Splash::opacity(0.85).expect("fading");
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(Splash::opacity(1.0), None);
    }
}
