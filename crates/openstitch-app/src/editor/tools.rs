use super::handles::{Handle, SelectionHandles};
use super::hit::{bounds_of, object_at, objects_in, rect_from};
use super::pen::{PenNode, is_valid, path_request, pen_contour};
use super::points::{
    Control, PointHit, PointRef, point_at, smooth_curve, with_control, with_vertex,
};
use super::request::{CommandRequest, mm};
use openstitch_design::{Contour, Design, DesignObject, ObjectKind, Rect, Transform, Vec2, v2};
use serde_json::json;
use std::collections::BTreeSet;

pub mod needles;
pub use needles::NeedleOverlay;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Rectangle,
    Ellipse,
    Polygon,
    /// Point editing: move vertices and curve handles of the selected shape.
    Point,
    /// Bézier pen: click for corners, drag for smooth points.
    Pen,
    /// Manual Punch: click the two ends of each sewing-direction line in turn.
    Punch,
    /// Open Straight Line: click the points of an open line.
    Line,
    /// Pencil (Closed Curve): drag to draw a closed outline freehand.
    Pencil,
    /// Pencil (Open Curve): drag to draw an open line freehand.
    PencilOpen,
    /// Manual Punch Curved Block: like [`Tool::Punch`], the sides curve through the line ends.
    CurvedBlock,
    /// Manual Punch Running: click the points of a running-stitch line.
    Running,
    /// Manual Punch Feed: click the points the frame jumps through without sewing.
    Feed,
}

impl Tool {
    pub const ALL: [Tool; 13] = [
        Tool::Select,
        Tool::Point,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Polygon,
        Tool::Pen,
        Tool::Punch,
        Tool::Line,
        Tool::Pencil,
        Tool::PencilOpen,
        Tool::CurvedBlock,
        Tool::Running,
        Tool::Feed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Rectangle => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Polygon => "Polygon",
            Tool::Point => "Point",
            Tool::Pen => "Pen",
            Tool::Punch => "Punch",
            Tool::Line => "Line",
            Tool::Pencil => "Pencil",
            Tool::PencilOpen => "OpenPencil",
            Tool::CurvedBlock => "CurvedBlock",
            Tool::Running => "Running",
            Tool::Feed => "Feed",
        }
    }

    /// Single-key shortcut on the canvas, if the tool has one.
    pub fn shortcut(self) -> Option<char> {
        Some(match self {
            Tool::Select => 'v',
            Tool::Rectangle => 'r',
            Tool::Ellipse => 'e',
            Tool::Polygon => 'p',
            Tool::Point => 'a',
            Tool::Pen => 'b',
            Tool::Punch => 'u',
            Tool::Line => 'l',
            _ => return None,
        })
    }

    pub fn from_shortcut(c: char) -> Option<Tool> {
        Self::ALL
            .into_iter()
            .find(|t| t.shortcut() == Some(c.to_ascii_lowercase()))
    }

    /// The Manual Punch tools (they share the Manual Punch tab).
    pub fn is_punch(self) -> bool {
        matches!(
            self,
            Tool::Punch | Tool::CurvedBlock | Tool::Running | Tool::Feed
        )
    }

    /// The freehand Pencil tools.
    pub fn is_pencil(self) -> bool {
        matches!(self, Tool::Pencil | Tool::PencilOpen)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Down,
    Dragged,
    Up,
    Moved,
}

/// A pointer event in model units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub phase: Phase,
    pub location: Vec2,
    /// Constrain: square/circle, axis-locked move, proportional scale, 15° rotate, toggle selection.
    pub shift: bool,
    /// 2 on the second press of a double click.
    pub clicks: u32,
}

impl PointerEvent {
    pub fn new(phase: Phase, location: Vec2) -> Self {
        PointerEvent {
            phase,
            location,
            shift: false,
            clicks: 1,
        }
    }

    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }
}

/// Counts presses so a second press soon after and near the first is a double-click (the
/// canvas sets [`PointerEvent::clicks`] from it).
#[derive(Clone, Copy, Debug, Default)]
pub struct ClickCounter {
    last: Option<(f64, Vec2, u32)>,
}

impl ClickCounter {
    /// Seconds within which presses count together.
    pub const INTERVAL: f64 = 0.4;
    /// Screen points the pointer may travel between them.
    pub const SLOP: f64 = 6.0;

    /// Registers a press at `time` (seconds) and screen position `at`; returns its count.
    pub fn press(&mut self, time: f64, at: Vec2) -> u32 {
        let count = match self.last {
            Some((t, p, n)) if time - t <= Self::INTERVAL && p.distance(at) <= Self::SLOP => n + 1,
            _ => 1,
        };
        self.last = Some((time, at, count));
        count
    }
}

/// What a tool needs to know about the document when it handles an event.
pub struct ToolContext<'a> {
    pub design: &'a Design,
    pub selection: &'a BTreeSet<u64>,
    /// Model units per screen point at the current zoom.
    pub units_per_point: f64,
    /// Whether `object.scale` takes `originX`/`originY` (checked against the command registry).
    pub scale_takes_origin: bool,
}

