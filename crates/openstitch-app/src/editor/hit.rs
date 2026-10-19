use openstitch_design::geometry::contains;
use openstitch_design::{Design, DesignObject, ObjectKind, Rect, Vec2};
use std::collections::BTreeSet;

/// Rectangle spanned by two corners.
pub fn rect_from(a: Vec2, b: Vec2) -> Rect {
    Rect {
        min_x: a.x.min(b.x),
        min_y: a.y.min(b.y),
        max_x: a.x.max(b.x),
        max_y: a.y.max(b.y),
    }
}

pub fn rect_contains_point(r: &Rect, p: Vec2) -> bool {
    p.x >= r.min_x && p.x <= r.max_x && p.y >= r.min_y && p.y <= r.max_y
}

pub fn rect_contains_rect(outer: &Rect, inner: &Rect) -> bool {
    inner.min_x >= outer.min_x
        && inner.max_x <= outer.max_x
        && inner.min_y >= outer.min_y
        && inner.max_y <= outer.max_y
}

pub fn inflate(r: &Rect, d: f64) -> Rect {
    Rect {
        min_x: r.min_x - d,
        min_y: r.min_y - d,
        max_x: r.max_x + d,
        max_y: r.max_y + d,
    }
}

pub fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 <= 0.0 {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Whether `p` hits `o`: inside a closed outline or within `tolerance` of any outline for a
/// shape; anywhere in the (inflated) bounding box for a stitch pattern.
pub fn hits(o: &DesignObject, p: Vec2, tolerance: f64) -> bool {
    match o.kind {
        ObjectKind::Shape | ObjectKind::Text => {
            contains(p, &o.contours)
                || o.contours.iter().any(|c| {
                    c.edges()
                        .iter()
                        .any(|&(a, b)| distance_to_segment(p, a, b) <= tolerance)
                })
                // Stitch glyphs of user-mapped text: their bounding box.
                || o.stitch_block.as_ref().is_some_and(|_| {
                    o.bounds()
                        .is_some_and(|b| rect_contains_point(&inflate(&b, tolerance), p))
                })
        }
        ObjectKind::Stitches => o
            .bounds()
            .is_some_and(|b| rect_contains_point(&inflate(&b, tolerance), p)),
    }
}

/// Topmost visible object at `p`. Objects sewn later lie on top, so the search runs backwards
/// through the sewing order.
pub fn object_at(design: &Design, p: Vec2, tolerance: f64) -> Option<u64> {
    design
        .objects
        .iter()
        .rev()
        .find(|o| o.visible && hits(o, p, tolerance))
        .map(|o| o.id)
}

/// Visible objects whose bounds lie entirely inside `rect` (marquee selection).
pub fn objects_in(design: &Design, rect: &Rect) -> BTreeSet<u64> {
    design
        .objects
        .iter()
        .filter(|o| o.visible && o.bounds().is_some_and(|b| rect_contains_rect(rect, &b)))
        .map(|o| o.id)
        .collect()
}

/// Common bounding box of the given objects.
pub fn bounds_of(design: &Design, ids: &BTreeSet<u64>) -> Option<Rect> {
    Rect::enclosing(
        design
            .objects
            .iter()
            .filter(|o| ids.contains(&o.id))
            .flat_map(DesignObject::points),
    )
}
