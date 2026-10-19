use super::hit::{bounds_of, object_at, objects_in, rect_from};
use super::inspector::{Common, InspectorModel, pec_index, pec_palette};
use super::render::{PatternStats, RenderModel, color_at};
use super::*;
use openstitch_core::{Command, Stitch, StitchPattern, Thread};
use openstitch_design::{Compiler, Design, RegionType, Vec2, v2};
use openstitch_engine::Session;
use serde_json::json;
use std::collections::BTreeSet;

fn session() -> Session {
    Session::new(std::env::temp_dir())
}

fn run(s: &mut Session, r: &CommandRequest) -> serde_json::Value {
    s.execute(r.command, Some(&r.params))
        .unwrap_or_else(|e| panic!("{} failed: {e}", r.command))
}

/// Applies tool effects the way the app does; returns the new selection.
fn apply(s: &mut Session, effects: Vec<ToolEffect>, mut selection: BTreeSet<u64>) -> BTreeSet<u64> {
    for e in effects {
        match e {
            ToolEffect::Select(sel) => selection = sel,
            ToolEffect::Execute {
                requests,
                select_result,
            } => {
                let mut last = json!(null);
                for r in &requests {
                    last = run(s, r);
                }
                if select_result {
                    selection = CommandRequest::created_ids(&last);
                }
            }
        }
    }
    selection
}

fn ctx<'a>(design: &'a Design, selection: &'a BTreeSet<u64>) -> ToolContext<'a> {
    ToolContext {
        design,
        selection,
        units_per_point: 1.0,
        scale_takes_origin: false,
    }
}

fn drag(
    tool: &mut ToolController,
    s: &mut Session,
    sel: BTreeSet<u64>,
    from: Vec2,
    to: Vec2,
    shift: bool,
) -> BTreeSet<u64> {
    let mut sel = sel;
    let mid = from.lerp(to, 0.5);
    for (phase, p) in [
        (Phase::Down, from),
        (Phase::Dragged, mid),
        (Phase::Dragged, to),
        (Phase::Up, to),
    ] {
        let mut e = PointerEvent::new(phase, p);
        e.shift = shift;
        let design = s.design().clone();
        let effects = tool.handle(e, &ctx(&design, &sel));
        sel = apply(s, effects, sel);
    }
    sel
}

fn rect_design(s: &mut Session) -> u64 {
    let r = run(
        s,
        &CommandRequest::new(
            "shape.rectangle",
            json!({ "x": 10, "y": 10, "width": 20, "height": 10 }),
        ),
    );
    r["id"].as_u64().expect("id")
}

// MARK: viewport

#[test]
fn viewport_round_trips_and_zooms_about_anchor() {
    let mut v = Viewport {
        scale: 2.0,
        offset: v2(50.0, 20.0),
    };
    let p = v2(123.0, -45.0);
    let back = v.to_model(v.to_screen(p));
    assert!(back.distance(p) < 1e-9);
    let anchor = v2(300.0, 200.0);
    let under = v.to_model(anchor);
    v.zoom_about(anchor, 1.5);
    assert!((v.scale - 3.0).abs() < 1e-12);
    assert!(v.to_model(anchor).distance(under) < 1e-9);
    v.zoom_about(anchor, 1e9);
    assert_eq!(v.scale, Viewport::MAX_SCALE);
    v.zoom_about(anchor, f64::NAN);
    assert_eq!(v.scale, Viewport::MAX_SCALE);
    v.pan(10.0, -5.0);
    assert!(v.to_model(anchor + v2(10.0, -5.0)).distance(under) < 1e-9);
}

#[test]
fn viewport_fit_centres_page_with_margin() {
    let page = openstitch_design::Rect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 1000.0,
        max_y: 500.0,
    };
    let screen = ScreenRect {
        x: 100.0,
        y: 50.0,
        width: 840.0,
        height: 640.0,
    };
    let v = Viewport::fit(page, screen, 20.0);
    assert!((v.scale - 0.8).abs() < 1e-12); // width-limited: 800 / 1000
    let c = v.to_screen(v2(500.0, 250.0));
    assert!(c.distance(screen.center()) < 1e-9);
    assert!((v.to_screen(v2(0.0, 0.0)).x - 120.0).abs() < 1e-9);
}

// MARK: hit testing

#[test]
fn hit_testing_prefers_topmost_and_outline_tolerance() {
    let mut s = session();
    let a = rect_design(&mut s);
    let b = run(
        &mut s,
        &CommandRequest::new("shape.ellipse", json!({ "cx": 20, "cy": 15, "rx": 3 })),
    )["id"]
        .as_u64()
        .expect("id");
    let d = s.design();
    assert_eq!(
        object_at(d, v2(200.0, 150.0), 1.0),
        Some(b),
        "ellipse is sewn later, so on top"
    );
    assert_eq!(object_at(d, v2(120.0, 120.0), 1.0), Some(a));
    assert_eq!(
        object_at(d, v2(97.0, 120.0), 4.0),
        Some(a),
        "near the outline"
    );
    assert_eq!(object_at(d, v2(90.0, 120.0), 4.0), None);
    assert_eq!(
        objects_in(d, &rect_from(v2(150.0, 100.0), v2(250.0, 200.0))),
        BTreeSet::from([b])
    );
    let all = BTreeSet::from([a, b]);
    let bb = bounds_of(d, &all).expect("bounds");
    assert_eq!((bb.min_x, bb.max_x), (100.0, 300.0));
    run(&mut s, &objects::set_visible(b, false));
    assert_eq!(
        object_at(s.design(), v2(200.0, 150.0), 1.0),
        Some(a),
        "hidden objects are not hit"
    );
}

// MARK: handles

