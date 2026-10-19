//! The Satin Skeleton tool and on-canvas character edits (Point Edit on a text), driven like
//! the canvas drives them and run through the engine.

use super::ribbon::Action;
use super::tools::characters::{char_edit, character_frame};
use super::*;
use openstitch_design::{Compiler, Design, Vec2, v2};
use openstitch_engine::Session;
use serde_json::json;
use std::collections::BTreeSet;

fn run(s: &mut Session, command: &str, params: serde_json::Value) -> serde_json::Value {
    s.execute(command, Some(&params))
        .unwrap_or_else(|e| panic!("{command} failed: {e}"))
}

/// Sends one event with `sel` selected and runs its effects; returns the commands run.
fn send(
    t: &mut ToolController,
    s: &mut Session,
    sel: &mut BTreeSet<u64>,
    e: PointerEvent,
) -> Vec<&'static str> {
    let design: Design = s.design().clone();
    let effects = t.handle(e, &ctx(&design, sel));
    apply(s, sel, effects)
}

fn ctx<'a>(design: &'a Design, sel: &'a BTreeSet<u64>) -> ToolContext<'a> {
    ToolContext {
        design,
        selection: sel,
        units_per_point: 1.0,
        scale_takes_origin: true,
    }
}

fn apply(s: &mut Session, sel: &mut BTreeSet<u64>, effects: Vec<ToolEffect>) -> Vec<&'static str> {
    let mut ran = Vec::new();
    for e in effects {
        match e {
            ToolEffect::Select(new) => *sel = new,
            ToolEffect::Execute {
                requests,
                select_result,
            } => {
                let mut last = json!(null);
                for r in &requests {
                    last = run(s, r.command, r.params.clone());
                    ran.push(r.command);
                }
                if select_result {
                    *sel = CommandRequest::created_ids(&last);
                }
            }
        }
    }
    ran
}

/// Press at `from`, drag through to `to`, release; returns the commands run.
fn drag(
    t: &mut ToolController,
    s: &mut Session,
    sel: &mut BTreeSet<u64>,
    from: Vec2,
    to: Vec2,
) -> Vec<&'static str> {
    let mut ran = send(t, s, sel, PointerEvent::new(Phase::Down, from));
    for k in 1..=4 {
        let p = from.lerp(to, f64::from(k) / 4.0);
        ran.extend(send(t, s, sel, PointerEvent::new(Phase::Dragged, p)));
    }
    ran.extend(send(t, s, sel, PointerEvent::new(Phase::Up, to)));
    ran
}

fn skeleton_of(s: &Session, id: u64) -> openstitch_design::skeleton::Skeleton {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")]
        .region
        .as_ref()
        .and_then(|r| r.skeleton.clone())
        .expect("skeleton")
}

#[test]
fn satin_skeleton_tool_draws_a_column_with_a_width_per_point() {
    let mut s = Session::new(std::env::temp_dir());
    let mut sel = BTreeSet::new();
    let mut t = ToolController::default();
    assert_eq!(Action::SatinSkeleton.tool(), Some(Tool::Skeleton));
    t.set_tool(Tool::Skeleton);
    t.set_skeleton_width(4.0);
    assert_eq!(t.skeleton_width(), 4.0);
    for p in [v2(100.0, 100.0), v2(300.0, 100.0)] {
        drag(&mut t, &mut s, &mut sel, p, p);
    }
    // A smooth third point: dragged out handles make the last segment a curve.
    drag(&mut t, &mut s, &mut sel, v2(500.0, 300.0), v2(600.0, 300.0));
    assert!(t.is_busy());
    let overlay = t
        .skeleton_overlay(&ctx(&Design::default(), &sel))
        .expect("overlay");
    assert_eq!(overlay.id, None);
    assert_eq!(overlay.widths.len(), 3);
    assert!(overlay.column.is_some());
    // The width handles of point 1 sit 2 mm either side of it; drag one out to 4 mm.
    let (at, left, right) = overlay.widths[1];
    assert!((at.distance(left) - 20.0).abs() < 1e-6 && (at.distance(right) - 20.0).abs() < 1e-6);
    let out = at + (left - at) * 2.0;
    assert!(drag(&mut t, &mut s, &mut sel, left, out).is_empty());
    let ran = apply(&mut s, &mut sel, t.finish());
    assert_eq!(ran, ["shape.skeletonSatin"]);
    assert!(!t.is_busy());
    let id = *sel.first().expect("selected");
    let sk = skeleton_of(&s, id);
    assert!(sk.points.len() > 3, "the curve is flattened");
    assert_eq!(sk.widths[0], 4.0);
    assert_eq!(sk.widths[1], 8.0);
    assert_eq!(*sk.widths.last().expect("w"), 4.0);
    assert_eq!(sk.points[1], v2(300.0, 100.0));
    let p = Compiler::default().compile(s.design());
    assert!(p.count(openstitch_core::Command::Stitch) > 100);
    // One Undo removes it.
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().objects.is_empty());
}

