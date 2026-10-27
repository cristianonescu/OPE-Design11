//! Point Edit on a text: editing single characters on the canvas (the guide's "Editing
//! individual characters"). Click a character of the selected text to select it; its box
//! gets the usual handles: drag inside to move it, the top handle to rotate it about its
//! middle, the other handles to resize it (its capital height; Shift snaps rotation to 15°
//! and locks a move to one axis). Each drag is one `text.setCharacter`, so one Undo.

use super::{
    DRAG_THRESHOLD, Gesture, Phase, PointerEvent, ToolContext, ToolController, ToolEffect,
    move_delta,
};
use crate::editor::handles::{Handle, SelectionHandles};
use crate::editor::hit::object_at;
use crate::editor::request::CommandRequest;
use openstitch_design::{
    CharEdit, Contour, Design, DesignObject, ObjectKind, Rect, TextSpec, Vec2, v2,
};
use serde_json::json;
use std::collections::BTreeSet;

/// What a character drag changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterDrag {
    Move,
    Rotate,
    Size,
}

/// A character as placed: its outlines' box, and how its offset moves it on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterFrame {
    pub bounds: Rect,
    /// Page movement (0.1 mm) of the character's box for 1 mm of offset along x and along y
    /// (the text's transform, path or Name Drop box included).
    pub per_mm: [Vec2; 2],
}

impl CharacterFrame {
    /// The offset (mm) that moves the character by `d` on the page.
    fn offset_for(&self, d: Vec2) -> Vec2 {
        let [a, b] = self.per_mm;
        let det = a.x * b.y - a.y * b.x;
        if det.abs() < 1e-9 {
            return d * 0.1;
        }
        v2((d.x * b.y - d.y * b.x) / det, (a.x * d.y - a.y * d.x) / det)
    }

    /// Whether the text is mirrored on the page (a clockwise turn shows anticlockwise).
    fn mirrored(&self) -> bool {
        let [a, b] = self.per_mm;
        a.x * b.y - a.y * b.x < 0.0
    }
}

/// The selected character and its handles, for the canvas (outlines as edited while a drag is
/// in progress).
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterOverlay {
    pub id: u64,
    pub index: usize,
    pub handles: SelectionHandles,
    pub outlines: Vec<Contour>,
}

/// Outlines of character `index` of `spec` laid out in `design` (its path and fonts).
fn outlines(design: &Design, spec: &TextSpec, index: usize) -> Vec<Contour> {
    let path = spec.path.and_then(|id| design.path_contour(id));
    openstitch_design::text::layout_in(spec, path.as_ref(), &design.fonts)
        .map(|l| {
            l.glyphs
                .into_iter()
                .filter(|g| g.index == index)
                .flat_map(|g| g.contours)
                .collect()
        })
        .unwrap_or_default()
}

fn bounds(contours: &[Contour]) -> Option<Rect> {
    Rect::enclosing(contours.iter().flat_map(Contour::outline_points))
}

/// `spec` with `edit` as the edit of its character.
fn with_edit(spec: &TextSpec, edit: &CharEdit) -> TextSpec {
    let mut s = spec.clone();
    s.set_char_edit(edit.clone());
    s
}

/// The edit of character `index` (an empty one when it has none).
pub fn char_edit(spec: &TextSpec, index: usize) -> CharEdit {
    spec.char_edit(index).cloned().unwrap_or(CharEdit {
        index,
        ..CharEdit::default()
    })
}

/// The frame of character `index` of text `o`: its box, and how 1 mm of offset moves it
/// (measured by laying the text out with the offset nudged).
pub fn character_frame(design: &Design, o: &DesignObject, index: usize) -> Option<CharacterFrame> {
    let spec = o.text.as_ref()?;
    let base = char_edit(spec, index);
    let b = bounds(&outlines(design, spec, index))?;
    let shift = |dx: f64, dy: f64| {
        let e = CharEdit {
            x: base.x + dx,
            y: base.y + dy,
            ..base.clone()
        };
        bounds(&outlines(design, &with_edit(spec, &e), index))
            .map_or(v2(dx * 10.0, dy * 10.0), |n| n.center() - b.center())
    };
    Some(CharacterFrame {
        bounds: b,
        per_mm: [shift(1.0, 0.0), shift(0.0, 1.0)],
    })
}

