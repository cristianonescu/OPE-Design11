//! The drawing tools behind the ribbon's Open Straight Line, Pencil, Edge radius and Manual
//! Punch (Curved Block, Running, Feed) buttons, and Move Sections, driven like the canvas
//! drives them and run through the engine.

use super::sections::{Axis, SectionMover, can_move, current_splits};
use super::*;
use openstitch_core::Command;
use openstitch_design::{Compiler, Design, ObjectKind, Vec2, v2};
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

/// Sends one event and applies its effects as the app does; returns the selection.
fn send(t: &mut ToolController, s: &mut Session, e: PointerEvent) -> BTreeSet<u64> {
    let design: Design = s.design().clone();
    let sel = BTreeSet::new();
    let effects = t.handle(
        e,
        &ToolContext {
            design: &design,
            selection: &sel,
            units_per_point: 1.0,
            scale_takes_origin: false,
        },
    );
    apply(s, effects)
}

fn apply(s: &mut Session, effects: Vec<ToolEffect>) -> BTreeSet<u64> {
    let mut selection = BTreeSet::new();
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

fn clicks(t: &mut ToolController, s: &mut Session, points: &[Vec2]) {
    for &p in points {
        send(t, s, PointerEvent::new(Phase::Down, p));
        send(t, s, PointerEvent::new(Phase::Up, p));
    }
}

fn finish(t: &mut ToolController, s: &mut Session) -> BTreeSet<u64> {
    let effects = t.finish();
    apply(s, effects)
}

#[test]
fn open_straight_line_ends_open_with_return_or_double_click() {
    let mut s = session();
    let mut t = ToolController::default();
    t.set_tool(Tool::Line);
    clicks(&mut t, &mut s, &[v2(0.0, 0.0), v2(100.0, 0.0)]);
    // Clicking the first point again does not close an open line.
    clicks(&mut t, &mut s, &[v2(0.5, 0.5)]);
    assert_eq!(t.polygon().len(), 3);
    let sel = finish(&mut t, &mut s);
    assert_eq!(sel.len(), 1);
    let o = &s.design().objects[0];
    assert!(!o.contours[0].closed && o.region.is_none() && o.line.is_some());
    assert_eq!(o.name.as_deref(), Some("Line"));
    // Double-click ends it too; one point makes nothing.
    clicks(&mut t, &mut s, &[v2(0.0, 50.0), v2(80.0, 50.0)]);
    let mut e = PointerEvent::new(Phase::Down, v2(80.0, 90.0));
    e.clicks = 2;
    send(&mut t, &mut s, e);
    assert_eq!(s.design().objects.len(), 2);
    clicks(&mut t, &mut s, &[v2(0.0, 0.0)]);
    assert!(finish(&mut t, &mut s).is_empty());
    assert_eq!(Tool::from_shortcut('L'), Some(Tool::Line));
}

/// A stroke dragged around a circle of radius `r` about `c`.
fn stroke(t: &mut ToolController, s: &mut Session, c: Vec2, r: f64, turn: f64) -> BTreeSet<u64> {
    let at = |k: usize| {
        let a = k as f64 / 200.0 * turn * std::f64::consts::TAU;
        c + v2(a.cos(), a.sin()) * r
    };
    send(t, s, PointerEvent::new(Phase::Down, at(0)));
    for k in 1..200 {
        send(t, s, PointerEvent::new(Phase::Dragged, at(k)));
    }
    assert!(t.is_busy() && matches!(t.preview(), Some(ToolPreview::Outline(_))));
    send(t, s, PointerEvent::new(Phase::Up, at(200)))
}

#[test]
fn pencil_strokes_become_smooth_outlines() {
    let mut s = session();
    let mut t = ToolController::default();
    t.set_tool(Tool::Pencil);
    let sel = stroke(&mut t, &mut s, v2(300.0, 300.0), 150.0, 1.0);
    assert_eq!(sel.len(), 1);
    assert!(!t.is_busy() && t.preview().is_none());
    let o = &s.design().objects[0];
    let c = &o.contours[0];
    assert!(c.closed && c.has_curves() && o.region.is_some());
    assert!(c.points.len() < 40, "{} vertices", c.points.len());
    for p in c.path() {
        assert!((p.distance(v2(300.0, 300.0)) - 150.0).abs() < 5.0, "{p:?}");
    }
    t.set_tool(Tool::PencilOpen);
    stroke(&mut t, &mut s, v2(300.0, 300.0), 100.0, 0.5);
    let o = &s.design().objects[1];
    assert!(!o.contours[0].closed && o.line.is_some() && o.region.is_none());
    // A click is not a stroke.
    send(&mut t, &mut s, PointerEvent::new(Phase::Down, v2(5.0, 5.0)));
    send(&mut t, &mut s, PointerEvent::new(Phase::Up, v2(5.0, 6.0)));
    assert_eq!(s.design().objects.len(), 2);
}

#[test]
fn edge_radius_rounds_the_rectangles_drawn() {
    let mut s = session();
    let mut t = ToolController::default();
    t.set_tool(Tool::Rectangle);
    t.set_corner_radius(5.0);
    assert_eq!(t.corner_radius(), 5.0);
    for (phase, p) in [
        (Phase::Down, v2(0.0, 0.0)),
        (Phase::Dragged, v2(300.0, 200.0)),
        (Phase::Up, v2(300.0, 200.0)),
    ] {
        send(&mut t, &mut s, PointerEvent::new(phase, p));
    }
    let c = &s.design().objects[0].contours[0];
    assert!(c.points.len() > 4, "rounded corners add points or curves");
    assert!(!c.points.contains(&v2(0.0, 0.0)), "the corner is cut");
    t.set_corner_radius(-3.0);
    assert_eq!(t.corner_radius(), 0.0);
    t.set_corner_radius(f64::NAN);
    assert_eq!(t.corner_radius(), 0.0);
}

#[test]
fn manual_punch_curved_block_running_and_feed() {
    let mut s = session();
    let mut t = ToolController::default();
    let rungs = [
        v2(100.0, 100.0),
        v2(100.0, 200.0),
        v2(400.0, 100.0),
        v2(350.0, 200.0),
        v2(500.0, 350.0),
        v2(400.0, 350.0),
    ];
    t.set_tool(Tool::CurvedBlock);
    clicks(&mut t, &mut s, &rungs);
    assert!(matches!(t.preview(), Some(ToolPreview::Outline(_))));
    finish(&mut t, &mut s);
    let o = &s.design().objects[0];
    let r = o.region.as_ref().expect("region");
    assert!(r.curved_lines && r.direction_lines.len() == 3);
    assert!(o.contours[0].has_curves());
    t.set_tool(Tool::Running);
    clicks(
        &mut t,
        &mut s,
        &[v2(500.0, 350.0), v2(600.0, 350.0), v2(600.0, 450.0)],
    );
    finish(&mut t, &mut s);
    let o = &s.design().objects[1];
    assert_eq!(o.name.as_deref(), Some("Running"));
    assert!(!o.contours[0].closed && o.line.is_some());
    t.set_tool(Tool::Feed);
    clicks(&mut t, &mut s, &[v2(600.0, 450.0), v2(900.0, 450.0)]);
    let mut e = PointerEvent::new(Phase::Down, v2(900.0, 450.0));
    e.clicks = 2;
    send(&mut t, &mut s, e);
    let o = &s.design().objects[2];
    assert_eq!(o.kind, ObjectKind::Stitches);
    let p = Compiler::default().compile(s.design());
    assert!(
        p.stitches
            .iter()
            .any(|x| x.command == Command::Jump && (x.x, x.y) == (900.0, 450.0))
    );
    // One point is not a line; one rung is not a block.
    clicks(&mut t, &mut s, &[v2(0.0, 0.0)]);
    assert!(finish(&mut t, &mut s).is_empty());
    t.set_tool(Tool::CurvedBlock);
    clicks(&mut t, &mut s, &rungs[..3]);
    assert!(finish(&mut t, &mut s).is_empty());
    assert_eq!(s.design().objects.len(), 3);
    assert!(Tool::ALL.iter().filter(|t| t.is_punch()).count() == 4);
}

/// A 230 × 160 mm custom page split into two 120 mm sections (130 × 180 mm hoop).
fn split_design(s: &mut Session) {
    run(
        s,
        &CommandRequest::new(
            "design.new",
            json!({ "name": "Banner", "width": 230, "height": 160 }),
        ),
    );
    run(
        s,
        &CommandRequest::new("sections.set", json!({ "hoop": "130x180" })),
    );
}

#[test]
fn move_sections_drags_a_split_line() {
    let mut s = session();
    assert!(!can_move(s.design()));
    split_design(&mut s);
    let d = s.design().clone();
    assert!(can_move(&d));
    let (xs, ys) = current_splits(&d);
    assert_eq!((xs.clone(), ys), (vec![115.0], vec![]));
    let x0 = xs[0];
    let mut m = SectionMover::default();
    assert!(!m.press(&d, v2(x0 * 10.0, 100.0), 10.0), "inactive");
    m.toggle();
    assert!(!m.press(&d, v2(x0 * 10.0 + 200.0, 100.0), 10.0), "too far");
    assert!(m.press(&d, v2(x0 * 10.0 + 3.0, 100.0), 10.0));
    m.drag(&d, v2(x0 * 10.0 - 50.0, 100.0));
    assert_eq!(m.dragged(), Some((Axis::Vertical, x0 - 5.0)));
    let r = m.release(&d, v2(x0 * 10.0 - 50.0, 100.0)).expect("moved");
    assert!(!m.is_dragging());
    run(&mut s, &r);
    assert_eq!(current_splits(s.design()).0[0], x0 - 5.0);
    // A line cannot pass its neighbour or the page edge.
    let d = s.design().clone();
    assert!(m.press(&d, v2((x0 - 5.0) * 10.0, 100.0), 10.0));
    m.drag(&d, v2(-500.0, 100.0));
    assert_eq!(m.dragged(), Some((Axis::Vertical, 1.0)));
    // Released where it was: nothing to do.
    let d = s.design().clone();
    assert!(m.press(&d, v2((x0 - 5.0) * 10.0, 100.0), 10.0));
    assert!(m.release(&d, v2((x0 - 5.0) * 10.0, 100.0)).is_none());
}
