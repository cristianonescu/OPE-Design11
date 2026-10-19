//! Floral pattern (PE-DESIGN's "Creating a floral pattern") and decorative frames (the
//! "Adding a decorative pattern" workflow over OpenStitch's own frame designs).

use crate::commands::{CommandSpec, Effect, MM, default_line, opt, req, spec, thread};
use openstitch_design::ornament::{self, FLORAL_MAX_DENSITY, FLORAL_PATTERNS, Frame};
use openstitch_design::{DesignObject, LineSewing, LineType, Rect, v2};
use serde_json::json;

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "shape.floral",
            "Create a floral line pattern inside a closed shape (running stitch, sewn after it)",
            Effect::Edit,
            vec![
                req("id", "int", "Shape or text with a closed outline"),
                opt("density", "int 1-10", "Nested petal rings (default 3)"),
                opt(
                    "pattern",
                    "int 0-251",
                    "Petal count, shape and twist (default 0)",
                ),
                opt(
                    "color",
                    "color",
                    "Thread (default the shape's line or black)",
                ),
            ],
            |s, p| {
                let id = p.int("id", None, 0, i64::MAX)? as u64;
                let i = s
                    .design
                    .index_of(id)
                    .ok_or_else(|| p.not_found(format!("object {id}")))?;
                let density = p.int("density", Some(3), 1, i64::from(FLORAL_MAX_DENSITY))? as u32;
                let pattern = p.int("pattern", Some(0), 0, i64::from(FLORAL_PATTERNS) - 1)? as u32;
                let o = &s.design.objects[i];
                if !o.is_outline() {
                    return Err(p.failed(format!("object {id} is a stitch pattern")));
                }
                // The largest closed outline.
                let outline = o
                    .contours
                    .iter()
                    .filter(|c| c.closed)
                    .max_by(|a, b| {
                        openstitch_design::boolean::area(a)
                            .total_cmp(&openstitch_design::boolean::area(b))
                    })
                    .ok_or_else(|| p.failed(format!("object {id} has no closed outline")))?;
                let rings = ornament::floral(outline, density, pattern);
                if rings.is_empty() {
                    return Err(p.failed("the outline is too small"));
                }
                let mut line = o
                    .line
                    .clone()
                    .unwrap_or_else(|| LineSewing::new(LineType::Running, default_line()));
                line.kind = LineType::Running;
                line.entry = None;
                if let Some(t) = thread(p, "color")? {
                    line.thread = t;
                }
                let mut object = DesignObject::shape(rings, None, Some(line));
                object.name = Some("Floral pattern".into());
                let new = insert_after(&mut s.design, i, object);
                Ok(json!({ "id": new }))
            },
        ),
        spec(
            "shape.decorative",
            "Add a decorative frame: around the given objects, or standard size at the page centre",
            Effect::Edit,
            vec![
                req(
                    "kind",
                    "scallop|wave|zigzag|scallopOval|corners",
                    "Frame design",
                ),
                opt("ids", "[int] | id: int", "Objects to frame"),
                opt(
                    "keepAspect",
                    "bool",
                    "Square / circular frame around the objects instead of following their proportions (default true)",
                ),
                opt(
                    "margin",
                    "mm",
                    "Gap between the objects and the frame (default 3)",
                ),
                opt("size", "mm", "Ornament size (default 5)"),
                opt("color", "color", "Thread (default black)"),
            ],
            |s, p| {
                let kind = Frame::from_id(&p.string("kind", None)?).ok_or_else(|| {
                    p.invalid(
                        "kind",
                        "must be scallop, wave, zigzag, scallopOval or corners",
                    )
                })?;
                let keep = p.bool("keepAspect", Some(true))?;
                let margin = p.opt("margin", 3.0, 0.0, 100.0)? * MM;
                let size = p.opt("size", 5.0, 1.0, 50.0)? * MM;
                let around = if p.has("ids") || p.has("id") {
                    let idx = crate::commands::indexes(p, &s.design)?;
                    Some(
                        Rect::enclosing(idx.iter().flat_map(|&i| s.design.objects[i].points()))
                            .ok_or_else(|| p.failed("the objects have no geometry"))?,
                    )
                } else {
                    None
                };
                let b = frame_box(around, s.design.page.center(), keep, margin);
                let contours = ornament::frame(kind, &b, size);
                let colour = thread(p, "color")?.unwrap_or_else(default_line);
                let mut object = DesignObject::shape(
                    contours,
                    None,
                    Some(LineSewing::new(LineType::Running, colour)),
                );
                object.name = Some("Decorative frame".into());
                Ok(json!({ "id": s.design.add(object) }))
            },
        ),
    ]
}

/// The frame's box: the objects' bounds grown by `margin` (made square when `keep`), or a
/// 60 mm square at `centre`.
fn frame_box(
    around: Option<Rect>,
    centre: openstitch_design::Vec2,
    keep: bool,
    margin: f64,
) -> Rect {
    let (c, mut w, mut h) = match around {
        Some(b) => (
            b.center(),
            b.width() + 2.0 * margin,
            b.height() + 2.0 * margin,
        ),
        None => (centre, 600.0, 600.0),
    };
    if keep {
        let side = w.max(h);
        (w, h) = (side, side);
    }
    let (w, h) = (w.max(50.0), h.max(50.0));
    let half = v2(w / 2.0, h / 2.0);
    Rect {
        min_x: c.x - half.x,
        min_y: c.y - half.y,
        max_x: c.x + half.x,
        max_y: c.y + half.y,
    }
}

fn insert_after(d: &mut openstitch_design::Design, i: usize, mut o: DesignObject) -> u64 {
    o.id = d.next_object_id;
    d.next_object_id += 1;
    let id = o.id;
    d.objects.insert(i + 1, o);
    id
}
