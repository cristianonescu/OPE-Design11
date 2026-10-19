//! Group-aware selection and the requests behind the Shapes and Edit menu operations (groups,
//! arrange copies, shape operations, offset lines, attributes, entry/exit points).

use super::request::{CommandRequest, mm};
use openstitch_design::{Design, Vec2};
use serde_json::json;
use std::collections::BTreeSet;

/// The selection after a tool changed it from `old` to `new`: objects newly selected bring the
/// rest of their group, objects deselected take their group with them. With `member_only`
/// (Alt held) the selection is taken as is, so one member can be edited alone.
pub fn group_selection(
    design: &Design,
    old: &BTreeSet<u64>,
    new: BTreeSet<u64>,
    member_only: bool,
) -> BTreeSet<u64> {
    if member_only {
        return new;
    }
    let added: Vec<u64> = new.difference(old).copied().collect();
    let removed: Vec<u64> = old.difference(&new).copied().collect();
    let mut out = new;
    out.extend(design.with_groups(&added));
    for id in design.with_groups(&removed) {
        out.remove(&id);
    }
    out
}

/// The selection in sewing order.
pub fn in_sewing_order(design: &Design, ids: &BTreeSet<u64>) -> Vec<u64> {
    design
        .objects
        .iter()
        .filter(|o| ids.contains(&o.id))
        .map(|o| o.id)
        .collect()
}

/// Whether any selected object belongs to a group.
pub fn any_grouped(design: &Design, ids: &BTreeSet<u64>) -> bool {
    design
        .objects
        .iter()
        .any(|o| ids.contains(&o.id) && o.group.is_some())
}

pub fn group(ids: &BTreeSet<u64>) -> CommandRequest {
    CommandRequest::new("object.group", json!({ "ids": CommandRequest::ids(ids) }))
}

pub fn ungroup(ids: &BTreeSet<u64>) -> CommandRequest {
    CommandRequest::new("object.ungroup", json!({ "ids": CommandRequest::ids(ids) }))
}

/// A preset shape or arc of `size` mm centred on `center` (model units).
pub fn insert_preset(kind: &str, center: Vec2, size: f64) -> CommandRequest {
    let (cx, cy) = (mm(center.x), mm(center.y));
    match kind {
        "arc" | "chord" | "fan" => CommandRequest::new(
            "shape.arc",
            json!({ "cx": cx, "cy": cy, "rx": size / 2.0, "start": 180, "sweep": 180, "mode": kind }),
        ),
        "roundedRectangle" => CommandRequest::new(
            "shape.rectangle",
            json!({
                "x": cx - size / 2.0, "y": cy - size / 3.0, "width": size,
                "height": size * 2.0 / 3.0, "cornerRadius": size / 8.0,
            }),
        ),
        "hexagon" => CommandRequest::new(
            "shape.preset",
            json!({ "kind": "polygon", "sides": 6, "cx": cx, "cy": cy, "width": size }),
        ),
        _ => CommandRequest::new(
            "shape.preset",
            json!({ "kind": kind, "cx": cx, "cy": cy, "width": size }),
        ),
    }
}

/// Presets offered in the Shapes menu: (menu label, kind for [`insert_preset`]).
pub const PRESETS: [(&str, &str); 13] = [
    ("Rounded rectangle", "roundedRectangle"),
    ("Arc", "arc"),
    ("Arc and string", "chord"),
    ("Fan", "fan"),
    ("Triangle", "triangle"),
    ("Diamond", "diamond"),
    ("Hexagon", "hexagon"),
    ("Star", "star"),
    ("Heart", "heart"),
    ("Arrow", "arrow"),
    ("Cross", "cross"),
    ("Crescent", "crescent"),
    ("Teardrop", "teardrop"),
];

/// Settings of the Floral Pattern dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct FloralForm {
    pub density: u32,
    pub pattern: u32,
}

impl Default for FloralForm {
    fn default() -> Self {
        FloralForm {
            density: 3,
            pattern: 0,
        }
    }
}

impl FloralForm {
    pub fn request(&self, id: u64) -> CommandRequest {
        CommandRequest::new(
            "shape.floral",
            json!({ "id": id, "density": self.density, "pattern": self.pattern }),
        )
    }
}

/// Decorative frames offered in the Shapes menu: (label, kind).
pub const FRAMES: [(&str, &str); 5] = [
    ("Scallops", "scallop"),
    ("Waves", "wave"),
    ("Zigzag", "zigzag"),
    ("Scalloped oval", "scallopOval"),
    ("Corner scrolls", "corners"),
];

