//! Design canvas: draws the Design Page and stitches, feeds pointer events to the tool
//! controller and handles zoom and pan.

use crate::app::EditorApp;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Sense, Shape, Stroke, StrokeKind};
use openstitch_app::editor::hit::bounds_of;
use openstitch_app::editor::points::handle_lines;
use openstitch_app::editor::{
    Handle, Phase, PointerEvent, ScreenRect, SelectionHandles, ToolPreview, Viewport,
};
use openstitch_design::{Contour, ObjectKind, Rect, Transform, Vec2, v2};

const BACKGROUND: Color32 = Color32::from_rgb(0x3a, 0x3d, 0x42);
const PAGE: Color32 = Color32::from_rgb(0xf6, 0xf3, 0xec);
const ACCENT: Color32 = Color32::from_rgb(0x1e, 0x88, 0xe5);

fn pos(p: Vec2) -> Pos2 {
    Pos2::new(p.x as f32, p.y as f32)
}

fn model_rect(v: &Viewport, r: &Rect) -> egui::Rect {
    egui::Rect::from_two_pos(
        pos(v.to_screen(v2(r.min_x, r.min_y))),
        pos(v.to_screen(v2(r.max_x, r.max_y))),
    )
}

pub fn canvas(app: &mut EditorApp, ui: &mut egui::Ui) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let screen = ScreenRect {
        x: f64::from(rect.min.x),
        y: f64::from(rect.min.y),
        width: f64::from(rect.width()),
        height: f64::from(rect.height()),
    };
    let page = app.session.design().page;
    let page_rect = Rect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: page.width,
        max_y: page.height,
    };
    if app.viewport.is_none() || app.fit_requested {
        let mut fitted = Viewport::fit(page_rect, screen, 24.0);
        if let Some(z) = app.initial_zoom.take() {
            fitted.zoom_about(screen.center(), z);
        }
        app.viewport = Some(fitted);
        app.fit_requested = false;
    }
    let mut vp = app.viewport.unwrap_or_default();

    navigate(ui, &response, rect, &mut vp);
    app.viewport = Some(vp);
    pointer(app, ui, &response, vp);
    app.ensure_render_model();

    app.workspace.canvas = Some(rect);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, BACKGROUND);
    draw_page(&painter, &vp, &page_rect);
    draw_background(app, ui.ctx(), &painter, &vp);
    crate::workspace_ui::draw_grid(&painter, &vp, &page_rect, &app.workspace.grid);
    if app.workspace.realistic {
        crate::workspace_ui::draw_realistic(app, &painter, &vp);
        draw_travel(app, &painter, &vp);
    } else {
        draw_stitches(app, &painter, &vp);
    }
    draw_unsewn_outlines(app, &painter, &vp);
    draw_selection(app, &painter, &vp);
    draw_preview(app, &painter, &vp);
    draw_points(app, &painter, &vp);
    crate::edit_ui::draw_arrange_preview(app, &painter, &vp);
    crate::edit_ui::draw_markers(app, &painter, &vp);
    crate::sections_ui::draw_sections(app, &painter, &vp);
    crate::workspace_ui::draw_measure(&painter, &vp, &app.workspace.measure);
    if let (Some(_), Some(n)) = (app.simulate, app.render.needle) {
        let c = pos(vp.to_screen(n));
        painter.circle_stroke(
            c,
            6.0,
            Stroke::new(2.0, Color32::from_rgb(0xff, 0x3d, 0x00)),
        );
        painter.line_segment(
            [c - egui::vec2(10.0, 0.0), c + egui::vec2(10.0, 0.0)],
            Stroke::new(1.0, Color32::from_rgb(0xff, 0x3d, 0x00)),
        );
        painter.line_segment(
            [c - egui::vec2(0.0, 10.0), c + egui::vec2(0.0, 10.0)],
            Stroke::new(1.0, Color32::from_rgb(0xff, 0x3d, 0x00)),
        );
    }
    if let Some(h) = response.hover_pos() {
        let m = vp.to_model(v2(f64::from(h.x), f64::from(h.y)));
        let text = format!(
            "{:.1}, {:.1} mm   {:.0}%",
            m.x / 10.0,
            m.y / 10.0,
            vp.zoom_percent(96.0 / 25.4)
        );
        painter.text(
            rect.right_bottom() + egui::vec2(-8.0, -6.0),
            egui::Align2::RIGHT_BOTTOM,
            text,
            egui::FontId::monospace(11.0),
            Color32::from_gray(220),
        );
    }
    if app.workspace.rulers {
        crate::workspace_ui::draw_rulers(&painter, rect, &vp, response.hover_pos());
    }
    let cursor = match app.tools.tool() {
        _ if app.workspace.measure.active => egui::CursorIcon::Crosshair,
        openstitch_app::editor::Tool::Select | openstitch_app::editor::Tool::Point => {
            egui::CursorIcon::Default
        }
        _ => egui::CursorIcon::Crosshair,
    };
    if response.hovered() {
        ui.ctx().set_cursor_icon(cursor);
    }
}