#[test]
fn satin_skeleton_tool_edits_a_stored_skeleton() {
    let mut s = Session::new(std::env::temp_dir());
    let id = run(
        &mut s,
        "shape.skeletonSatin",
        json!({ "points": [[10, 10], [40, 10]], "width": 3 }),
    )["id"]
        .as_u64()
        .expect("id");
    let mut sel = BTreeSet::from([id]);
    let mut t = ToolController::default();
    t.set_tool(Tool::Skeleton);
    let design = s.design().clone();
    let overlay = t.skeleton_overlay(&ctx(&design, &sel)).expect("overlay");
    assert_eq!(overlay.id, Some(id));
    // Point 0's width handle from 3 mm to 5 mm; the column's outline follows.
    let (at, left, _) = overlay.widths[0];
    let before = s.design().objects[0].contours.clone();
    let ran = drag(
        &mut t,
        &mut s,
        &mut sel,
        left,
        at + (left - at) * (5.0 / 3.0),
    );
    assert_eq!(ran, ["object.setSkeleton"]);
    assert_eq!(skeleton_of(&s, id).widths, vec![5.0, 3.0]);
    assert_ne!(s.design().objects[0].contours, before);
    // A centre point moves.
    let ran = drag(&mut t, &mut s, &mut sel, v2(400.0, 100.0), v2(400.0, 150.0));
    assert_eq!(ran, ["object.setSkeleton"]);
    assert_eq!(skeleton_of(&s, id).points[1], v2(400.0, 150.0));
    // A click does nothing; each drag was one Undo.
    assert!(drag(&mut t, &mut s, &mut sel, v2(400.0, 150.0), v2(400.0, 150.0)).is_empty());
    run(&mut s, "history.undo", json!({}));
    assert_eq!(skeleton_of(&s, id).points[1], v2(400.0, 100.0));
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().objects[0].contours, before);
}

#[test]
fn satin_skeleton_needs_a_line() {
    let mut s = Session::new(std::env::temp_dir());
    let mut sel = BTreeSet::new();
    let mut t = ToolController::default();
    t.set_tool(Tool::Skeleton);
    drag(&mut t, &mut s, &mut sel, v2(0.0, 0.0), v2(0.0, 0.0));
    assert!(t.finish().is_empty(), "one point is no line");
    drag(&mut t, &mut s, &mut sel, v2(0.0, 0.0), v2(0.0, 0.0));
    drag(&mut t, &mut s, &mut sel, v2(100.0, 0.0), v2(100.0, 0.0));
    t.cancel();
    assert!(!t.is_busy());
    assert!(t.finish().is_empty());
    // Double-click ends it.
    drag(&mut t, &mut s, &mut sel, v2(0.0, 0.0), v2(0.0, 0.0));
    drag(&mut t, &mut s, &mut sel, v2(100.0, 0.0), v2(100.0, 0.0));
    let mut e = PointerEvent::new(Phase::Down, v2(100.0, 0.0));
    e.clicks = 2;
    assert_eq!(send(&mut t, &mut s, &mut sel, e), ["shape.skeletonSatin"]);
}