#[test]
fn handles_scale_about_opposite_corner_and_rotate() {
    let h = SelectionHandles {
        bounds: rect_from(v2(0.0, 0.0), v2(100.0, 50.0)),
        units_per_point: 1.0,
    };
    assert_eq!(h.handle_at(v2(101.0, 51.0)), Some(Handle::BottomRight));
    assert_eq!(h.handle_at(v2(50.0, -22.0)), Some(Handle::Rotate));
    assert_eq!(h.handle_at(v2(50.0, 25.0)), None);
    let s = h.scale(Handle::BottomRight, v2(100.0, 50.0), v2(200.0, 75.0), false);
    assert_eq!((s.sx, s.sy, s.origin), (2.0, 1.5, v2(0.0, 0.0)));
    let p = h.scale(Handle::BottomRight, v2(100.0, 50.0), v2(200.0, 75.0), true);
    assert_eq!((p.sx, p.sy), (2.0, 2.0));
    let e = h.scale(Handle::Right, v2(100.0, 25.0), v2(-50.0, 90.0), false);
    assert_eq!(
        (e.sx, e.sy),
        (0.01, 1.0),
        "no mirroring past the fixed edge; edge handles keep height"
    );
    let deg = h.rotation(v2(100.0, 25.0), v2(50.0, 125.0), false);
    assert!((deg - 90.0).abs() < 1e-9, "clockwise on screen is positive");
    assert_eq!(h.rotation(v2(100.0, 25.0), v2(100.0, 28.0), true), 0.0);
    assert_eq!(h.rotation(v2(100.0, 25.0), v2(100.0, 32.0), true), 15.0);
}

// MARK: tools through the engine

#[test]
fn rectangle_and_ellipse_tools_create_and_select() {
    let mut s = session();
    let mut t = ToolController::default();
    t.set_tool(Tool::Rectangle);
    let sel = drag(
        &mut t,
        &mut s,
        BTreeSet::new(),
        v2(100.0, 100.0),
        v2(300.0, 150.0),
        false,
    );
    assert_eq!(sel.len(), 1);
    let o = &s.design().objects[0];
    let b = o.bounds().expect("bounds");
    assert_eq!(
        (b.min_x, b.min_y, b.width(), b.height()),
        (100.0, 100.0, 200.0, 50.0)
    );
    assert_eq!(o.region.as_ref().map(|r| r.kind), Some(RegionType::Fill));

    t.set_tool(Tool::Ellipse);
    let sel = drag(&mut t, &mut s, sel, v2(0.0, 0.0), v2(80.0, 30.0), true);
    let b = s.design().objects[1].bounds().expect("bounds");
    assert!(
        (b.width() - 80.0).abs() < 0.01 && (b.height() - 80.0).abs() < 0.01,
        "shift draws a circle"
    );
    assert_eq!(sel, BTreeSet::from([s.design().objects[1].id]));

    drag(&mut t, &mut s, sel, v2(0.0, 0.0), v2(3.0, 3.0), false);
    assert_eq!(s.design().objects.len(), 2, "tiny drags create nothing");
}

#[test]
fn polygon_tool_closes_on_first_vertex_double_click_or_return() {
    let mut s = session();
    let mut t = ToolController::default();
    t.set_tool(Tool::Polygon);
    let empty = BTreeSet::new();
    let d = Design::default();
    for p in [v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 0.5), v2(50.0, 80.0)] {
        assert!(
            t.handle(PointerEvent::new(Phase::Down, p), &ctx(&d, &empty))
                .is_empty()
        );
    }
    assert_eq!(
        t.polygon().len(),
        3,
        "a click on the last vertex adds nothing"
    );
    t.handle(
        PointerEvent::new(Phase::Moved, v2(10.0, 60.0)),
        &ctx(&d, &empty),
    );
    assert!(matches!(t.preview(), Some(ToolPreview::Outline(c)) if c.points.len() == 4));
    let effects = t.handle(
        PointerEvent::new(Phase::Down, v2(1.0, 1.0)),
        &ctx(&d, &empty),
    );
    let sel = apply(&mut s, effects, BTreeSet::new());
    assert_eq!(sel.len(), 1);
    assert_eq!(s.design().objects[0].contours[0].points.len(), 3);
    assert!(!t.is_busy());

    t.handle(
        PointerEvent::new(Phase::Down, v2(0.0, 0.0)),
        &ctx(&d, &empty),
    );
    t.handle(
        PointerEvent::new(Phase::Down, v2(50.0, 0.0)),
        &ctx(&d, &empty),
    );
    assert!(t.finish_polygon().is_empty(), "two vertices are discarded");
    for p in [v2(0.0, 0.0), v2(50.0, 0.0), v2(50.0, 50.0)] {
        t.handle(PointerEvent::new(Phase::Down, p), &ctx(&d, &empty));
    }
    let mut dbl = PointerEvent::new(Phase::Down, v2(50.0, 50.0));
    dbl.clicks = 2;
    assert_eq!(t.handle(dbl, &ctx(&d, &empty)).len(), 1);
}