/// A decorative frame around `ids`, or at the page centre when empty.
pub fn decorative(kind: &str, ids: &BTreeSet<u64>, keep_aspect: bool) -> CommandRequest {
    let mut p = json!({ "kind": kind, "keepAspect": keep_aspect });
    if !ids.is_empty() {
        p["ids"] = CommandRequest::ids(ids);
    }
    CommandRequest::new("shape.decorative", p)
}

/// Embossing/engraving effects offered in the Shapes menu: (label, effect id).
pub const EMBOSS: [(&str, &str); 3] = [
    ("Engraved Line", "engravedLine"),
    ("Engraved Region", "engravedRegion"),
    ("Embossed Region", "embossedRegion"),
];

/// `shape.emboss` for a selection of two: the first in sewing order is the target, the last
/// the tool shape on top.
pub fn emboss(design: &Design, ids: &BTreeSet<u64>, effect: &str) -> Option<CommandRequest> {
    let order = in_sewing_order(design, ids);
    if order.len() != 2 {
        return None;
    }
    Some(CommandRequest::new(
        "shape.emboss",
        json!({ "id": order[0], "tool": order[1], "effect": effect }),
    ))
}

/// `object.addStamp` of `pattern` centred at `at` (model units), `size` mm.
pub fn add_stamp(id: u64, pattern: &str, effect: &str, at: Vec2, size: f64) -> CommandRequest {
    CommandRequest::new(
        "object.addStamp",
        json!({ "id": id, "pattern": pattern, "effect": effect, "x": mm(at.x), "y": mm(at.y), "size": size }),
    )
}

pub fn clear_stamps(id: u64) -> CommandRequest {
    CommandRequest::new("object.deleteStamp", json!({ "id": id, "all": true }))
}

/// Stamp outlines of object `id` with whether each is embossed (else engraved).
pub fn stamp_outlines(design: &Design, id: u64) -> Vec<(bool, Vec<Vec<Vec2>>)> {
    let Some(r) = design
        .index_of(id)
        .and_then(|i| design.objects[i].region.as_ref())
    else {
        return Vec::new();
    };
    r.stamps
        .iter()
        .map(|s| {
            (
                s.effect == openstitch_design::stamp::StampEffect::EmbossedRegion,
                s.contours.iter().map(|c| c.path()).collect(),
            )
        })
        .collect()
}

pub fn merge(ids: &BTreeSet<u64>) -> CommandRequest {
    CommandRequest::new("shape.merge", json!({ "ids": CommandRequest::ids(ids) }))
}

pub fn remove_overlap(ids: &BTreeSet<u64>) -> CommandRequest {
    CommandRequest::new(
        "shape.removeOverlap",
        json!({ "ids": CommandRequest::ids(ids) }),
    )
}

/// Boolean combination; the first shape in sewing order is the subject.
pub fn combine(design: &Design, ids: &BTreeSet<u64>, op: &str) -> CommandRequest {
    CommandRequest::new(
        "shape.combine",
        json!({ "ids": in_sewing_order(design, ids), "op": op }),
    )
}

/// `shape.split` of `id` along a cut outline drawn as `points` (mm), e.g. with the Polygon tool.
pub fn split(id: u64, points: serde_json::Value) -> CommandRequest {
    CommandRequest::new("shape.split", json!({ "id": id, "points": points }))
}

/// Settings of the Offset Lines dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetForm {
    pub inward: bool,
    pub spacing: f64,
    pub count: u32,
    pub rounded: bool,
}

impl Default for OffsetForm {
    fn default() -> Self {
        OffsetForm {
            inward: false,
            spacing: 2.0,
            count: 1,
            rounded: false,
        }
    }
}