#[test]
fn characters_are_moved_turned_and_resized_on_the_canvas() {
    let mut s = Session::new(std::env::temp_dir());
    let id = run(
        &mut s,
        "text.add",
        json!({ "text": "AB", "x": 20, "y": 30, "size": 10 }),
    )["id"]
        .as_u64()
        .expect("id");
    let mut sel = BTreeSet::from([id]);
    let mut t = ToolController::default();
    t.set_tool(Tool::Point);
    let o = |s: &Session| s.design().objects[0].clone();
    let frame = character_frame(s.design(), &o(&s), 1).expect("B");
    // Unrotated text: 1 mm of offset moves the letter 1 mm.
    assert!((frame.per_mm[0] - v2(10.0, 0.0)).length() < 0.1);
    assert!((frame.per_mm[1] - v2(0.0, 10.0)).length() < 0.1);
    let c = frame.bounds.center();
    // Click B to select it; the text stays selected.
    drag(&mut t, &mut s, &mut sel, c, c);
    assert_eq!(sel, BTreeSet::from([id]));
    let design = s.design().clone();
    assert_eq!(t.selected_character(&ctx(&design, &sel)), Some((id, 1)));
    assert!(t.character_overlay(&ctx(&design, &sel)).is_some());
    // Drag it 5 mm right and 2 mm up.
    let ran = drag(&mut t, &mut s, &mut sel, c, c + v2(50.0, -20.0));
    assert_eq!(ran, ["text.setCharacter"]);
    let e = char_edit(o(&s).text.as_ref().expect("t"), 1);
    assert!(
        (e.x - 5.0).abs() < 0.05 && (e.y + 2.0).abs() < 0.05,
        "{e:?}"
    );
    assert!(char_edit(o(&s).text.as_ref().expect("t"), 0).is_noop());
    // The rotate handle a quarter turn clockwise.
    let frame = character_frame(s.design(), &o(&s), 1).expect("B");
    let h = SelectionHandles {
        bounds: frame.bounds,
        units_per_point: 1.0,
    };
    let (top, c) = (h.position(Handle::Rotate), frame.bounds.center());
    let right = c + v2(c.distance(top), 0.0);
    let mut sh = PointerEvent::new(Phase::Down, top);
    sh.shift = true;
    send(&mut t, &mut s, &mut sel, sh);
    let mut up = PointerEvent::new(Phase::Up, right);
    up.shift = true;
    let mut mv = PointerEvent::new(Phase::Dragged, right);
    mv.shift = true;
    send(&mut t, &mut s, &mut sel, mv);
    assert_eq!(send(&mut t, &mut s, &mut sel, up), ["text.setCharacter"]);
    let e = char_edit(o(&s).text.as_ref().expect("t"), 1);
    assert_eq!(e.rotation, 90.0);
    // A corner handle dragged out to one and a half times the distance: size 15 mm.
    let frame = character_frame(s.design(), &o(&s), 1).expect("B");
    let h = SelectionHandles {
        bounds: frame.bounds,
        units_per_point: 1.0,
    };
    let corner = h.position(Handle::BottomRight);
    let c = frame.bounds.center();
    let ran = drag(&mut t, &mut s, &mut sel, corner, c + (corner - c) * 1.5);
    assert_eq!(ran, ["text.setCharacter"]);
    let e = char_edit(o(&s).text.as_ref().expect("t"), 1);
    assert_eq!(e.size, Some(15.0));
    assert_eq!(e.rotation, 90.0, "the other values stay");
    // Three drags, three Undos.
    for _ in 0..3 {
        run(&mut s, "history.undo", json!({}));
    }
    assert!(o(&s).text.expect("t").characters.is_empty());
    // Clicking off the text clears the selection.
    drag(
        &mut t,
        &mut s,
        &mut sel,
        v2(5000.0, 5000.0),
        v2(5000.0, 5000.0),
    );
    assert!(sel.is_empty());
}

#[test]
fn character_moves_follow_the_texts_rotation() {
    let mut s = Session::new(std::env::temp_dir());
    let id = run(
        &mut s,
        "text.add",
        json!({ "text": "AB", "x": 20, "y": 30, "size": 10 }),
    )["id"]
        .as_u64()
        .expect("id");
    run(
        &mut s,
        "object.rotate",
        json!({ "ids": [id], "degrees": 90 }),
    );
    let o = s.design().objects[0].clone();
    let frame = character_frame(s.design(), &o, 0).expect("A");
    // Turned a quarter clockwise: the text's x runs down the page.
    assert!(
        (frame.per_mm[0] - v2(0.0, 10.0)).length() < 0.2,
        "{:?}",
        frame.per_mm
    );
    let mut sel = BTreeSet::from([id]);
    let mut t = ToolController::default();
    t.set_tool(Tool::Point);
    let c = frame.bounds.center();
    drag(&mut t, &mut s, &mut sel, c, c + v2(0.0, 30.0));
    let e = char_edit(s.design().objects[0].text.as_ref().expect("t"), 0);
    assert!((e.x - 3.0).abs() < 0.1 && e.y.abs() < 0.1, "{e:?}");
}