#[test]
fn select_tool_moves_marquees_and_toggles() {
    let mut s = session();
    let a = rect_design(&mut s);
    let b = run(
        &mut s,
        &CommandRequest::new(
            "shape.rectangle",
            json!({ "x": 50, "y": 50, "width": 10, "height": 10 }),
        ),
    )["id"]
        .as_u64()
        .expect("id");
    let mut t = ToolController::default();
    // Click selects; drag moves.
    let sel = drag(
        &mut t,
        &mut s,
        BTreeSet::new(),
        v2(150.0, 150.0),
        v2(170.0, 140.0),
        false,
    );
    assert_eq!(sel, BTreeSet::from([a]));
    let bb = s.design().objects[0].bounds().expect("bounds");
    assert_eq!((bb.min_x, bb.min_y), (120.0, 90.0));
    // Shift-click toggles the other object in.
    let design = s.design().clone();
    let effects = t.handle(
        PointerEvent::new(Phase::Down, v2(550.0, 550.0)).shift(),
        &ctx(&design, &sel),
    );
    let sel = apply(&mut s, effects, sel);
    t.handle(
        PointerEvent::new(Phase::Up, v2(550.0, 550.0)),
        &ctx(&design, &sel),
    );
    assert_eq!(sel, BTreeSet::from([a, b]));
    // Marquee from empty space selects only fully enclosed objects.
    let sel = drag(
        &mut t,
        &mut s,
        sel,
        v2(400.0, 400.0),
        v2(700.0, 700.0),
        false,
    );
    assert_eq!(sel, BTreeSet::from([b]));
    assert!(t.preview().is_none());
    // Click on empty canvas clears.
    let sel = drag(&mut t, &mut s, sel, v2(900.0, 10.0), v2(900.0, 10.0), false);
    assert!(sel.is_empty());
    // Move is one undo step.
    assert!(s.can_undo());
}

#[test]
fn handle_scale_keeps_opposite_corner_fixed_through_engine() {
    for takes_origin in [false, true] {
        let mut s = session();
        let a = rect_design(&mut s); // 100,100 – 300,200
        let sel = BTreeSet::from([a]);
        let design = s.design().clone();
        let mut c = ctx(&design, &sel);
        c.scale_takes_origin = takes_origin;
        let mut t = ToolController::default();
        t.handle(PointerEvent::new(Phase::Down, v2(300.0, 200.0)), &c);
        t.handle(PointerEvent::new(Phase::Dragged, v2(500.0, 300.0)), &c);
        assert!(matches!(t.preview(), Some(ToolPreview::Transform(_))));
        let effects = t.handle(PointerEvent::new(Phase::Up, v2(500.0, 300.0)), &c);
        let ToolEffect::Execute { requests, .. } = &effects[0] else {
            panic!("expected commands")
        };
        assert_eq!(requests.len(), if takes_origin { 1 } else { 2 });
        if takes_origin {
            assert_eq!(requests[0].params["originX"], json!(10.0));
            continue; // the current engine scales about the centre; only the request shape is checked
        }
        apply(&mut s, effects, sel.clone());
        let b = s.design().objects[0].bounds().expect("bounds");
        assert!(
            (b.min_x - 100.0).abs() < 0.01 && (b.min_y - 100.0).abs() < 0.01,
            "{b:?}"
        );
        assert!(
            (b.max_x - 500.0).abs() < 0.01 && (b.max_y - 300.0).abs() < 0.01,
            "{b:?}"
        );
    }
}

#[test]
fn rotate_handle_issues_rotation() {
    let mut s = session();
    let a = rect_design(&mut s);
    let sel = BTreeSet::from([a]);
    let mut t = ToolController::default();
    let rotate = v2(200.0, 100.0 - SelectionHandles::ROTATE_OFFSET);
    let to = v2(200.0 + 200.0, 150.0); // quarter turn clockwise about the centre (200, 150)
    drag(&mut t, &mut s, sel, rotate, to, false);
    let b = s.design().objects[0].bounds().expect("bounds");
    assert!(
        (b.width() - 100.0).abs() < 0.01 && (b.height() - 200.0).abs() < 0.01,
        "{b:?}"
    );
}

#[test]
fn tool_shortcuts_and_cancel() {
    assert_eq!(Tool::from_shortcut('E'), Some(Tool::Ellipse));
    assert_eq!(Tool::from_shortcut('x'), None);
    let mut t = ToolController::default();
    t.set_tool(Tool::Polygon);
    let d = Design::default();
    let sel = BTreeSet::new();
    t.handle(PointerEvent::new(Phase::Down, v2(0.0, 0.0)), &ctx(&d, &sel));
    assert!(t.is_busy());
    t.cancel();
    assert!(!t.is_busy() && t.preview().is_none());
}

// MARK: render model

fn pattern() -> StitchPattern {
    let mut p = StitchPattern::new();
    p.threads = vec![Thread::rgb(0xff0000), Thread::rgb(0x0000ff)];
    p.stitches = vec![
        Stitch::new(0.0, 0.0, Command::Jump),
        Stitch::new(0.0, 0.0, Command::Stitch),
        Stitch::new(10.0, 0.0, Command::Stitch),
        Stitch::new(20.0, 0.0, Command::Stitch),
        Stitch::new(20.0, 0.0, Command::Trim),
        Stitch::new(50.0, 50.0, Command::Jump),
        Stitch::new(50.0, 50.0, Command::Stitch),
        Stitch::new(60.0, 50.0, Command::Stitch),
        Stitch::new(60.0, 50.0, Command::ColorChange),
        Stitch::new(60.0, 60.0, Command::Stitch),
        Stitch::new(70.0, 60.0, Command::Stitch),
        Stitch::new(70.0, 60.0, Command::End),
    ];
    p
}

#[test]
fn render_model_splits_colours_strokes_and_travel() {
    let m = RenderModel::build(&pattern(), usize::MAX);
    assert_eq!(m.layers.len(), 2);
    assert_eq!(m.layers[0].rgb, [255, 0, 0]);
    assert_eq!(
        m.layers[0].strokes.len(),
        2,
        "the trim + jump breaks the stroke"
    );
    assert_eq!(
        m.layers[0].strokes[0],
        vec![v2(0.0, 0.0), v2(0.0, 0.0), v2(10.0, 0.0), v2(20.0, 0.0)]
    );
    assert_eq!(
        m.layers[1].strokes,
        vec![vec![v2(60.0, 50.0), v2(60.0, 60.0), v2(70.0, 60.0)]]
    );
    assert_eq!(m.travel, vec![(v2(20.0, 0.0), v2(50.0, 50.0))]);
    assert_eq!((m.shown, m.needle), (7, Some(v2(70.0, 60.0))));
}

