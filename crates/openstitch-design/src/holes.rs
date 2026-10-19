//! Hole sewing (PE-DESIGN): a pair of regions where the outer one fully encloses the inner
//! one. The outer region is sewn around the inner shape instead of under it. The outer object
//! keeps the inner object's id ([`DesignObject::hole_inner`]); designs saved before the pair
//! was stored pair the outer object with the first region sewn after it that it encloses.

use crate::boolean::{self, BoolOp};
use crate::geometry::Contour;
use crate::model::{Design, DesignObject};

/// Relative area tolerance of the enclosure test (numerical slivers along shared edges).
const TOLERANCE: f64 = 0.01;

/// The closed contours of `o` when it is an outline object with region sewing.
pub fn region_contours(o: &DesignObject) -> Option<Vec<Contour>> {
    if !o.is_outline() || o.region.is_none() {
        return None;
    }
    let closed: Vec<Contour> = o.contours.iter().filter(|c| c.closed).cloned().collect();
    (!closed.is_empty()).then_some(closed)
}

fn area(contours: &[Contour]) -> f64 {
    boolean::simplify(contours)
        .iter()
        .map(|r| boolean::region_area(r))
        .sum()
}

/// Whether the region `outer` fully encloses the region `inner`: removing `inner` from
/// `outer` takes away exactly the area of `inner` (±1 %), and `inner` is the smaller one.
pub fn encloses(outer: &[Contour], inner: &[Contour]) -> bool {
    let (a_outer, a_inner) = (area(outer), area(inner));
    if a_inner <= 0.0 || a_inner >= a_outer {
        return false;
    }
    let rest: f64 = boolean::combine(outer, inner, BoolOp::Difference)
        .iter()
        .map(|r| boolean::region_area(r))
        .sum();
    ((a_outer - rest) - a_inner).abs() <= TOLERANCE * a_inner
}

/// Whether region object `outer` fully encloses region object `inner`.
pub fn object_encloses(outer: &DesignObject, inner: &DesignObject) -> bool {
    match (region_contours(outer), region_contours(inner)) {
        (Some(a), Some(b)) => encloses(&a, &b),
        _ => false,
    }
}

/// The object whose shape is cut out of `object` when it is sewn: its stored pair, or for
/// designs saved before the pair was stored, the first region sewn after it that it encloses.
pub fn paired_inner<'a>(object: &DesignObject, design: &'a Design) -> Option<&'a DesignObject> {
    if !object.hole_sewing {
        return None;
    }
    if let Some(id) = object.hole_inner {
        return design.index_of(id).map(|i| &design.objects[i]);
    }
    let i = design.index_of(object.id)?;
    design.objects[i + 1..]
        .iter()
        .find(|o| o.visible && object_encloses(object, o))
}