impl ToolContext<'_> {
    /// Pointer slop in model units.
    pub fn tolerance(&self) -> f64 {
        4.0 * self.units_per_point
    }

    /// Hit radius of vertices and curve handles in model units.
    pub fn point_radius(&self) -> f64 {
        SelectionHandles::RADIUS * 1.4 * self.units_per_point
    }

    /// The shape the Point tool edits: the selection when it is exactly one visible shape.
    pub fn point_target(&self) -> Option<&DesignObject> {
        if self.selection.len() != 1 {
            return None;
        }
        let id = *self.selection.first()?;
        self.design
            .objects
            .iter()
            .find(|o| o.id == id && o.kind == ObjectKind::Shape && o.visible)
    }

    pub fn handles(&self) -> Option<SelectionHandles> {
        if self.selection.is_empty() {
            return None;
        }
        bounds_of(self.design, self.selection).map(|bounds| SelectionHandles {
            bounds,
            units_per_point: self.units_per_point,
        })
    }
}

/// Feedback while a gesture is in progress; the document changes only when it ends.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolPreview {
    Marquee(Rect),
    /// Outline being drawn (open while a polygon is still being clicked in).
    Outline(Contour),
    /// The selection shown transformed.
    Transform(Transform),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToolEffect {
    Select(BTreeSet<u64>),
    /// Commands to run in order; `select_result` selects what the last one created.
    Execute {
        requests: Vec<CommandRequest>,
        select_result: bool,
    },
}