#[test]
fn render_model_stops_at_simulator_position() {
    let m = RenderModel::build(&pattern(), 4);
    assert_eq!(m.shown, 4);
    assert_eq!(m.layers.len(), 1);
    assert_eq!(m.needle, Some(v2(50.0, 50.0)));
    assert_eq!(RenderModel::build(&pattern(), 0), RenderModel::default());
    assert_eq!(color_at(&pattern(), 5), (0, 2));
    assert_eq!(color_at(&pattern(), 6), (1, 2));
    let mut few = pattern();
    few.threads.truncate(1);
    assert_eq!(
        RenderModel::build(&few, usize::MAX).layers[1].rgb,
        render::UNKNOWN_THREAD
    );
}

#[test]
fn stats_match_engine_and_compiled_design() {
    let mut s = session();
    rect_design(&mut s);
    run(
        &mut s,
        &CommandRequest::new(
            "shape.ellipse",
            json!({ "cx": 50, "cy": 50, "rx": 10, "outline": "Red", "fill": "#00ff00" }),
        ),
    );
    let p = Compiler::default().compile(s.design());
    let st = PatternStats::of(&p);
    let engine = run(&mut s, &CommandRequest::new("design.generate", json!({})));
    assert_eq!(st.stitches as u64, engine["stitches"].as_u64().expect("n"));
    assert_eq!(
        st.color_changes as u64,
        engine["colorChanges"].as_u64().expect("n")
    );
    assert_eq!(st.width_mm, engine["widthMM"].as_f64().expect("w"));
    assert_eq!(st.colors.len(), 3);
    let m = RenderModel::build(&p, usize::MAX);
    assert_eq!(m.shown, st.stitches);
}

// MARK: inspector and object list

#[test]
fn inspector_summarises_mixed_selection_and_edits_through_engine() {
    let mut s = session();
    let a = rect_design(&mut s);
    let b = run(
        &mut s,
        &CommandRequest::new(
            "shape.polygon",
            json!({ "points": [[0, 0], [10, 0]], "closed": false }),
        ),
    )["id"]
        .as_u64()
        .expect("id");
    let both = BTreeSet::from([a, b]);
    let m = InspectorModel::new(s.design(), &both);
    assert!(!m.closed, "the polyline has no region");
    assert_eq!(m.region_kind, Common::Mixed);
    assert_eq!(m.line_kind, Common::Mixed);

    let only = BTreeSet::from([a]);
    let m = InspectorModel::new(s.design(), &only);
    assert!(m.closed);
    assert_eq!(m.region_kind, Common::Same(Some(RegionType::Fill)));
    run(&mut s, &m.set_region(json!({ "angle": 45, "density": 4, "stitchLength": 3, "pullCompensation": 0.2, "underlay": true })));
    run(&mut s, &m.set_region(json!({ "color": 5 })));
    run(&mut s, &m.set_region_type(Some(RegionType::Satin)));
    run(
        &mut s,
        &m.set_line_type(Some(openstitch_design::LineType::Zigzag)),
    );
    let hole = m.set_hole_sewing(s.design(), true).expect("has region");
    run(&mut s, &hole);
    let o = &s.design().objects[0];
    let r = o.region.as_ref().expect("region");
    assert_eq!(
        (r.kind, r.angle),
        (RegionType::Satin, 45.0),
        "type change keeps the angle"
    );
    assert_eq!(pec_index(&r.thread), Some(5));
    assert!(o.hole_sewing);
    assert_eq!(
        o.line.as_ref().map(|l| l.kind),
        Some(openstitch_design::LineType::Zigzag)
    );
    let m = InspectorModel::new(s.design(), &both);
    assert!(
        m.set_hole_sewing(s.design(), false)
            .is_some_and(|r| r.params["ids"] == json!([a]))
    );
    assert_eq!(pec_palette().len(), 64);
}

#[test]
fn object_list_rows_and_reorder() {
    let mut s = session();
    let a = rect_design(&mut s);
    let b = run(
        &mut s,
        &CommandRequest::new(
            "shape.ellipse",
            json!({ "cx": 5, "cy": 5, "rx": 2, "name": "Dot" }),
        ),
    )["id"]
        .as_u64()
        .expect("id");
    let rows = objects::rows(s.design(), &BTreeSet::from([b]));
    assert_eq!(
        rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        ["Shape 1", "Dot"]
    );
    assert!(rows[1].selected && !rows[0].selected);
    assert_eq!(rows[0].detail, "fill");
    assert_eq!(rows[0].swatches.len(), 1);
    assert!(objects::shift(s.design(), a, -1).is_none());
    let down = objects::shift(s.design(), a, 1).expect("can move down");
    run(&mut s, &down);
    assert_eq!(
        s.design().objects.iter().map(|o| o.id).collect::<Vec<_>>(),
        [b, a]
    );
    assert_eq!(objects::drop_index(0, 2), 1);
    assert_eq!(objects::drop_index(1, 0), 0);
    run(&mut s, &objects::reorder(a, objects::drop_index(1, 0)));
    assert_eq!(s.design().objects[0].id, a);
    s.execute("history.undo", None).expect("undo");
    assert_eq!(s.design().objects[0].id, b);
}