impl OffsetForm {
    pub fn request(&self, id: u64) -> CommandRequest {
        CommandRequest::new(
            "shape.offsetLines",
            json!({
                "id": id,
                "direction": if self.inward { "inward" } else { "outward" },
                "spacing": self.spacing,
                "count": self.count,
                "roundedCorners": self.rounded,
            }),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrangeMode {
    MirrorHorizontal,
    MirrorVertical,
    MirrorFourWay,
    Circle,
    Matrix,
}

impl ArrangeMode {
    pub const ALL: [ArrangeMode; 5] = [
        ArrangeMode::MirrorHorizontal,
        ArrangeMode::MirrorVertical,
        ArrangeMode::MirrorFourWay,
        ArrangeMode::Circle,
        ArrangeMode::Matrix,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ArrangeMode::MirrorHorizontal => "Mirror horizontal",
            ArrangeMode::MirrorVertical => "Mirror vertical",
            ArrangeMode::MirrorFourWay => "Mirror four-way",
            ArrangeMode::Circle => "Circle",
            ArrangeMode::Matrix => "Matrix",
        }
    }

    fn id(self) -> &'static str {
        match self {
            ArrangeMode::MirrorHorizontal => "mirrorHorizontal",
            ArrangeMode::MirrorVertical => "mirrorVertical",
            ArrangeMode::MirrorFourWay => "mirrorFourWay",
            ArrangeMode::Circle => "circle",
            ArrangeMode::Matrix => "matrix",
        }
    }
}

/// Settings of the Arrange Copies dialog (mm).
#[derive(Clone, Debug, PartialEq)]
pub struct ArrangeForm {
    pub mode: ArrangeMode,
    pub spacing: f64,
    pub count: u32,
    pub radius: f64,
    pub rotate: bool,
    pub rows: u32,
    pub columns: u32,
    pub spacing_x: f64,
    pub spacing_y: f64,
    pub merge: bool,
    pub optimize: bool,
}

impl Default for ArrangeForm {
    fn default() -> Self {
        ArrangeForm {
            mode: ArrangeMode::MirrorHorizontal,
            spacing: 0.0,
            count: 6,
            radius: 30.0,
            rotate: true,
            rows: 2,
            columns: 2,
            spacing_x: 2.0,
            spacing_y: 2.0,
            merge: false,
            optimize: false,
        }
    }
}

impl ArrangeForm {
    pub fn request(&self, ids: &BTreeSet<u64>) -> CommandRequest {
        let mut p = json!({
            "ids": CommandRequest::ids(ids),
            "mode": self.mode.id(),
            "mergeOverlapping": self.merge,
            "optimizeOrder": self.optimize,
        });
        match self.mode {
            ArrangeMode::Circle => {
                p["count"] = json!(self.count);
                p["radius"] = json!(self.radius);
                p["rotate"] = json!(self.rotate);
            }
            ArrangeMode::Matrix => {
                p["rows"] = json!(self.rows);
                p["columns"] = json!(self.columns);
                p["spacingX"] = json!(self.spacing_x);
                p["spacingY"] = json!(self.spacing_y);
            }
            _ => p["spacing"] = json!(self.spacing),
        }
        CommandRequest::new("object.arrangeCopies", p)
    }
}

pub fn paste_attributes(from: u64, ids: &BTreeSet<u64>) -> CommandRequest {
    let targets: Vec<u64> = ids.iter().copied().filter(|&i| i != from).collect();
    CommandRequest::new(
        "object.pasteAttributes",
        json!({ "from": from, "ids": targets }),
    )
}

/// Which entry/exit point a canvas click sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryExit {
    RegionEntry,
    RegionExit,
    LineEntry,
}

impl EntryExit {
    pub fn key(self) -> &'static str {
        match self {
            EntryExit::RegionEntry => "regionEntry",
            EntryExit::RegionExit => "regionExit",
            EntryExit::LineEntry => "lineEntry",
        }
    }

    /// PE-DESIGN's marker labels.
    pub fn label(self) -> &'static str {
        match self {
            EntryExit::RegionEntry => "R1",
            EntryExit::RegionExit => "R2",
            EntryExit::LineEntry => "L1",
        }
    }

    pub fn set(self, id: u64, at: Vec2) -> CommandRequest {
        let mut p = json!({ "id": id });
        p[self.key()] = json!([mm(at.x), mm(at.y)]);
        CommandRequest::new("object.setEntryExit", p)
    }
}

/// What a canvas click places on the selected shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Point(EntryExit),
    /// Centre of concentric circle, radial and spiral stitches.
    Center,
    /// Pattern reference point of programmable, motif, cross, net and decorative fills.
    Reference,
}

impl Pick {
    pub fn label(self) -> &'static str {
        match self {
            Pick::Point(p) => p.label(),
            Pick::Center => "the centre point",
            Pick::Reference => "the reference point",
        }
    }

    pub fn request(self, id: u64, at: Vec2) -> CommandRequest {
        let point = json!([mm(at.x), mm(at.y)]);
        match self {
            Pick::Point(p) => p.set(id, at),
            Pick::Center => {
                CommandRequest::new("object.setCenterPoint", json!({ "id": id, "point": point }))
            }
            Pick::Reference => CommandRequest::new(
                "object.setReferencePoint",
                json!({ "id": id, "point": point }),
            ),
        }
    }
}