impl ToolEffect {
    fn run(request: CommandRequest, select_result: bool) -> Self {
        ToolEffect::Execute {
            requests: vec![request],
            select_result,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Gesture {
    Pending {
        start: Vec2,
        hit: Option<u64>,
        shift: bool,
    },
    Moving {
        start: Vec2,
    },
    Scaling {
        handle: Handle,
        start: Vec2,
        handles: SelectionHandles,
    },
    Rotating {
        start: Vec2,
        handles: SelectionHandles,
    },
    Marquee {
        start: Vec2,
        shift: bool,
        base: BTreeSet<u64>,
    },
    Drawing {
        start: Vec2,
    },
    /// Point tool: a vertex being dragged; `edited` is the contour once the drag has started.
    Vertex {
        point: PointRef,
        start: Vec2,
        edited: Option<Contour>,
    },
    /// Point tool: a curve handle being dragged.
    Control {
        id: u64,
        contour: usize,
        segment: usize,
        control: Control,
        start: Vec2,
        edited: Option<Contour>,
    },
    /// Pen tool: the anchor just placed, its handles being pulled out.
    PenAnchor {
        index: usize,
    },
    /// Point tool on a stitch pattern: selected needle points being dragged, or a marquee.
    Needles {
        start: Vec2,
        delta: Option<Vec2>,
        marquee: bool,
    },
}

/// What the Point tool shows over the selected shape: its contours (as edited while a drag is
/// in progress) and the selected vertex.
#[derive(Clone, Debug, PartialEq)]
pub struct PointOverlay {
    pub id: u64,
    pub contours: Vec<Contour>,
    pub selected: Option<(usize, usize)>,
}

/// Pointer-driven tool state machine: select / move / scale / rotate / marquee and the
/// rectangle, ellipse, polygon, point and pen tools. The canvas feeds it events and applies
/// its effects; every gesture ends in at most one command group, so it is one Undo.
#[derive(Clone, Debug, Default)]
pub struct ToolController {
    tool: Option<Tool>,
    preview: Option<ToolPreview>,
    gesture: Option<Gesture>,
    polygon: Vec<Vec2>,
    pen: Vec<PenNode>,
    /// Manual Punch: direction-line ends clicked so far (left, right, left, right, …), or
    /// the points of a running line or feed.
    punch: Vec<Vec2>,
    /// Pencil: the pointer trail of the stroke being drawn.
    pencil: Vec<Vec2>,
    /// Edge radius (mm) of the rectangles the Rectangle tool draws.
    corner_radius: f64,
    /// Point tool: the selected vertex.
    point: Option<PointRef>,
    /// Last pointer location (for the Point tool's segment shortcuts).
    hover: Option<Vec2>,
    /// Point tool on a stitch pattern: selected needle point indexes, and whose they are.
    needles: BTreeSet<usize>,
    needle_owner: Option<u64>,
    /// Snap to grid: the grid step in model units.
    snap: Option<f64>,
}

/// Drags shorter than this (points) are clicks.
pub const DRAG_THRESHOLD: f64 = 3.0;
/// Shapes smaller than this (model units, 0.5 mm) are ignored.
pub const MINIMUM_SIZE: f64 = 5.0;

impl ToolController {
    pub fn tool(&self) -> Tool {
        self.tool.unwrap_or(Tool::Select)
    }

    pub fn preview(&self) -> Option<&ToolPreview> {
        self.preview.as_ref()
    }

    pub fn polygon(&self) -> &[Vec2] {
        &self.polygon
    }

    pub fn pen(&self) -> &[PenNode] {
        &self.pen
    }

    pub fn is_busy(&self) -> bool {
        self.gesture.is_some()
            || !self.polygon.is_empty()
            || !self.pen.is_empty()
            || !self.punch.is_empty()
            || !self.pencil.is_empty()
    }

    /// Edge radius (mm) for new rectangles; 0 = square corners.
    pub fn corner_radius(&self) -> f64 {
        self.corner_radius
    }

    pub fn set_corner_radius(&mut self, mm: f64) {
        self.corner_radius = if mm.is_finite() {
            mm.clamp(0.0, 1000.0)
        } else {
            0.0
        };
    }

    /// Snaps drawing, moving and point editing to a grid of `step` model units (`None`: off).
    pub fn set_snap(&mut self, step: Option<f64>) {
        self.snap = step.filter(|s| *s > 0.0);
    }

    pub fn snap(&self) -> Option<f64> {
        self.snap
    }

    /// `p` on the nearest grid crossing when snapping is on.
    pub fn snapped(&self, p: Vec2) -> Vec2 {
        self.snap.map_or(p, |s| snap_to_grid(p, s))
    }

    /// The move for a drag from `start` to `p`; with snapping the selection's top-left corner
    /// lands on the grid (an axis-locked move stays on its axis).
    fn move_by(&self, start: Vec2, p: Vec2, axis_lock: bool, ctx: &ToolContext) -> Vec2 {
        let d = move_delta(start, p, axis_lock);
        let (Some(step), Some(b)) = (self.snap, bounds_of(ctx.design, ctx.selection)) else {
            return d;
        };
        let corner = v2(b.min_x, b.min_y);
        let to = snap_to_grid(corner + d, step) - corner;
        v2(
            if d.x == 0.0 && axis_lock { 0.0 } else { to.x },
            if d.y == 0.0 && axis_lock { 0.0 } else { to.y },
        )
    }

    pub fn set_tool(&mut self, t: Tool) {
        self.cancel();
        self.point = None;
        self.tool = Some(t);
    }

    /// Escape: abandon the gesture, polygon or pen path in progress.
    pub fn cancel(&mut self) {
        self.gesture = None;
        self.polygon.clear();
        self.pen.clear();
        self.punch.clear();
        self.pencil.clear();
        self.preview = None;
    }

    pub fn handle(&mut self, mut e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        self.hover = Some(e.location);
        // Drawing tools place every point on the grid.
        if !matches!(self.tool(), Tool::Select | Tool::Point) && !self.tool().is_pencil() {
            e.location = self.snapped(e.location);
        }
        match self.tool() {
            Tool::Select => self.select_tool(e, ctx),
            Tool::Rectangle | Tool::Ellipse => self.drag_shape_tool(e),
            Tool::Polygon | Tool::Line => self.polygon_tool(e, ctx),
            Tool::Point => self.point_tool(e, ctx),
            Tool::Pen => self.pen_tool(e, ctx),
            Tool::Pencil | Tool::PencilOpen => self.pencil_tool(e, ctx),
            Tool::Punch | Tool::CurvedBlock | Tool::Running | Tool::Feed => self.punch_tool(e, ctx),
        }
    }

    /// Return (or a double-click outside the Point tool): close the polygon, finish the pen
    /// path open.
    pub fn finish(&mut self) -> Vec<ToolEffect> {
        match self.tool() {
            Tool::Polygon | Tool::Line => self.finish_polygon(),
            Tool::Pen => self.finish_pen(false),
            t if t.is_punch() => self.finish_punch(),
            _ => vec![],
        }
    }

    // MARK: point tool

    /// The selected vertex, if it still exists on the Point tool's target.
    pub fn selected_point(&self, ctx: &ToolContext) -> Option<PointRef> {
        let p = self.point?;
        let o = ctx.point_target().filter(|o| o.id == p.id)?;
        (p.index < o.contours.get(p.contour)?.points.len()).then_some(p)
    }

    /// The vertices and handles to draw while the Point tool is active.
    pub fn point_overlay(&self, ctx: &ToolContext) -> Option<PointOverlay> {
        if self.tool() != Tool::Point {
            return None;
        }
        let o = ctx.point_target()?;
        let mut contours = o.contours.clone();
        match &self.gesture {
            Some(Gesture::Vertex {
                point,
                edited: Some(c),
                ..
            }) if point.id == o.id => contours[point.contour] = c.clone(),
            Some(Gesture::Control {
                id,
                contour,
                edited: Some(c),
                ..
            }) if *id == o.id => contours[*contour] = c.clone(),
            _ => {}
        }
        Some(PointOverlay {
            id: o.id,
            contours,
            selected: self.selected_point(ctx).map(|p| (p.contour, p.index)),
        })
    }

    fn point_tool(&mut self, e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        if let Some(o) = ctx.needle_target() {
            return self.needle_tool(e, ctx, o);
        }
        let p = e.location;
        match (e.phase, self.gesture.clone()) {
            (Phase::Down, _) => {
                let target = ctx.point_target();
                let hit = target.and_then(|o| {
                    point_at(o, p, ctx.point_radius(), ctx.tolerance()).map(|h| (o, h))
                });
                match hit {
                    Some((o, PointHit::Vertex { contour, index })) => {
                        let point = PointRef {
                            id: o.id,
                            contour,
                            index,
                        };
                        self.point = Some(point);
                        self.gesture = Some(Gesture::Vertex {
                            point,
                            start: p,
                            edited: None,
                        });
                        vec![]
                    }
                    Some((
                        o,
                        PointHit::Handle {
                            contour,
                            segment,
                            control,
                        },
                    )) => {
                        self.gesture = Some(Gesture::Control {
                            id: o.id,
                            contour,
                            segment,
                            control,
                            start: p,
                            edited: None,
                        });
                        vec![]
                    }
                    Some((
                        o,
                        PointHit::Segment {
                            contour,
                            segment,
                            t,
                        },
                    )) => {
                        if e.clicks < 2 || !(0.001..=0.999).contains(&t) {
                            self.point = None;
                            return vec![];
                        }
                        self.point = Some(PointRef {
                            id: o.id,
                            contour,
                            index: segment + 1,
                        });
                        vec![ToolEffect::run(
                            CommandRequest::new(
                                "object.insertPoint",
                                json!({ "id": o.id, "contour": contour, "segment": segment, "t": (t * 1000.0).round() / 1000.0 }),
                            ),
                            false,
                        )]
                    }
                    None => {
                        self.point = None;
                        match object_at(ctx.design, p, ctx.tolerance()) {
                            Some(id) if ctx.selection.len() == 1 && ctx.selection.contains(&id) => {
                                vec![]
                            }
                            Some(id) => vec![ToolEffect::Select(BTreeSet::from([id]))],
                            None => vec![ToolEffect::Select(BTreeSet::new())],
                        }
                    }
                }
            }
            (Phase::Dragged, Some(Gesture::Vertex { point, start, .. })) => {
                let Some(c) = ctx
                    .point_target()
                    .and_then(|o| o.contours.get(point.contour))
                    .filter(|c| point.index < c.points.len())
                else {
                    return vec![];
                };
                let edited =
                    (p.distance(start) >= DRAG_THRESHOLD * ctx.units_per_point).then(|| {
                        let d = move_delta(start, p, e.shift);
                        with_vertex(c, point.index, self.snapped(c.points[point.index] + d))
                    });
                self.gesture = Some(Gesture::Vertex {
                    point,
                    start,
                    edited,
                });
                vec![]
            }
            (
                Phase::Dragged,
                Some(Gesture::Control {
                    id,
                    contour,
                    segment,
                    control,
                    start,
                    ..
                }),
            ) => {
                let Some(c) = ctx.point_target().and_then(|o| o.contours.get(contour)) else {
                    return vec![];
                };
                let edited = (p.distance(start) >= DRAG_THRESHOLD * ctx.units_per_point)
                    .then(|| with_control(c, segment, control, self.snapped(p)));
                self.gesture = Some(Gesture::Control {
                    id,
                    contour,
                    segment,
                    control,
                    start,
                    edited,
                });
                vec![]
            }
            (Phase::Up, Some(g)) => {
                self.gesture = None;
                match g {
                    Gesture::Vertex {
                        point,
                        edited: Some(c),
                        ..
                    } => {
                        let v = c.points[point.index];
                        vec![ToolEffect::run(
                            CommandRequest::new(
                                "object.setPoint",
                                json!({ "id": point.id, "contour": point.contour, "index": point.index, "x": mm(v.x), "y": mm(v.y) }),
                            ),
                            false,
                        )]
                    }
                    Gesture::Control {
                        id,
                        contour,
                        segment,
                        edited: Some(c),
                        ..
                    } => match c.curve(segment) {
                        Some(z) => vec![ToolEffect::run(
                            CommandRequest::new(
                                "object.setSegment",
                                json!({ "id": id, "contour": contour, "segment": segment, "curve": [[mm(z.c1.x), mm(z.c1.y)], [mm(z.c2.x), mm(z.c2.y)]] }),
                            ),
                            false,
                        )],
                        None => vec![],
                    },
                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }

    /// Delete with the Point tool: removes the selected vertex. `None` when no vertex is
    /// selected (Delete then removes objects as usual).
    pub fn delete_point(&mut self, ctx: &ToolContext) -> Option<Vec<ToolEffect>> {
        if self.tool() != Tool::Point {
            return None;
        }
        if let Some(effects) = self.delete_needles(ctx) {
            return Some(effects);
        }
        let p = self.selected_point(ctx)?;
        self.point = None;
        Some(vec![ToolEffect::run(
            CommandRequest::new(
                "object.deletePoint",
                json!({ "id": p.id, "contour": p.contour, "index": p.index }),
            ),
            false,
        )])
    }

    /// Point tool shortcut: makes the segment under the pointer a smooth curve (`curved`) or
    /// straight. Nothing happens when it already is, or no segment is near the pointer.
    pub fn set_segment_under_pointer(
        &mut self,
        curved: bool,
        ctx: &ToolContext,
    ) -> Vec<ToolEffect> {
        if self.tool() != Tool::Point || self.gesture.is_some() {
            return vec![];
        }
        let (Some(o), Some(p)) = (ctx.point_target(), self.hover) else {
            return vec![];
        };
        let Some((contour, segment)) = o
            .contours
            .iter()
            .enumerate()
            .filter_map(|(ci, c)| super::points::nearest_segment(c, p).map(|(s, _, d)| (ci, s, d)))
            .filter(|&(.., d)| d <= 2.0 * ctx.tolerance())
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(ci, s, _)| (ci, s))
        else {
            return vec![];
        };
        let c = &o.contours[contour];
        if c.curve(segment).is_some() == curved {
            return vec![];
        }
        let params = if curved {
            let z = smooth_curve(c, segment);
            json!({ "id": o.id, "contour": contour, "segment": segment, "curve": [[mm(z.c1.x), mm(z.c1.y)], [mm(z.c2.x), mm(z.c2.y)]] })
        } else {
            json!({ "id": o.id, "contour": contour, "segment": segment, "straight": true })
        };
        vec![ToolEffect::run(
            CommandRequest::new("object.setSegment", params),
            false,
        )]
    }

    // MARK: pen tool

    /// Anchor/handle pairs of the pen anchor being placed (both handles once dragged).
    pub fn pen_handles(&self) -> Vec<(Vec2, Vec2)> {
        let Some(n) = self.pen.last() else {
            return vec![];
        };
        [n.out, n.incoming()]
            .into_iter()
            .flatten()
            .map(|h| (n.at, h))
            .collect()
    }

    fn pen_preview(&mut self, cursor: Option<Vec2>) {
        let mut nodes = self.pen.clone();
        nodes.extend(cursor.map(PenNode::corner));
        self.preview =
            (!nodes.is_empty()).then(|| ToolPreview::Outline(pen_contour(&nodes, false)));
    }

    /// Click adds a corner, click-drag pulls out symmetric handles, clicking the first anchor
    /// closes the path.
    fn pen_tool(&mut self, e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        let p = e.location;
        match (e.phase, self.gesture.clone()) {
            (Phase::Down, _) => {
                if self.pen.len() >= 2 && p.distance(self.pen[0].at) <= 2.0 * ctx.tolerance() {
                    return self.finish_pen(true);
                }
                if self
                    .pen
                    .last()
                    .is_some_and(|last| last.at.distance(p) <= ctx.tolerance() / 2.0)
                {
                    return vec![];
                }
                self.pen.push(PenNode::corner(p));
                self.gesture = Some(Gesture::PenAnchor {
                    index: self.pen.len() - 1,
                });
                self.pen_preview(None);
                vec![]
            }
            (Phase::Dragged, Some(Gesture::PenAnchor { index })) => {
                if let Some(n) = self.pen.get_mut(index) {
                    n.out = (p.distance(n.at) >= DRAG_THRESHOLD * ctx.units_per_point).then_some(p);
                }
                self.pen_preview(None);
                vec![]
            }
            (Phase::Up, Some(Gesture::PenAnchor { .. })) => {
                self.gesture = None;
                vec![]
            }
            (Phase::Moved, _) if !self.pen.is_empty() => {
                self.pen_preview(Some(p));
                vec![]
            }
            _ => vec![],
        }
    }

    /// Ends the pen path as one `shape.path` (closed or open); too few anchors are discarded.
    pub fn finish_pen(&mut self, closed: bool) -> Vec<ToolEffect> {
        let nodes = std::mem::take(&mut self.pen);
        self.preview = None;
        self.gesture = None;
        if self.tool() != Tool::Pen || !is_valid(&nodes, closed) {
            return vec![];
        }
        vec![ToolEffect::run(path_request(&nodes, closed), true)]
    }

    fn select_tool(&mut self, e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        let p = e.location;
        match (e.phase, self.gesture.clone()) {
            (Phase::Down, _) => {
                if let Some((handles, h)) = ctx
                    .handles()
                    .and_then(|hs| hs.handle_at(p).map(|h| (hs, h)))
                {
                    self.gesture = Some(if h == Handle::Rotate {
                        Gesture::Rotating { start: p, handles }
                    } else {
                        Gesture::Scaling {
                            handle: h,
                            start: p,
                            handles,
                        }
                    });
                    return vec![];
                }
                let hit = object_at(ctx.design, p, ctx.tolerance());
                self.gesture = Some(Gesture::Pending {
                    start: p,
                    hit,
                    shift: e.shift,
                });
                match hit {
                    Some(id) if e.shift => {
                        let mut s = ctx.selection.clone();
                        if !s.remove(&id) {
                            s.insert(id);
                        }
                        vec![ToolEffect::Select(s)]
                    }
                    Some(id) if !ctx.selection.contains(&id) => {
                        vec![ToolEffect::Select(BTreeSet::from([id]))]
                    }
                    Some(_) => vec![],
                    None if e.shift => vec![],
                    None => vec![ToolEffect::Select(BTreeSet::new())],
                }
            }
            (Phase::Dragged, Some(Gesture::Pending { start, hit, shift })) => {
                if p.distance(start) < DRAG_THRESHOLD * ctx.units_per_point {
                    return vec![];
                }
                self.gesture = Some(if hit.is_some() && !shift && !ctx.selection.is_empty() {
                    Gesture::Moving { start }
                } else {
                    Gesture::Marquee {
                        start,
                        shift,
                        base: if shift {
                            ctx.selection.clone()
                        } else {
                            BTreeSet::new()
                        },
                    }
                });
                self.select_tool(e, ctx)
            }
            (Phase::Dragged, Some(Gesture::Moving { start })) => {
                let d = self.move_by(start, p, e.shift, ctx);
                self.preview = Some(ToolPreview::Transform(Transform::translation(d.x, d.y)));
                vec![]
            }
            (
                Phase::Dragged,
                Some(Gesture::Scaling {
                    handle,
                    start,
                    handles,
                }),
            ) => {
                let s = handles.scale(handle, start, p, e.shift);
                self.preview = Some(ToolPreview::Transform(Transform::scaling(
                    s.sx, s.sy, s.origin,
                )));
                vec![]
            }
            (Phase::Dragged, Some(Gesture::Rotating { start, handles })) => {
                let deg = handles.rotation(start, p, e.shift);
                self.preview = Some(ToolPreview::Transform(Transform::rotation(
                    deg,
                    handles.bounds.center(),
                )));
                vec![]
            }
            (Phase::Dragged, Some(Gesture::Marquee { start, shift, base })) => {
                let r = rect_from(start, p);
                self.preview = Some(ToolPreview::Marquee(r));
                let inside = objects_in(ctx.design, &r);
                let sel = if shift {
                    base.symmetric_difference(&inside).copied().collect()
                } else {
                    inside
                };
                vec![ToolEffect::Select(sel)]
            }
            (Phase::Up, Some(g)) => {
                self.gesture = None;
                self.preview = None;
                let moved = match g {
                    Gesture::Moving { start } => Some(self.move_by(start, p, e.shift, ctx)),
                    _ => None,
                };
                finish_select(g, e, ctx, moved)
            }
            _ => vec![],
        }
    }

    fn drag_shape_tool(&mut self, e: PointerEvent) -> Vec<ToolEffect> {
        let ellipse = self.tool() == Tool::Ellipse;
        match (e.phase, self.gesture.clone()) {
            (Phase::Down, _) => {
                self.gesture = Some(Gesture::Drawing { start: e.location });
                self.preview = None;
                vec![]
            }
            (Phase::Dragged, Some(Gesture::Drawing { start })) => {
                self.preview = Some(ToolPreview::Outline(outline(
                    drag_rect(start, e.location, e.shift),
                    ellipse,
                )));
                vec![]
            }
            (Phase::Up, Some(Gesture::Drawing { start })) => {
                self.gesture = None;
                self.preview = None;
                let r = drag_rect(start, e.location, e.shift);
                if r.width() < MINIMUM_SIZE || r.height() < MINIMUM_SIZE {
                    return vec![];
                }
                let c = r.center();
                let request = if ellipse {
                    CommandRequest::new(
                        "shape.ellipse",
                        json!({ "cx": mm(c.x), "cy": mm(c.y), "rx": mm(r.width() / 2.0), "ry": mm(r.height() / 2.0), "name": "Ellipse" }),
                    )
                } else {
                    let mut params = json!({ "x": mm(r.min_x), "y": mm(r.min_y), "width": mm(r.width()), "height": mm(r.height()), "name": "Rectangle" });
                    if self.corner_radius > 0.0 {
                        params["cornerRadius"] = json!(self.corner_radius);
                    }
                    CommandRequest::new("shape.rectangle", params)
                };
                vec![ToolEffect::run(request, true)]
            }
            _ => vec![],
        }
    }

    /// Click to add vertices; click the first vertex, double-click or press Return to close
    /// (Open Straight Line: double-click or Return ends the open line).
    fn polygon_tool(&mut self, e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        match e.phase {
            Phase::Down => {
                let closes_on_first = self.tool() == Tool::Polygon
                    && self.polygon.len() >= 3
                    && e.location.distance(self.polygon[0]) <= 2.0 * ctx.tolerance();
                if e.clicks >= 2 || closes_on_first {
                    return self.finish_polygon();
                }
                if self
                    .polygon
                    .last()
                    .is_some_and(|last| last.distance(e.location) <= ctx.tolerance() / 2.0)
                {
                    return vec![];
                }
                self.polygon.push(e.location);
                self.preview = Some(ToolPreview::Outline(Contour::new(
                    self.polygon.clone(),
                    false,
                )));
                vec![]
            }
            Phase::Moved | Phase::Dragged => {
                if !self.polygon.is_empty() {
                    let mut pts = self.polygon.clone();
                    pts.push(e.location);
                    self.preview = Some(ToolPreview::Outline(Contour::new(pts, false)));
                }
                vec![]
            }
            Phase::Up => vec![],
        }
    }

    /// Manual Punch: each click is the next end of a direction line (left, then right);
    /// double-click or Return finishes. The preview walks each line and the side to the next.
    fn punch_tool(&mut self, e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        match e.phase {
            Phase::Down => {
                if e.clicks >= 2 {
                    return self.finish_punch();
                }
                if self
                    .punch
                    .last()
                    .is_some_and(|last| last.distance(e.location) <= ctx.tolerance() / 2.0)
                {
                    return vec![];
                }
                self.punch.push(e.location);
                self.preview = Some(ToolPreview::Outline(self.punch_preview(None)));
                vec![]
            }
            Phase::Moved | Phase::Dragged => {
                if !self.punch.is_empty() {
                    self.preview = Some(ToolPreview::Outline(self.punch_preview(Some(e.location))));
                }
                vec![]
            }
            Phase::Up => vec![],
        }
    }

    pub fn punch(&self) -> &[Vec2] {
        &self.punch
    }

    /// Blocks show the ladder of direction lines; Running and Feed the line through the points.
    fn punch_preview(&self, hover: Option<Vec2>) -> Contour {
        match self.tool() {
            Tool::Running | Tool::Feed => {
                let mut pts = self.punch.clone();
                pts.extend(hover);
                Contour::new(pts, false)
            }
            _ => ladder(&self.punch, hover),
        }
    }

    /// Ends the Manual Punch shape: complete line pairs become one `shape.column` (curved for
    /// Curved Block); an unpaired last click is dropped; fewer than two lines make nothing.
    /// Running makes an open running line, Feed a `shape.feed`; both need two points.
    pub fn finish_punch(&mut self) -> Vec<ToolEffect> {
        let pts = std::mem::take(&mut self.punch);
        self.preview = None;
        let points = || -> Vec<serde_json::Value> {
            pts.iter().map(|p| json!([mm(p.x), mm(p.y)])).collect()
        };
        match self.tool() {
            Tool::Running if pts.len() >= 2 => {
                return vec![ToolEffect::run(
                    CommandRequest::new(
                        "shape.polygon",
                        json!({ "points": points(), "closed": false, "name": "Running" }),
                    ),
                    true,
                )];
            }
            Tool::Feed if pts.len() >= 2 => {
                return vec![ToolEffect::run(
                    CommandRequest::new("shape.feed", json!({ "points": points() })),
                    true,
                )];
            }
            Tool::Punch | Tool::CurvedBlock if pts.len() >= 4 => {}
            _ => return vec![],
        }
        let lines: Vec<_> = pts
            .as_chunks::<2>()
            .0
            .iter()
            .map(|w| json!([[mm(w[0].x), mm(w[0].y)], [mm(w[1].x), mm(w[1].y)]]))
            .collect();
        let mut params = json!({ "lines": lines });
        if self.tool() == Tool::CurvedBlock {
            params["curved"] = json!(true);
        }
        vec![ToolEffect::run(
            CommandRequest::new("shape.column", params),
            true,
        )]
    }

    /// Closes the polygon in progress (Return). Fewer than three vertices are discarded; an
    /// open line needs two.
    pub fn finish_polygon(&mut self) -> Vec<ToolEffect> {
        let pts = std::mem::take(&mut self.polygon);
        self.preview = None;
        let closed = match self.tool() {
            Tool::Polygon => true,
            Tool::Line => false,
            _ => return vec![],
        };
        if pts.len() < if closed { 3 } else { 2 } {
            return vec![];
        }
        let points: Vec<_> = pts.iter().map(|p| json!([mm(p.x), mm(p.y)])).collect();
        let name = if closed { "Polygon" } else { "Line" };
        vec![ToolEffect::run(
            CommandRequest::new(
                "shape.polygon",
                json!({ "points": points, "closed": closed, "name": name }),
            ),
            true,
        )]
    }

    /// Pencil: press and drag to trace; releasing ends the stroke as one `shape.freehand`,
    /// simplified to within about a screen point and a half.
    fn pencil_tool(&mut self, e: PointerEvent, ctx: &ToolContext) -> Vec<ToolEffect> {
        let p = e.location;
        match e.phase {
            Phase::Down => {
                self.pencil = vec![p];
                self.preview = None;
                vec![]
            }
            Phase::Dragged if !self.pencil.is_empty() => {
                if self
                    .pencil
                    .last()
                    .is_some_and(|q| q.distance(p) >= ctx.units_per_point)
                    && self.pencil.len() < openstitch_design::freehand::MAX_POINTS
                {
                    self.pencil.push(p);
                }
                self.preview = Some(ToolPreview::Outline(Contour::new(
                    self.pencil.clone(),
                    false,
                )));
                vec![]
            }
            Phase::Up if !self.pencil.is_empty() => {
                let mut trail = std::mem::take(&mut self.pencil);
                self.preview = None;
                if trail.last() != Some(&p) && trail.len() < openstitch_design::freehand::MAX_POINTS
                {
                    trail.push(p);
                }
                let closed = self.tool() == Tool::Pencil;
                let size = Rect::enclosing(trail.iter().copied())
                    .map_or(0.0, |r| r.width().max(r.height()));
                if size < MINIMUM_SIZE {
                    return vec![];
                }
                let tolerance = (1.5 * ctx.units_per_point / 10.0).clamp(0.05, 2.0);
                let points: Vec<_> = trail.iter().map(|q| json!([mm(q.x), mm(q.y)])).collect();
                let name = if closed { "Freehand" } else { "Freehand line" };
                vec![ToolEffect::run(
                    CommandRequest::new(
                        "shape.freehand",
                        json!({ "points": points, "closed": closed, "tolerance": (tolerance * 100.0).round() / 100.0, "name": name }),
                    ),
                    true,
                )]
            }
            _ => vec![],
        }
    }

    /// The Pencil trail being drawn.
    pub fn pencil(&self) -> &[Vec2] {
        &self.pencil
    }
}

/// Manual Punch preview: each direction line, then along the side to the next line's same end
/// (so the path zigzags left-right, right-left), plus the line to the pointer.
fn ladder(clicks: &[Vec2], hover: Option<Vec2>) -> Contour {
    let mut pts: Vec<Vec2> = Vec::new();
    for (i, pair) in clicks.chunks(2).enumerate() {
        if pair.len() == 2 && i % 2 == 1 {
            pts.extend([pair[1], pair[0]]);
        } else {
            pts.extend(pair.iter().copied());
        }
    }
    pts.extend(hover);
    Contour::new(pts, false)
}

fn move_delta(start: Vec2, p: Vec2, axis_lock: bool) -> Vec2 {
    let d = p - start;
    if !axis_lock {
        d
    } else if d.x.abs() >= d.y.abs() {
        v2(d.x, 0.0)
    } else {
        v2(0.0, d.y)
    }
}

/// `p` rounded to the nearest crossing of a grid of `step` from the page origin.
pub fn snap_to_grid(p: Vec2, step: f64) -> Vec2 {
    v2((p.x / step).round() * step, (p.y / step).round() * step)
}

/// Ends a Select-tool gesture; `moved` is the (snapped) move of a Moving gesture.
fn finish_select(
    g: Gesture,
    e: PointerEvent,
    ctx: &ToolContext,
    moved: Option<Vec2>,
) -> Vec<ToolEffect> {
    let p = e.location;
    let sel = ctx.selection;
    match g {
        Gesture::Moving { start } => {
            let d = moved.unwrap_or_else(|| move_delta(start, p, e.shift));
            if mm(d.x) == 0.0 && mm(d.y) == 0.0 {
                return vec![];
            }
            vec![ToolEffect::run(
                CommandRequest::move_by(sel, d.x, d.y),
                false,
            )]
        }
        Gesture::Scaling {
            handle,
            start,
            handles,
        } => {
            let s = handles.scale(handle, start, p, e.shift);
            if (s.sx - 1.0).abs() < 1e-4 && (s.sy - 1.0).abs() < 1e-4 {
                return vec![];
            }
            let requests = CommandRequest::scale_about(
                sel,
                s.sx,
                s.sy,
                s.origin,
                handles.bounds,
                ctx.scale_takes_origin,
            );
            vec![ToolEffect::Execute {
                requests,
                select_result: false,
            }]
        }
        Gesture::Rotating { start, handles } => {
            let deg = handles.rotation(start, p, e.shift);
            if deg.abs() < 0.01 {
                return vec![];
            }
            vec![ToolEffect::run(CommandRequest::rotate(sel, deg), false)]
        }
        Gesture::Pending { .. }
        | Gesture::Marquee { .. }
        | Gesture::Drawing { .. }
        | Gesture::Vertex { .. }
        | Gesture::Control { .. }
        | Gesture::PenAnchor { .. }
        | Gesture::Needles { .. } => vec![],
    }
}

fn drag_rect(start: Vec2, p: Vec2, square: bool) -> Rect {
    if !square {
        return rect_from(start, p);
    }
    let side = (p.x - start.x).abs().max((p.y - start.y).abs());
    let end = v2(
        start.x + if p.x < start.x { -side } else { side },
        start.y + if p.y < start.y { -side } else { side },
    );
    rect_from(start, end)
}

fn outline(r: Rect, ellipse: bool) -> Contour {
    if ellipse {
        Contour::ellipse(r.center(), r.width() / 2.0, r.height() / 2.0, 72)
    } else {
        Contour::rectangle(r.min_x, r.min_y, r.width(), r.height())
    }
}

#[cfg(test)]
mod punch_tests {
    use super::*;

    fn click(t: &mut ToolController, ctx: &ToolContext, at: Vec2, clicks: u32) -> Vec<ToolEffect> {
        let mut e = PointerEvent::new(Phase::Down, at);
        e.clicks = clicks;
        let mut out = t.handle(e, ctx);
        out.extend(t.handle(PointerEvent::new(Phase::Up, at), ctx));
        out
    }

    #[test]
    fn punch_clicks_make_one_column() {
        let design = Design::default();
        let sel = BTreeSet::new();
        let ctx = ToolContext {
            design: &design,
            selection: &sel,
            units_per_point: 1.0,
            scale_takes_origin: false,
        };
        let mut t = ToolController::default();
        t.set_tool(Tool::Punch);
        assert_eq!(Tool::from_shortcut('U'), Some(Tool::Punch));
        for p in [
            v2(100.0, 100.0),
            v2(100.0, 200.0),
            v2(400.0, 100.0),
            v2(350.0, 200.0),
            v2(500.0, 350.0),
        ] {
            assert!(click(&mut t, &ctx, p, 1).is_empty());
        }
        assert!(t.is_busy());
        // Preview: line 0 down, side across, line 1 up, then the pending click.
        let Some(ToolPreview::Outline(c)) = t.preview() else {
            panic!("preview")
        };
        assert_eq!(c.points[2], v2(350.0, 200.0));
        assert_eq!(c.points[3], v2(400.0, 100.0));
        let effects = click(&mut t, &ctx, v2(400.0, 350.0), 1);
        assert!(effects.is_empty());
        let effects = click(&mut t, &ctx, v2(400.0, 350.0), 2);
        let [
            ToolEffect::Execute {
                requests,
                select_result,
            },
        ] = effects.as_slice()
        else {
            panic!("one command")
        };
        assert!(*select_result);
        assert_eq!(requests[0].command, "shape.column");
        assert_eq!(
            requests[0].params["lines"],
            json!([
                [[10.0, 10.0], [10.0, 20.0]],
                [[40.0, 10.0], [35.0, 20.0]],
                [[50.0, 35.0], [40.0, 35.0]]
            ])
        );
        let mut s = openstitch_engine::Session::new(std::env::temp_dir());
        assert!(
            s.execute(requests[0].command, Some(&requests[0].params))
                .is_ok()
        );
        assert!(!t.is_busy());
    }

    #[test]
    fn punch_needs_two_lines_and_escape_cancels() {
        let design = Design::default();
        let sel = BTreeSet::new();
        let ctx = ToolContext {
            design: &design,
            selection: &sel,
            units_per_point: 1.0,
            scale_takes_origin: false,
        };
        let mut t = ToolController::default();
        t.set_tool(Tool::Punch);
        for p in [v2(0.0, 0.0), v2(0.0, 100.0), v2(100.0, 0.0)] {
            click(&mut t, &ctx, p, 1);
        }
        assert!(t.finish().is_empty(), "one line and a half is not a column");
        for p in [v2(0.0, 0.0), v2(0.0, 100.0)] {
            click(&mut t, &ctx, p, 1);
        }
        t.cancel();
        assert!(!t.is_busy() && t.punch().is_empty());
    }
}
