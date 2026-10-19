//! Object list (sewing order) model.

use super::request::CommandRequest;
use openstitch_design::{Design, DesignObject, ObjectKind};
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectRow {
    pub id: u64,
    pub index: usize,
    pub label: String,
    pub detail: String,
    /// Thread colours in sewing order (region, line or the pattern's threads).
    pub swatches: Vec<[u8; 3]>,
    pub visible: bool,
    pub selected: bool,
}

pub fn label(o: &DesignObject) -> String {
    o.name.clone().unwrap_or_else(|| match o.kind {
        ObjectKind::Shape => format!("Shape {}", o.id),
        ObjectKind::Stitches => format!("Stitches {}", o.id),
        ObjectKind::Text => match &o.text {
            Some(t) => format!("“{}”", t.text.chars().take(24).collect::<String>()),
            None => format!("Text {}", o.id),
        },
    })
}

fn detail(o: &DesignObject) -> String {
    match o.kind {
        ObjectKind::Stitches => {
            let n = o.stitch_block.as_ref().map_or(0, |b| b.stitches.len());
            format!("Stitch pattern · {n} points")
        }
        ObjectKind::Shape | ObjectKind::Text => {
            let region = o
                .region
                .as_ref()
                .map(|r| super::inspector::region_name(Some(r.kind)));
            let line = o
                .line
                .as_ref()
                .map(|l| super::inspector::line_name(Some(l.kind)));
            match (region, line) {
                (Some(r), Some(l)) => format!("{r} + {l}"),
                (Some(r), None) => r.to_string(),
                (None, Some(l)) => l.to_string(),
                (None, None) => "not sewn".into(),
            }
        }
    }
}

fn swatches(o: &DesignObject) -> Vec<[u8; 3]> {
    let mut v: Vec<[u8; 3]> = Vec::new();
    let threads = o
        .region
        .iter()
        .map(|r| &r.thread)
        .chain(o.line.iter().map(|l| &l.thread));
    let threads: Vec<_> = threads
        .chain(o.stitch_block.iter().flat_map(|b| b.threads.iter()))
        .collect();
    for t in threads {
        let rgb = [t.red, t.green, t.blue];
        if !v.contains(&rgb) {
            v.push(rgb);
        }
    }
    v
}

/// Rows in sewing order (first sewn first).
pub fn rows(design: &Design, selection: &BTreeSet<u64>) -> Vec<ObjectRow> {
    design
        .objects
        .iter()
        .enumerate()
        .map(|(index, o)| ObjectRow {
            id: o.id,
            index,
            label: label(o),
            detail: detail(o),
            swatches: swatches(o),
            visible: o.visible,
            selected: selection.contains(&o.id),
        })
        .collect()
}

/// `object.reorder` moving `id` by `delta` places in the sewing order, if it can move.
pub fn shift(design: &Design, id: u64, delta: i64) -> Option<CommandRequest> {
    let from = design.index_of(id)? as i64;
    let to = from + delta;
    if delta == 0 || to < 0 || to >= design.objects.len() as i64 {
        return None;
    }
    Some(reorder(id, to as usize))
}

pub fn reorder(id: u64, index: usize) -> CommandRequest {
    CommandRequest::new("object.reorder", json!({ "id": id, "index": index }))
}

/// Target index when a row dragged from `from` is dropped above row `before` (`len` = at the end).
pub fn drop_index(from: usize, before: usize) -> usize {
    if before > from { before - 1 } else { before }
}

pub fn set_visible(id: u64, visible: bool) -> CommandRequest {
    CommandRequest::new("object.setVisible", json!({ "id": id, "visible": visible }))
}

/// `order.move` of `ids` to `to` (first, last, earlier, later).
pub fn move_to(ids: &BTreeSet<u64>, to: &str) -> CommandRequest {
    CommandRequest::new(
        "order.move",
        json!({ "ids": CommandRequest::ids(ids), "to": to }),
    )
}

/// `order.move` of `ids` as a block before (or after) `target`.
pub fn move_next_to(ids: &[u64], target: u64, after: bool) -> CommandRequest {
    CommandRequest::new(
        "order.move",
        json!({ "ids": ids, "to": if after { "after" } else { "before" }, "target": target }),
    )
}

/// PE-DESIGN's "Show by color order": consecutive objects starting with the same thread form
/// one frame.
pub fn colour_frames(design: &Design) -> Vec<Vec<u64>> {
    let mut frames: Vec<(Option<[u8; 3]>, Vec<u64>)> = Vec::new();
    for o in &design.objects {
        let c = swatches(o).first().copied();
        match frames.last_mut() {
            Some((last, ids)) if *last == c => ids.push(o.id),
            _ => frames.push((c, vec![o.id])),
        }
    }
    frames.into_iter().map(|(_, ids)| ids).collect()
}

/// Moving the frame at `index` one frame earlier (`delta` −1) or later (+1).
pub fn shift_frame(frames: &[Vec<u64>], index: usize, delta: i64) -> Option<CommandRequest> {
    let other = frames.get(usize::try_from(index as i64 + delta).ok()?)?;
    let frame = frames.get(index)?;
    Some(if delta < 0 {
        move_next_to(frame, other[0], false)
    } else {
        move_next_to(frame, *other.last()?, true)
    })
}

#[cfg(test)]
mod order_tests {
    use super::*;
    use openstitch_engine::Session;

    fn run(s: &mut Session, r: &CommandRequest) {
        s.execute(r.command, Some(&r.params))
            .unwrap_or_else(|e| panic!("{}: {e}", r.command));
    }

    fn ids(s: &Session) -> Vec<u64> {
        s.design().objects.iter().map(|o| o.id).collect()
    }

    #[test]
    fn colour_frames_move_as_blocks() {
        let mut s = Session::new(std::env::temp_dir());
        for (k, c) in ["Red", "Red", "Blue", "Red", "Blue", "Blue"]
            .iter()
            .enumerate()
        {
            let r = CommandRequest::new(
                "shape.rectangle",
                json!({ "x": k * 12, "y": 0, "width": 10, "height": 10, "fill": c }),
            );
            run(&mut s, &r);
        }
        let frames = colour_frames(s.design());
        assert_eq!(frames, vec![vec![1, 2], vec![3], vec![4], vec![5, 6]]);
        assert!(shift_frame(&frames, 0, -1).is_none());
        assert!(shift_frame(&frames, 3, 1).is_none());
        run(&mut s, &shift_frame(&frames, 3, -1).expect("earlier"));
        assert_eq!(ids(&s), [1, 2, 3, 5, 6, 4]);
        let frames = colour_frames(s.design());
        assert_eq!(frames, vec![vec![1, 2], vec![3, 5, 6], vec![4]]);
        run(&mut s, &shift_frame(&frames, 0, 1).expect("later"));
        assert_eq!(ids(&s), [3, 5, 6, 1, 2, 4]);
        run(&mut s, &move_to(&BTreeSet::from([4]), "first"));
        assert_eq!(ids(&s), [4, 3, 5, 6, 1, 2]);
    }
}