/// Scroll pans, pinch / Ctrl+scroll zooms at the pointer, middle-drag or Space+drag pans.
fn navigate(ui: &egui::Ui, response: &egui::Response, rect: egui::Rect, vp: &mut Viewport) {
    if let Some(h) = response.hover_pos() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta(), i.zoom_delta()));
        if zoom != 1.0 {
            vp.zoom_about(v2(f64::from(h.x), f64::from(h.y)), f64::from(zoom));
        } else if scroll != egui::Vec2::ZERO {
            vp.pan(f64::from(scroll.x), f64::from(scroll.y));
        }
    }
    let space = ui.input(|i| i.key_down(egui::Key::Space));
    if response.dragged_by(egui::PointerButton::Middle)
        || (space && response.dragged_by(egui::PointerButton::Primary))
    {
        let d = response.drag_delta();
        vp.pan(f64::from(d.x), f64::from(d.y));
    }
    let _ = rect;
}

fn pointer(app: &mut EditorApp, ui: &egui::Ui, response: &egui::Response, vp: Viewport) {
    if ui.input(|i| i.key_down(egui::Key::Space)) {
        return;
    }
    let (pressed, down, released, at, shift, moved) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.pointer.interact_pos().or(i.pointer.hover_pos()),
            i.modifiers.shift,
            // The first move after the pointer enters has no delta; the event still counts.
            i.pointer.delta() != egui::Vec2::ZERO
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::PointerMoved(_))),
        )
    });
    let Some(at) = at else { return };
    let mut clicks = 1;
    let phase = if pressed && response.hovered() {
        app.canvas_pointer_captured = true;
        let time = ui.input(|i| i.time);
        clicks = app.clicks.press(time, v2(f64::from(at.x), f64::from(at.y)));
        Phase::Down
    } else if app.canvas_pointer_captured && released {
        app.canvas_pointer_captured = false;
        Phase::Up
    } else if app.canvas_pointer_captured && down && moved {
        Phase::Dragged
    } else if !down && moved && response.hovered() {
        Phase::Moved
    } else {
        if response.double_clicked() {
            let effects = app.tools.finish();
            app.apply_effects(effects);
        }
        return;
    };
    let model = vp.to_model(v2(f64::from(at.x), f64::from(at.y)));
    if app.workspace.measure.active {
        let m = &mut app.workspace.measure;
        match phase {
            Phase::Down => m.press(model),
            Phase::Dragged => m.drag(model, shift),
            Phase::Up => m.release(model, shift),
            Phase::Moved => {}
        }
        return;
    }
    if matches!(phase, Phase::Down) && crate::edit_ui::pick(app, model) {
        return;
    }
    if crate::edit_ui::drag(app, phase, model) {
        return;
    }
    let mut e = PointerEvent::new(phase, model);
    e.shift = shift;
    e.clicks = clicks;
    app.with_tools(|t, ctx| t.handle(e, ctx));
}

fn draw_page(painter: &egui::Painter, vp: &Viewport, page: &Rect) {
    let r = model_rect(vp, page);
    painter.rect_filled(
        r.translate(egui::vec2(3.0, 4.0)),
        CornerRadius::ZERO,
        Color32::from_black_alpha(70),
    );
    painter.rect_filled(r, CornerRadius::ZERO, PAGE);
    painter.rect_stroke(
        r,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_gray(150)),
        StrokeKind::Outside,
    );
}