#[test]
fn transform_requests_run() {
    let mut s = session();
    let a = rect_design(&mut s);
    let sel = BTreeSet::from([a]);
    run(&mut s, &CommandRequest::flip(&sel, true));
    run(&mut s, &CommandRequest::rotate(&sel, 90.0));
    let created = CommandRequest::created_ids(&run(&mut s, &CommandRequest::duplicate(&sel)));
    assert_eq!(created.len(), 1);
    run(&mut s, &CommandRequest::delete(&created));
    assert_eq!(s.design().objects.len(), 1);
    assert_eq!(request::mm(123.456), 12.35);
}

// MARK: point editing and pen

use super::pen::{PenNode, path_request, pen_contour};
use super::points::{
    Control, PointHit, cubic_at, handle_lines, nearest_segment, point_at, segment_at, smooth_curve,
    with_vertex,
};
use openstitch_design::{Bezier, Contour};

/// Sends one pointer event and applies its effects.
fn send(
    tool: &mut ToolController,
    s: &mut Session,
    sel: BTreeSet<u64>,
    e: PointerEvent,
) -> BTreeSet<u64> {
    let design = s.design().clone();
    let effects = tool.handle(e, &ctx(&design, &sel));
    apply(s, effects, sel)
}

fn click_at(
    tool: &mut ToolController,
    s: &mut Session,
    sel: BTreeSet<u64>,
    p: Vec2,
    clicks: u32,
) -> BTreeSet<u64> {
    let mut e = PointerEvent::new(Phase::Down, p);
    e.clicks = clicks;
    let sel = send(tool, s, sel, e);
    send(tool, s, sel, PointerEvent::new(Phase::Up, p))
}

/// Rectangle (100,100)-(300,200) whose top segment is a curve bulging upwards.
fn curved_rect(s: &mut Session) -> u64 {
    let id = rect_design(s);
    run(
        s,
        &CommandRequest::new(
            "object.setSegment",
            json!({ "id": id, "segment": 0, "curve": [[15, 5], [25, 5]] }),
        ),
    );
    id
}

fn undo(s: &mut Session) {
    s.execute("history.undo", None).expect("undo");
}

#[test]
fn nearest_segment_finds_parameter_on_lines_and_curves() {
    let c = Contour::rectangle(0.0, 0.0, 100.0, 50.0);
    let (seg, t, d) = nearest_segment(&c, v2(25.0, -3.0)).expect("segment");
    assert_eq!(seg, 0);
    assert!((t - 0.25).abs() < 1e-9 && (d - 3.0).abs() < 1e-9);
    let (seg, t, _) = nearest_segment(&c, v2(-2.0, 40.0)).expect("closing segment");
    assert_eq!(seg, 3);
    assert!((t - 0.2).abs() < 1e-9);

    let mut curved = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0)], false);
    curved.set_curve(
        0,
        Some(Bezier {
            c1: v2(10.0, -80.0),
            c2: v2(90.0, -80.0),
        }),
    );
    for t in [0.1, 0.37, 0.5, 0.83] {
        let on = segment_at(&curved, 0, t);
        let (_, found, d) = nearest_segment(&curved, on).expect("segment");
        assert!(d < 1e-6, "point on the curve is at distance {d}");
        assert!((found - t).abs() < 1e-4, "t {found} vs {t}");
    }
    // Off the curve: the found point is the true nearest (no sample beats it).
    let p = v2(30.0, -20.0);
    let (_, t, d) = nearest_segment(&curved, p).expect("segment");
    assert!(
        (0..=1000).all(|k| p.distance(segment_at(&curved, 0, f64::from(k) / 1000.0)) >= d - 1e-6)
    );
    assert!(
        segment_at(&curved, 0, t).distance(cubic_at(
            v2(0.0, 0.0),
            v2(10.0, -80.0),
            v2(90.0, -80.0),
            v2(100.0, 0.0),
            t
        )) < 1e-12
    );
}

#[test]
fn point_hits_prefer_nearest_vertex_or_handle_then_segment() {
    let mut s = session();
    let id = curved_rect(&mut s);
    let o = s.design().objects[0].clone();
    assert_eq!(o.id, id);
    assert_eq!(handle_lines(&o.contours[0]).len(), 2);
    assert_eq!(
        point_at(&o, v2(302.0, 199.0), 7.0, 4.0),
        Some(PointHit::Vertex {
            contour: 0,
            index: 2
        })
    );
    assert_eq!(
        point_at(&o, v2(151.0, 52.0), 7.0, 4.0),
        Some(PointHit::Handle {
            contour: 0,
            segment: 0,
            control: Control::Start
        })
    );
    // On the right edge halfway down: a segment hit at t = 0.5.
    match point_at(&o, v2(302.0, 150.0), 7.0, 4.0) {
        Some(PointHit::Segment {
            contour: 0,
            segment: 1,
            t,
        }) => assert!((t - 0.5).abs() < 1e-9),
        other => panic!("expected segment 1, got {other:?}"),
    }
    assert_eq!(point_at(&o, v2(200.0, 150.0), 7.0, 4.0), None);
    // A vertex wins a tie with a handle on top of it.
    let mut c = o.clone();
    c.contours[0].set_curve(
        1,
        Some(Bezier {
            c1: v2(300.0, 200.0),
            c2: v2(300.0, 200.0),
        }),
    );
    assert_eq!(
        point_at(&c, v2(300.0, 200.0), 7.0, 4.0),
        Some(PointHit::Vertex {
            contour: 0,
            index: 2
        })
    );
}