/// `object.setDirectionCurve` from a `shape.path` request drawn with the Pen tool (open paths
/// only).
pub fn direction_curve(id: u64, path: &CommandRequest) -> Option<CommandRequest> {
    if path.command != "shape.path" || path.params["closed"] != json!(false) {
        return None;
    }
    let mut p = json!({ "id": id, "points": path.params["points"].clone() });
    if let Some(c) = path.params.get("curves") {
        p["curves"] = c.clone();
    }
    Some(CommandRequest::new("object.setDirectionCurve", p))
}

/// `object.setDirectionCurve` back to a straight direction.
pub fn straight_direction(id: u64) -> CommandRequest {
    CommandRequest::new(
        "object.setDirectionCurve",
        json!({ "id": id, "points": null }),
    )
}

/// Centre point, reference point and direction curve of object `id`, for drawing.
pub fn direction_markers(
    design: &Design,
    id: u64,
) -> (Option<Vec2>, Option<Vec2>, Option<Vec<Vec2>>) {
    let r = design
        .index_of(id)
        .and_then(|i| design.objects[i].region.as_ref());
    (
        r.and_then(|r| r.center),
        r.and_then(|r| r.reference),
        r.and_then(|r| r.direction_curve.as_ref().map(|c| c.path())),
    )
}

pub fn clear_entry_exit(id: u64) -> CommandRequest {
    CommandRequest::new(
        "object.setEntryExit",
        json!({ "id": id, "regionEntry": null, "regionExit": null, "lineEntry": null }),
    )
}