/// The background reference image, decoded once per path (size limits as for the wizard).
fn draw_background(
    app: &mut EditorApp,
    ctx: &egui::Context,
    painter: &egui::Painter,
    vp: &Viewport,
) {
    let Some(b) = app.session.design().background.clone() else {
        return;
    };
    if !b.visible || b.opacity <= 0.0 {
        return;
    }
    if app
        .background_texture
        .as_ref()
        .is_none_or(|(p, _)| *p != b.path)
    {
        let texture = std::fs::read(&b.path)
            .ok()
            .and_then(|data| openstitch_digitize::Raster::decode(&data).ok())
            .map(|r| {
                let size = [r.width() as usize, r.height() as usize];
                let mut rgba = Vec::with_capacity(size[0] * size[1] * 4);
                for y in 0..r.height() {
                    for x in 0..r.width() {
                        rgba.extend_from_slice(&r.pixel(x, y));
                    }
                }
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                ctx.load_texture("background", image, egui::TextureOptions::LINEAR)
            });
        app.background_texture = Some((b.path.clone(), texture));
    }
    if let Some((_, Some(texture))) = &app.background_texture {
        let r = model_rect(
            vp,
            &Rect {
                min_x: b.x,
                min_y: b.y,
                max_x: b.x + b.width,
                max_y: b.y + b.height,
            },
        );
        let uv = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let tint = Color32::from_white_alpha((b.opacity.clamp(0.0, 1.0) * 255.0) as u8);
        painter.image(texture.id(), r, uv, tint);
    }
}

fn draw_stitches(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    // Thinner than real thread (~0.4 mm) so rows stay distinguishable.
    let width = (2.5 * vp.scale).clamp(0.5, 3.0) as f32;
    for layer in &app.render.layers {
        let [r, g, b] = layer.rgb;
        let stroke = Stroke::new(width, Color32::from_rgb(r, g, b));
        for s in &layer.strokes {
            let pts: Vec<Pos2> = s.iter().map(|&p| pos(vp.to_screen(p))).collect();
            painter.add(Shape::line(pts, stroke));
        }
    }
    draw_travel(app, painter, vp);
}

fn draw_travel(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    if app.show_travel {
        let stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(90, 90, 90, 110));
        for &(a, b) in &app.render.travel {
            painter.add(Shape::dashed_line(
                &[pos(vp.to_screen(a)), pos(vp.to_screen(b))],
                stroke,
                4.0,
                3.0,
            ));
        }
    }
}

/// Shapes without any sewing would be invisible; show their outlines.
fn draw_unsewn_outlines(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    for o in &app.session.design().objects {
        if o.kind == ObjectKind::Shape && o.visible && o.region.is_none() && o.line.is_none() {
            for c in &o.contours {
                draw_contour(
                    painter,
                    vp,
                    c,
                    Stroke::new(1.0, Color32::from_gray(120)),
                    None,
                );
            }
        }
    }
}

fn draw_contour(
    painter: &egui::Painter,
    vp: &Viewport,
    c: &Contour,
    stroke: Stroke,
    t: Option<&Transform>,
) {
    let pts: Vec<Pos2> = c
        .path()
        .iter()
        .map(|&p| pos(vp.to_screen(t.map_or(p, |t| t.apply(p)))))
        .collect();
    painter.add(Shape::line(pts, stroke));
}

fn draw_selection(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    if app.selection.is_empty() || matches!(app.tools.preview(), Some(ToolPreview::Transform(_))) {
        return;
    }
    let design = app.session.design();
    for o in design
        .objects
        .iter()
        .filter(|o| app.selection.contains(&o.id))
    {
        for c in &o.contours {
            draw_contour(
                painter,
                vp,
                c,
                Stroke::new(1.0, ACCENT.gamma_multiply(0.7)),
                None,
            );
        }
    }
    // The Point tool shows vertices instead of the transform handles.
    if app.tools.tool() == openstitch_app::editor::Tool::Point {
        return;
    }
    let Some(bounds) = bounds_of(design, &app.selection) else {
        return;
    };
    let h = SelectionHandles {
        bounds,
        units_per_point: vp.units_per_point(),
    };
    let r = model_rect(vp, &bounds);
    painter.add(Shape::dashed_line(
        &[
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ],
        Stroke::new(1.0, ACCENT),
        5.0,
        3.0,
    ));
    let top = pos(vp.to_screen(h.position(Handle::Top)));
    let rot = pos(vp.to_screen(h.position(Handle::Rotate)));
    painter.line_segment([top, rot], Stroke::new(1.0, ACCENT));
    painter.circle(
        rot,
        SelectionHandles::RADIUS as f32,
        Color32::WHITE,
        Stroke::new(1.5, ACCENT),
    );
    for handle in Handle::SCALE {
        let c = pos(vp.to_screen(h.position(handle)));
        let s = SelectionHandles::RADIUS as f32 * 0.8;
        painter.rect(
            egui::Rect::from_center_size(c, egui::vec2(2.0 * s, 2.0 * s)),
            CornerRadius::same(1),
            Color32::WHITE,
            Stroke::new(1.5, ACCENT),
            StrokeKind::Middle,
        );
    }
}