#[test]
fn preview_edits_match_the_engine() {
    let mut s = session();
    let id = curved_rect(&mut s);
    for index in 0..4 {
        let c = s.design().objects[0].contours[0].clone();
        let expected = with_vertex(&c, index, c.points[index] + v2(30.0, -20.0));
        let v = expected.points[index];
        run(
            &mut s,
            &CommandRequest::new(
                "object.setPoint",
                json!({ "id": id, "index": index, "x": v.x / 10.0, "y": v.y / 10.0 }),
            ),
        );
        let got = &s.design().objects[0].contours[0];
        assert_eq!(got.curves.len(), expected.curves.len());
        for i in 0..got.segment_count() {
            let (a, b) = (got.curve(i), expected.curve(i));
            assert_eq!(a.is_some(), b.is_some());
            if let (Some(a), Some(b)) = (a, b) {
                assert!(a.c1.distance(b.c1) < 1e-9 && a.c2.distance(b.c2) < 1e-9);
            }
        }
    }
}

#[test]
fn point_tool_drags_vertex_as_one_undoable_command() {
    let mut s = session();
    let id = rect_design(&mut s);
    let before = s.design().clone();
    let mut t = ToolController::default();
    t.set_tool(Tool::from_shortcut('a').expect("point shortcut"));
    assert_eq!(t.tool(), Tool::Point);
    let sel = BTreeSet::from([id]);
    let sel = drag(
        &mut t,
        &mut s,
        sel,
        v2(300.0, 200.0),
        v2(350.0, 240.0),
        false,
    );
    assert_eq!(sel, BTreeSet::from([id]), "selection kept");
    assert_eq!(
        s.design().objects[0].contours[0].points[2],
        v2(350.0, 240.0)
    );
    let design = s.design().clone();
    let overlay = t.point_overlay(&ctx(&design, &sel)).expect("overlay");
    assert_eq!(overlay.selected, Some((0, 2)));
    undo(&mut s);
    assert_eq!(*s.design(), before, "one Undo reverts the drag");

    // The overlay follows the drag before release; nothing is committed yet.
    let design = s.design().clone();
    let c = ctx(&design, &sel);
    t.handle(PointerEvent::new(Phase::Down, v2(100.0, 100.0)), &c);
    t.handle(PointerEvent::new(Phase::Dragged, v2(80.0, 90.0)), &c);
    let o = t.point_overlay(&c).expect("overlay");
    assert_eq!(o.contours[0].points[0], v2(80.0, 90.0));
    t.cancel();
    assert_eq!(*s.design(), before);

    // Shift locks the move to an axis; a click without a drag changes nothing.
    let sel = drag(
        &mut t,
        &mut s,
        sel,
        v2(100.0, 200.0),
        v2(140.0, 210.0),
        true,
    );
    assert_eq!(
        s.design().objects[0].contours[0].points[3],
        v2(140.0, 200.0)
    );
    undo(&mut s);
    click_at(&mut t, &mut s, sel, v2(300.0, 100.0), 1);
    assert_eq!(*s.design(), before);
}

#[test]
fn point_tool_drags_handle_and_selects_shapes() {
    let mut s = session();
    let id = curved_rect(&mut s);
    let other = run(
        &mut s,
        &CommandRequest::new("shape.ellipse", json!({ "cx": 60, "cy": 60, "rx": 5 })),
    )["id"]
        .as_u64()
        .expect("id");
    let before = s.design().clone();
    let mut t = ToolController::default();
    t.set_tool(Tool::Point);
    // No target yet: clicking a shape selects it.
    let sel = click_at(&mut t, &mut s, BTreeSet::new(), v2(200.0, 150.0), 1);
    assert_eq!(sel, BTreeSet::from([id]));
    let sel = drag(&mut t, &mut s, sel, v2(250.0, 50.0), v2(260.0, 20.0), false);
    let z = s.design().objects[0].contours[0].curve(0).expect("curve");
    assert_eq!(z.c2, v2(260.0, 20.0));
    assert_eq!(z.c1, v2(150.0, 50.0), "the other handle stays");
    undo(&mut s);
    assert_eq!(*s.design(), before);
    let sel = click_at(&mut t, &mut s, sel, v2(600.0, 600.0), 1);
    assert_eq!(sel, BTreeSet::from([other]));
    let sel = click_at(&mut t, &mut s, sel, v2(900.0, 900.0), 1);
    assert!(sel.is_empty());
}

#[test]
fn point_tool_double_click_inserts_and_delete_removes() {
    let mut s = session();
    let id = curved_rect(&mut s);
    let mut t = ToolController::default();
    t.set_tool(Tool::Point);
    let sel = BTreeSet::from([id]);
    // Double-click the curved top edge at its apex (t = 0.5).
    let apex = segment_at(&s.design().objects[0].contours[0], 0, 0.5);
    let sel = click_at(&mut t, &mut s, sel, apex + v2(0.0, 1.0), 1);
    assert_eq!(
        s.design().objects[0].contours[0].points.len(),
        4,
        "single click inserts nothing"
    );
    let sel = click_at(&mut t, &mut s, sel, apex + v2(0.0, 1.0), 2);
    let c = s.design().objects[0].contours[0].clone();
    assert_eq!(c.points.len(), 5);
    assert!(
        c.points[1].distance(apex) < 0.5,
        "inserted at the clicked parameter"
    );
    assert!(
        c.curve(0).is_some() && c.curve(1).is_some(),
        "curve split in two"
    );
    let design = s.design().clone();
    let cx = ctx(&design, &sel);
    assert_eq!(t.selected_point(&cx).map(|p| p.index), Some(1));
    let effects = t.delete_point(&cx).expect("a vertex is selected");
    apply(&mut s, effects, sel.clone());
    assert_eq!(s.design().objects[0].contours[0].points.len(), 4);
    let design = s.design().clone();
    assert!(
        t.delete_point(&ctx(&design, &sel)).is_none(),
        "nothing selected now"
    );
    undo(&mut s);
    undo(&mut s);
    assert_eq!(s.design().objects[0].contours[0].points.len(), 4);
    assert!(s.design().objects[0].contours[0].curve(0).is_some());
    let mut select = ToolController::default();
    assert!(
        select.delete_point(&ctx(&design, &sel)).is_none(),
        "only the Point tool"
    );
}

