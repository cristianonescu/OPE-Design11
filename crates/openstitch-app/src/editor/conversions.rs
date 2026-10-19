//! Requests for the Convert menu: shape/text → stitch pattern, region → line, stitch pattern →
//! shapes (whole outline, one per colour, blocks). Which items apply depends on the kinds of
//! the selected objects.

use crate::editor::CommandRequest;
use openstitch_design::{Design, ObjectKind};
use serde_json::json;
use std::collections::BTreeSet;

/// How a stitch pattern becomes shapes (`object.toShapes` `mode`).
pub const TO_SHAPES: [(&str, &str); 3] = [
    ("Convert to Outline (whole)", "whole"),
    ("Convert to Outlines (per colour)", "parts"),
    ("Convert to Blocks", "blocks"),
];

/// What the selection holds: (shapes or text, any with region sewing, stitch patterns).
pub fn kinds(d: &Design, ids: &BTreeSet<u64>) -> (bool, bool, bool) {
    let objects = || {
        ids.iter()
            .filter_map(|&id| d.index_of(id))
            .map(|i| &d.objects[i])
    };
    (
        objects().any(|o| o.is_outline()),
        objects().any(|o| o.is_outline() && o.region.is_some()),
        objects().any(|o| o.kind == ObjectKind::Stitches),
    )
}

pub fn to_stitches(ids: &BTreeSet<u64>) -> CommandRequest {
    CommandRequest::new(
        "object.toStitches",
        json!({ "ids": CommandRequest::ids(ids) }),
    )
}

pub fn region_to_line(ids: &BTreeSet<u64>) -> CommandRequest {
    CommandRequest::new(
        "object.regionToLine",
        json!({ "ids": CommandRequest::ids(ids) }),
    )
}

pub fn to_shapes(d: &Design, ids: &BTreeSet<u64>, mode: &str) -> CommandRequest {
    // Only the stitch patterns in the selection.
    let stitches: BTreeSet<u64> = ids
        .iter()
        .copied()
        .filter(|&id| {
            d.index_of(id)
                .is_some_and(|i| d.objects[i].kind == ObjectKind::Stitches)
        })
        .collect();
    CommandRequest::new(
        "object.toShapes",
        json!({ "ids": CommandRequest::ids(&stitches), "mode": mode }),
    )
}

pub fn divide_by_color(ids: &[u64]) -> CommandRequest {
    CommandRequest::new("stitches.divideByColor", json!({ "ids": ids }))
}

pub fn split_at(id: u64, index: usize) -> CommandRequest {
    CommandRequest::new("stitches.splitAt", json!({ "id": id, "index": index }))
}

pub fn connect(ids: &[u64]) -> CommandRequest {
    CommandRequest::new("stitches.connect", json!({ "ids": ids }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_engine::Session;

    fn exec(s: &mut Session, r: &CommandRequest) -> serde_json::Value {
        s.execute(r.command, Some(&r.params))
            .unwrap_or_else(|e| panic!("{}: {e}", r.command))
    }

    #[test]
    fn round_trip_through_the_menu_requests() {
        let mut s = Session::new(std::env::temp_dir());
        exec(
            &mut s,
            &CommandRequest::new(
                "shape.rectangle",
                json!({"x": 10, "y": 10, "width": 30, "height": 20}),
            ),
        );
        let sel = BTreeSet::from([1]);
        assert_eq!(kinds(s.design(), &sel), (true, true, false));
        exec(&mut s, &to_stitches(&sel));
        assert_eq!(kinds(s.design(), &sel), (false, false, true));
        for (_, mode) in TO_SHAPES {
            let request = to_shapes(s.design(), &sel, mode);
            let r = exec(&mut s, &request);
            let ids = CommandRequest::created_ids(&r);
            assert_eq!(ids.len(), 1, "{mode}");
            assert_eq!(kinds(s.design(), &ids), (true, true, false));
            s.execute("history.undo", None).expect("undo");
        }
        exec(
            &mut s,
            &CommandRequest::new(
                "shape.rectangle",
                json!({"x": 50, "y": 50, "width": 10, "height": 10}),
            ),
        );
        let shape = BTreeSet::from([2]);
        exec(&mut s, &region_to_line(&shape));
        assert_eq!(kinds(s.design(), &shape), (true, false, false));
    }
}