/// Point tool: the target's outline, curve handles and vertices (the selected one filled);
/// Pen tool: the handles of the anchor being placed.
fn draw_points(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    let handle_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.8));
    let handle = |at: Pos2| {
        painter.circle(at, 3.5, Color32::WHITE, Stroke::new(1.5, ACCENT));
    };
    for (a, h) in app.tools.pen_handles() {
        let (a, h) = (pos(vp.to_screen(a)), pos(vp.to_screen(h)));
        painter.line_segment([a, h], handle_stroke);
        handle(h);
    }
    let design = app.session.design().clone();
    if let Some(needles) = app.tools.needle_overlay(&app.tool_context(&design)) {
        // Stitch pattern: needle points as small squares (jump ends as circles), selected
        // ones filled.
        for (_, p, selected, jump) in needles.points {
            let at = pos(vp.to_screen(p));
            let fill = if selected { ACCENT } else { Color32::WHITE };
            if jump {
                painter.circle(at, 3.0, fill, Stroke::new(1.0, ACCENT));
            } else {
                painter.rect(
                    egui::Rect::from_center_size(at, egui::vec2(5.0, 5.0)),
                    CornerRadius::ZERO,
                    fill,
                    Stroke::new(1.0, ACCENT),
                    StrokeKind::Middle,
                );
            }
        }
        return;
    }
    let Some(overlay) = app.tools.point_overlay(&app.tool_context(&design)) else {
        return;
    };
    for (ci, c) in overlay.contours.iter().enumerate() {
        draw_contour(painter, vp, c, Stroke::new(1.5, ACCENT), None);
        for h in handle_lines(c) {
            let (a, at) = (pos(vp.to_screen(h.anchor)), pos(vp.to_screen(h.at)));
            painter.line_segment([a, at], handle_stroke);
            handle(at);
        }
        for (i, &p) in c.points.iter().enumerate() {
            let selected = overlay.selected == Some((ci, i));
            let s = SelectionHandles::RADIUS as f32 * 0.8;
            painter.rect(
                egui::Rect::from_center_size(pos(vp.to_screen(p)), egui::vec2(2.0 * s, 2.0 * s)),
                CornerRadius::ZERO,
                if selected { ACCENT } else { Color32::WHITE },
                Stroke::new(1.5, ACCENT),
                StrokeKind::Middle,
            );
        }
    }
}

fn draw_preview(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    match app.tools.preview() {
        Some(ToolPreview::Marquee(r)) => {
            let r = model_rect(vp, r);
            painter.rect(
                r,
                CornerRadius::ZERO,
                ACCENT.gamma_multiply(0.12),
                Stroke::new(1.0, ACCENT),
                StrokeKind::Middle,
            );
        }
        Some(ToolPreview::Outline(c)) => {
            draw_contour(painter, vp, c, Stroke::new(1.5, ACCENT), None);
            for &p in &c.points {
                painter.circle_filled(pos(vp.to_screen(p)), 3.0, ACCENT);
            }
        }
        Some(ToolPreview::Transform(t)) => {
            let design = app.session.design();
            for o in design
                .objects
                .iter()
                .filter(|o| app.selection.contains(&o.id))
            {
                match o.kind {
                    ObjectKind::Shape | ObjectKind::Text => {
                        for c in &o.contours {
                            draw_contour(painter, vp, c, Stroke::new(1.5, ACCENT), Some(t));
                        }
                    }
                    ObjectKind::Stitches => {
                        if let Some(b) = o.bounds() {
                            let c = Contour::rectangle(b.min_x, b.min_y, b.width(), b.height());
                            draw_contour(painter, vp, &c, Stroke::new(1.5, ACCENT), Some(t));
                        }
                    }
                }
            }
        }
        None => {}
    }
}