/// The character of text `o` under `p` (inside its outlines, else within `tolerance` of its
/// box; the smallest box wins).
pub fn character_at(design: &Design, o: &DesignObject, p: Vec2, tolerance: f64) -> Option<usize> {
    let spec = o.text.as_ref()?;
    let path = spec.path.and_then(|id| design.path_contour(id));
    let layout = openstitch_design::text::layout_in(spec, path.as_ref(), &design.fonts).ok()?;
    let mut by_index: Vec<(usize, Vec<Contour>)> = Vec::new();
    for g in layout.glyphs {
        match by_index.iter_mut().find(|(i, _)| *i == g.index) {
            Some((_, c)) => c.extend(g.contours),
            None => by_index.push((g.index, g.contours)),
        }
    }
    if let Some((i, _)) = by_index
        .iter()
        .find(|(_, c)| openstitch_design::geometry::contains(p, c))
    {
        return Some(*i);
    }
    by_index
        .iter()
        .filter_map(|(i, c)| {
            let b = bounds(c)?;
            let inside = p.x >= b.min_x - tolerance
                && p.x <= b.max_x + tolerance
                && p.y >= b.min_y - tolerance
                && p.y <= b.max_y + tolerance;
            inside.then_some((*i, b.width() * b.height()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

impl ToolContext<'_> {
    /// The text whose characters Point Edit edits: the selection when it is exactly one
    /// visible text object.
    pub fn text_target(&self) -> Option<&DesignObject> {
        if self.selection.len() != 1 {
            return None;
        }
        let id = *self.selection.first()?;
        self.design
            .objects
            .iter()
            .find(|o| o.id == id && o.kind == ObjectKind::Text && o.visible && o.text.is_some())
    }
}

/// The edit a drag of `mode` from `start` to `p` makes of `base`.
fn dragged(
    mode: CharacterDrag,
    base: &CharEdit,
    spec: &TextSpec,
    frame: &CharacterFrame,
    start: Vec2,
    p: Vec2,
    shift: bool,
) -> CharEdit {
    let round = |v: f64, k: f64| (v * k).round() / k;
    let mut e = base.clone();
    match mode {
        CharacterDrag::Move => {
            let o = frame.offset_for(move_delta(start, p, shift));
            e.x = round(base.x + o.x, 100.0).clamp(-500.0, 500.0);
            e.y = round(base.y + o.y, 100.0).clamp(-500.0, 500.0);
        }
        CharacterDrag::Rotate => {
            let h = SelectionHandles {
                bounds: frame.bounds,
                units_per_point: 1.0,
            };
            let mut deg = h.rotation(start, p, false);
            if frame.mirrored() {
                deg = -deg;
            }
            let mut r = base.rotation + deg;
            if shift {
                r = (r / 15.0).round() * 15.0;
            }
            // Keep within ±180°, as the engine takes ±360°.
            r = (r + 180.0).rem_euclid(360.0) - 180.0;
            e.rotation = round(r, 100.0);
        }
        CharacterDrag::Size => {
            let c = frame.bounds.center();
            let was = start.distance(c);
            let k = if was > 1e-9 { p.distance(c) / was } else { 1.0 };
            let size = base.size.unwrap_or(spec.size) * k;
            let size = round(size.clamp(1.0, 300.0), 10.0);
            e.size = ((size - spec.size).abs() > 1e-9).then_some(size);
        }
    }
    e
}

impl ToolController {
    /// The selected character of the Point Edit target text: (text id, character index).
    pub fn selected_character(&self, ctx: &ToolContext) -> Option<(u64, usize)> {
        let (id, index) = self.character?;
        let o = ctx.text_target().filter(|o| o.id == id)?;
        (index < o.text.as_ref()?.text.chars().count()).then_some((id, index))
    }

    /// The selected character's box, handles and outlines to draw.
    pub fn character_overlay(&self, ctx: &ToolContext) -> Option<CharacterOverlay> {
        if self.tool() != super::Tool::Point {
            return None;
        }
        let (id, index) = self.selected_character(ctx)?;
        let o = ctx.text_target()?;
        let spec = o.text.as_ref()?;
        let outlines = match &self.gesture {
            Some(Gesture::Character {
                edited: Some(e), ..
            }) => outlines(ctx.design, &with_edit(spec, e), index),
            _ => outlines(ctx.design, spec, index),
        };
        Some(CharacterOverlay {
            id,
            index,
            handles: SelectionHandles {
                bounds: bounds(&outlines)?,
                units_per_point: ctx.units_per_point,
            },
            outlines,
        })
    }

    pub(super) fn character_tool(
        &mut self,
        e: PointerEvent,
        ctx: &ToolContext,
        o: &DesignObject,
    ) -> Vec<ToolEffect> {
        let p = e.location;
        let Some(spec) = o.text.as_ref() else {
            return vec![];
        };
        match (e.phase, self.gesture.clone()) {
            (Phase::Down, _) => {
                let selected = self.selected_character(ctx).map(|(_, i)| i);
                let frame = selected.and_then(|i| character_frame(ctx.design, o, i));
                let on_handle = frame.as_ref().and_then(|f| {
                    SelectionHandles {
                        bounds: f.bounds,
                        units_per_point: ctx.units_per_point,
                    }
                    .handle_at(p)
                });
                let (index, mode) = match (selected, on_handle) {
                    (Some(i), Some(Handle::Rotate)) => (i, CharacterDrag::Rotate),
                    (Some(i), Some(_)) => (i, CharacterDrag::Size),
                    _ => match character_at(ctx.design, o, p, ctx.tolerance()) {
                        Some(i) => (i, CharacterDrag::Move),
                        None => {
                            self.character = None;
                            return match object_at(ctx.design, p, ctx.tolerance()) {
                                Some(id) if id == o.id => vec![],
                                Some(id) => vec![ToolEffect::Select(BTreeSet::from([id]))],
                                None => vec![ToolEffect::Select(BTreeSet::new())],
                            };
                        }
                    },
                };
                self.character = Some((o.id, index));
                let frame = match frame {
                    Some(f) if Some(index) == selected => Some(f),
                    _ => character_frame(ctx.design, o, index),
                };
                if let Some(frame) = frame {
                    self.gesture = Some(Gesture::Character {
                        id: o.id,
                        index,
                        mode,
                        start: p,
                        base: char_edit(spec, index),
                        frame,
                        edited: None,
                    });
                }
                vec![]
            }
            (
                Phase::Dragged,
                Some(Gesture::Character {
                    id,
                    index,
                    mode,
                    start,
                    base,
                    frame,
                    ..
                }),
            ) => {
                let edited = (p.distance(start) >= DRAG_THRESHOLD * ctx.units_per_point)
                    .then(|| dragged(mode, &base, spec, &frame, start, p, e.shift));
                self.gesture = Some(Gesture::Character {
                    id,
                    index,
                    mode,
                    start,
                    base,
                    frame,
                    edited,
                });
                vec![]
            }
            (
                Phase::Up,
                Some(Gesture::Character {
                    id, base, edited, ..
                }),
            ) => {
                self.gesture = None;
                match edited {
                    Some(e) if e != base => vec![ToolEffect::run(set_character(id, &e), false)],
                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }
}

/// `text.setCharacter` making `e` the edit of its character (size `null` = the text's size).
pub fn set_character(id: u64, e: &CharEdit) -> CommandRequest {
    CommandRequest::new(
        "text.setCharacter",
        json!({ "id": id, "index": e.index, "x": e.x, "y": e.y, "rotation": e.rotation, "size": e.size }),
    )
}