#[test]
fn segment_shortcuts_curve_and_straighten_under_pointer() {
    let mut s = session();
    let id = rect_design(&mut s);
    let mut t = ToolController::default();
    t.set_tool(Tool::Point);
    let sel = BTreeSet::from([id]);
    send(
        &mut t,
        &mut s,
        sel.clone(),
        PointerEvent::new(Phase::Moved, v2(301.0, 150.0)),
    );
    let design = s.design().clone();
    let effects = t.set_segment_under_pointer(true, &ctx(&design, &sel));
    apply(&mut s, effects, sel.clone());
    let c = s.design().objects[0].contours[0].clone();
    let z = c.curve(1).expect("right edge curved");
    // Catmull-Rom: the handle at (300,100) leans along (300,200) − (100,100).
    assert!(z.c1.x > 300.0 && z.c2.x > 300.0, "bulges outwards: {z:?}");
    assert_eq!(c.curves.iter().flatten().count(), 1);
    let design = s.design().clone();
    assert!(
        t.set_segment_under_pointer(true, &ctx(&design, &sel))
            .is_empty(),
        "already curved"
    );
    let on_curve = segment_at(&design.objects[0].contours[0], 1, 0.5);
    send(
        &mut t,
        &mut s,
        sel.clone(),
        PointerEvent::new(Phase::Moved, on_curve),
    );
    let design = s.design().clone();
    let effects = t.set_segment_under_pointer(false, &ctx(&design, &sel));
    apply(&mut s, effects, sel.clone());
    assert!(!s.design().objects[0].contours[0].has_curves());
    // Far from any segment: nothing.
    send(
        &mut t,
        &mut s,
        sel.clone(),
        PointerEvent::new(Phase::Moved, v2(200.0, 150.0)),
    );
    let design = s.design().clone();
    assert!(
        t.set_segment_under_pointer(true, &ctx(&design, &sel))
            .is_empty()
    );
}

#[test]
fn smooth_curve_uses_neighbour_tangents_and_chord_at_open_ends() {
    let open = Contour::new(vec![v2(0.0, 0.0), v2(90.0, 0.0), v2(90.0, 90.0)], false);
    let z = smooth_curve(&open, 0);
    assert_eq!(z.c1, v2(30.0, 0.0), "open start: along the chord");
    assert_eq!(
        z.c2,
        v2(90.0 - 15.0, -15.0),
        "tangent (90,90)−(0,0) halved, a third"
    );
}

#[test]
fn pen_builds_corner_and_smooth_anchors_and_closes_on_first() {
    let mut s = session();
    let mut t = ToolController::default();
    t.set_tool(Tool::from_shortcut('b').expect("pen shortcut"));
    assert_eq!(t.tool(), Tool::Pen);
    let sel = click_at(&mut t, &mut s, BTreeSet::new(), v2(100.0, 100.0), 1);
    // Click-drag: a smooth anchor with symmetric handles.
    let sel = drag(
        &mut t,
        &mut s,
        sel,
        v2(300.0, 100.0),
        v2(350.0, 150.0),
        false,
    );
    assert_eq!(t.pen()[1].out, Some(v2(350.0, 150.0)));
    assert_eq!(t.pen()[1].incoming(), Some(v2(250.0, 50.0)));
    assert_eq!(t.pen_handles().len(), 2);
    let sel = click_at(&mut t, &mut s, sel, v2(200.0, 300.0), 1);
    send(
        &mut t,
        &mut s,
        sel.clone(),
        PointerEvent::new(Phase::Moved, v2(120.0, 200.0)),
    );
    match t.preview() {
        Some(ToolPreview::Outline(c)) => {
            assert_eq!(c.points.len(), 4, "rubber band to the pointer")
        }
        other => panic!("expected outline preview, got {other:?}"),
    }
    assert!(
        s.design().objects.is_empty(),
        "nothing committed while drawing"
    );
    let sel = click_at(&mut t, &mut s, sel, v2(102.0, 101.0), 1);
    assert!(!t.is_busy());
    assert_eq!(s.design().objects.len(), 1);
    let o = &s.design().objects[0];
    assert_eq!(sel, BTreeSet::from([o.id]), "new path selected");
    let c = &o.contours[0];
    assert!(c.closed);
    assert_eq!(
        c.points,
        vec![v2(100.0, 100.0), v2(300.0, 100.0), v2(200.0, 300.0)]
    );
    assert_eq!(
        c.curve(0),
        Some(Bezier {
            c1: v2(100.0, 100.0),
            c2: v2(250.0, 50.0)
        })
    );
    assert_eq!(
        c.curve(1),
        Some(Bezier {
            c1: v2(350.0, 150.0),
            c2: v2(200.0, 300.0)
        })
    );
    assert_eq!(c.curve(2), None, "corner to corner stays straight");
    undo(&mut s);
    assert!(s.design().objects.is_empty(), "the whole path is one Undo");
}

#[test]
fn pen_return_finishes_open_and_rejects_too_few_points() {
    let mut s = session();
    let mut t = ToolController::default();
    t.set_tool(Tool::Pen);
    let sel = click_at(&mut t, &mut s, BTreeSet::new(), v2(0.0, 0.0), 1);
    assert!(t.finish().is_empty(), "one anchor is not a path");
    assert!(!t.is_busy());
    let sel = click_at(&mut t, &mut s, sel, v2(0.0, 0.0), 1);
    let sel = click_at(&mut t, &mut s, sel, v2(1.0, 0.0), 1);
    assert_eq!(t.pen().len(), 1, "a click on the last anchor adds nothing");
    let sel = click_at(&mut t, &mut s, sel, v2(100.0, 0.0), 2);
    let effects = t.finish();
    let sel = apply(&mut s, effects, sel);
    let c = &s.design().objects[0].contours[0];
    assert!(!c.closed && c.points.len() == 2 && !c.has_curves());
    assert_eq!(sel.len(), 1);
    // Escape abandons.
    click_at(&mut t, &mut s, sel, v2(0.0, 50.0), 1);
    t.cancel();
    assert!(!t.is_busy() && t.preview().is_none());
}