/// The entry/exit points of object `id` to draw: (which, position in model units).
pub fn entry_exit_markers(design: &Design, id: u64) -> Vec<(EntryExit, Vec2)> {
    let Some(o) = design.index_of(id).map(|i| &design.objects[i]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(r) = &o.region {
        out.extend(r.entry.map(|p| (EntryExit::RegionEntry, p)));
        out.extend(r.exit.map(|p| (EntryExit::RegionExit, p)));
    }
    if let Some(l) = &o.line {
        out.extend(l.entry.map(|p| (EntryExit::LineEntry, p)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_design::v2;
    use openstitch_engine::Session;

    fn session_with(n: usize) -> Session {
        let mut s = Session::new(std::env::temp_dir());
        for k in 0..n {
            s.execute(
                "shape.rectangle",
                Some(&json!({"x": k * 20, "y": 0, "width": 15, "height": 15})),
            )
            .expect("rectangle");
        }
        s
    }

    fn run(s: &mut Session, r: &CommandRequest) -> serde_json::Value {
        s.execute(r.command, Some(&r.params))
            .unwrap_or_else(|e| panic!("{}: {e}", r.command))
    }

    #[test]
    fn selecting_a_member_selects_its_group_unless_alt() {
        let mut s = session_with(3);
        run(&mut s, &group(&BTreeSet::from([1, 2])));
        let d = s.design();
        let none = BTreeSet::new();
        assert_eq!(
            group_selection(d, &none, BTreeSet::from([1]), false),
            BTreeSet::from([1, 2])
        );
        assert_eq!(
            group_selection(d, &none, BTreeSet::from([1]), true),
            BTreeSet::from([1])
        );
        // Shift-click removing a member removes the whole group.
        let all = BTreeSet::from([1, 2, 3]);
        assert_eq!(
            group_selection(d, &all, BTreeSet::from([2, 3]), false),
            BTreeSet::from([3])
        );
        assert!(any_grouped(d, &BTreeSet::from([2])));
        run(&mut s, &ungroup(&BTreeSet::from([2])));
        assert!(!any_grouped(s.design(), &BTreeSet::from([1, 2])));
    }

    #[test]
    fn menu_requests_run_in_the_engine() {
        for (_, kind) in PRESETS {
            let mut s = session_with(0);
            let r = run(&mut s, &insert_preset(kind, v2(500.0, 500.0), 30.0));
            assert!(r["id"].as_u64().is_some(), "{kind}");
        }
        let mut s = session_with(2);
        run(
            &mut s,
            &CommandRequest::move_by(&BTreeSet::from([2]), -100.0, 50.0),
        );
        let both = BTreeSet::from([1, 2]);
        let r = combine(s.design(), &both, "difference");
        run(&mut s, &r);
        assert_eq!(s.design().objects.len(), 1);
        let r = run(
            &mut s,
            &OffsetForm {
                count: 2,
                ..Default::default()
            }
            .request(1),
        );
        let lines = r["id"].as_u64().expect("id");
        run(&mut s, &paste_attributes(1, &BTreeSet::from([1, lines])));
        for mode in ArrangeMode::ALL {
            let form = ArrangeForm {
                mode,
                ..Default::default()
            };
            run(&mut s, &form.request(&BTreeSet::from([1])));
        }
        let mut s = session_with(2);
        run(
            &mut s,
            &CommandRequest::move_by(&BTreeSet::from([2]), -100.0, 50.0),
        );
        run(&mut s, &remove_overlap(&BTreeSet::from([1, 2])));
        run(&mut s, &merge(&BTreeSet::from([1, 2])));
        run(
            &mut s,
            &split(2, json!([[-5, -5], [20, -5], [20, 10], [-5, 10]])),
        );
        assert_eq!(s.design().objects.len(), 2);
    }

    #[test]
    fn entry_exit_points_set_and_listed() {
        let mut s = session_with(1);
        run(&mut s, &EntryExit::RegionEntry.set(1, v2(-20.0, 50.0)));
        run(&mut s, &EntryExit::RegionExit.set(1, v2(160.0, 50.0)));
        let m = entry_exit_markers(s.design(), 1);
        assert_eq!(
            m,
            vec![
                (EntryExit::RegionEntry, v2(0.0, 50.0)),
                (EntryExit::RegionExit, v2(150.0, 50.0)),
            ]
        );
        // No line sewing: setting L1 fails cleanly.
        let r = EntryExit::LineEntry.set(1, v2(0.0, 0.0));
        assert!(s.execute(r.command, Some(&r.params)).is_err());
        run(&mut s, &clear_entry_exit(1));
        assert!(entry_exit_markers(s.design(), 1).is_empty());
    }

    #[test]
    fn centre_reference_and_curve_requests() {
        let mut s = session_with(1);
        run(
            &mut s,
            &CommandRequest::new("object.setRegion", json!({"id": 1, "type": "radial"})),
        );
        run(&mut s, &Pick::Center.request(1, v2(40.0, 50.0)));
        assert_eq!(direction_markers(s.design(), 1).0, Some(v2(40.0, 50.0)));
        let r = Pick::Reference.request(1, v2(0.0, 0.0));
        assert!(
            s.execute(r.command, Some(&r.params)).is_err(),
            "radial has no reference"
        );
        run(
            &mut s,
            &CommandRequest::new("object.setRegion", json!({"id": 1, "type": "fill"})),
        );
        let path = CommandRequest::new(
            "shape.path",
            json!({"points": [[-1, 7], [16, 7]], "closed": false, "curves": [[[4, 0], [11, 0]]]}),
        );
        run(&mut s, &direction_curve(1, &path).expect("open path"));
        assert!(
            direction_markers(s.design(), 1)
                .2
                .is_some_and(|c| c.len() > 2)
        );
        let closed = CommandRequest::new(
            "shape.path",
            json!({"points": [[0, 0], [5, 5]], "closed": true}),
        );
        assert!(direction_curve(1, &closed).is_none());
        run(&mut s, &straight_direction(1));
        assert!(direction_markers(s.design(), 1).2.is_none());
    }

    #[test]
    fn floral_and_frame_requests() {
        let mut s = session_with(1);
        let r = run(&mut s, &FloralForm::default().request(1));
        assert!(r["id"].as_u64().is_some());
        for (_, kind) in FRAMES {
            run(&mut s, &decorative(kind, &BTreeSet::from([1]), true));
            run(&mut s, &decorative(kind, &BTreeSet::new(), false));
        }
        assert_eq!(s.design().objects.len(), 2 + 2 * FRAMES.len());
    }

    #[test]
    fn emboss_and_stamp_requests() {
        let mut s = session_with(1);
        run(
            &mut s,
            &CommandRequest::new("shape.ellipse", json!({"cx": 7, "cy": 7, "rx": 4})),
        );
        assert!(emboss(s.design(), &BTreeSet::from([1]), "engravedLine").is_none());
        let r = emboss(s.design(), &BTreeSet::from([2, 1]), "embossedRegion").expect("two");
        assert_eq!(r.params["id"], json!(1));
        run(&mut s, &r);
        run(
            &mut s,
            &add_stamp(1, "heart", "engravedLine", v2(70.0, 70.0), 5.0),
        );
        let outlines = stamp_outlines(s.design(), 1);
        assert_eq!(outlines.len(), 2);
        assert!(outlines[0].0 && !outlines[1].0);
        run(&mut s, &clear_stamps(1));
        assert!(stamp_outlines(s.design(), 1).is_empty());
    }
}