#[test]
fn pen_request_omits_curves_when_all_straight() {
    let corner = [
        PenNode::corner(v2(0.0, 0.0)),
        PenNode::corner(v2(100.0, 0.0)),
        PenNode::corner(v2(0.0, 100.0)),
    ];
    let r = path_request(&corner, true);
    assert_eq!(r.command, "shape.path");
    assert!(r.params.get("curves").is_none());
    assert_eq!(
        r.params["points"],
        json!([[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]])
    );
    let smooth = [
        PenNode::corner(v2(0.0, 0.0)),
        PenNode {
            at: v2(100.0, 0.0),
            out: Some(v2(100.0, 50.0)),
        },
    ];
    assert!(pen_contour(&smooth, true).has_curves());
    let r = path_request(&smooth, true);
    assert_eq!(
        r.params["curves"],
        json!([[[0.0, 0.0], [10.0, -5.0]], [[10.0, 5.0], [0.0, 0.0]]])
    );
    let mut s = session();
    run(&mut s, &r);
    assert_eq!(s.design().objects[0].contours[0].curves.len(), 2);
}

#[test]
fn click_counter_detects_double_clicks() {
    let mut c = ClickCounter::default();
    assert_eq!(c.press(1.0, v2(10.0, 10.0)), 1);
    assert_eq!(c.press(1.2, v2(12.0, 11.0)), 2);
    assert_eq!(c.press(1.3, v2(12.0, 11.0)), 3);
    assert_eq!(c.press(2.0, v2(12.0, 11.0)), 1, "too slow");
    assert_eq!(c.press(2.1, v2(40.0, 11.0)), 1, "too far");
}

#[test]
fn every_line_type_has_inspector_controls_the_engine_accepts() {
    use super::inspector::{Control, line_controls, line_types};
    use openstitch_design::{LineSewing, LineType};
    let mut s = Session::new(std::env::temp_dir());
    s.execute(
        "shape.ellipse",
        Some(&json!({"cx": 50, "cy": 50, "rx": 20, "ry": 20, "outline": "Black"})),
    )
    .expect("ellipse");
    assert_eq!(line_types().len(), LineType::ALL.len() + 1);
    for kind in LineType::ALL {
        let controls = line_controls(&LineSewing::new(kind, Thread::rgb(0)));
        assert!(!controls.is_empty(), "{kind:?}");
        s.execute("object.setLine", Some(&json!({"id": 1, "type": kind.id()})))
            .expect("type");
        for c in &controls {
            let v = match c {
                Control::Number { value, .. } => json!(value),
                Control::Choice { options, .. } => json!(options.last().expect("options").0),
                Control::Toggle { value, .. } => json!(!value),
            };
            let mut params = c.params(v);
            params["id"] = json!(1);
            s.execute("object.setLine", Some(&params))
                .unwrap_or_else(|e| panic!("{kind:?} {params}: {e}"));
        }
    }
    let times = &line_controls(&LineSewing::new(LineType::Chain, Thread::rgb(0)))[2];
    assert_eq!(times.params(json!(2.6)), json!({"runTimes": 3}));
}

#[test]
fn auto_satin_offers_no_travel() {
    use super::inspector::{Control, region_controls};
    use openstitch_design::RegionSewing;
    let travel = |r: &RegionSewing| {
        region_controls(r).iter().any(|c| {
            matches!(
                c,
                Control::Toggle {
                    param: "travel",
                    ..
                }
            )
        })
    };
    let mut r = RegionSewing::new(RegionType::Satin, Thread::rgb(0));
    assert!(travel(&r));
    r.auto_direction = true;
    assert!(!travel(&r), "no Running Stitch Path with Auto satin");
    r.kind = RegionType::Fill;
    assert!(travel(&r));
}

#[test]
fn region_option_controls_are_accepted_by_the_engine() {
    use super::inspector::{Control, region_controls};
    use openstitch_design::RegionSewing;
    let mut s = Session::new(std::env::temp_dir());
    s.execute(
        "shape.ellipse",
        Some(&json!({"cx": 50, "cy": 50, "rx": 20, "ry": 10, "fill": "Red"})),
    )
    .expect("ellipse");
    for kind in RegionType::ALL {
        s.execute(
            "object.setRegion",
            Some(&json!({"id": 1, "type": kind.id()})),
        )
        .expect("type");
        let mut r = RegionSewing::new(kind, Thread::rgb(0));
        r.underlay = true;
        r.underlay_style = openstitch_design::UnderlayStyle::Zigzag;
        r.feather = Some(openstitch_design::Feather {
            side: openstitch_design::FeatherSide::Start,
            width: 1.0,
        });
        for c in region_controls(&r) {
            let v = match &c {
                Control::Number { value, range, .. } => {
                    json!(value.max(range.0 + 2.5).min(range.1))
                }
                Control::Choice { options, .. } => json!(options.last().expect("options").0),
                Control::Toggle { value, .. } => json!(!value),
            };
            let mut params = c.params(v);
            params["id"] = json!(1);
            s.execute("object.setRegion", Some(&params))
                .unwrap_or_else(|e| panic!("{kind:?} {params}: {e}"));
        }
    }
}
