//! Boolean operations and offsets on closed outlines, using the `i_overlay` polygon engine
//! (MIT OR Apache-2.0, pure Rust, integer-snapped and robust to self-intersections).
//!
//! Regions follow the model's even-odd rule across a shape's closed contours. Curved segments
//! are flattened (to [`crate::geometry::FLATNESS`]) first, so results are straight polygons.
//! Lengths are 0.1 mm.

use crate::geometry::{Contour, Vec2, v2};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};

/// A boolean combination of two regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    /// Subject minus clip.
    Difference,
    Intersection,
    /// Either but not both.
    Xor,
}

/// One connected region: an outer boundary and its holes.
pub type Region = Vec<Contour>;

type Path = Vec<[f64; 2]>;

fn paths(contours: &[Contour]) -> Vec<Path> {
    contours
        .iter()
        .filter(|c| c.closed)
        .map(|c| {
            c.outline_points()
                .iter()
                .map(|p| [p.x, p.y])
                .collect::<Path>()
        })
        .filter(|p| p.len() >= 3)
        .collect()
}

fn regions(shapes: Vec<Vec<Path>>) -> Vec<Region> {
    shapes
        .into_iter()
        .map(|shape| {
            shape
                .into_iter()
                .filter(|p| p.len() >= 3)
                .map(|p| Contour::new(p.into_iter().map(|[x, y]| v2(x, y)).collect(), true))
                .collect::<Region>()
        })
        .filter(|r| !r.is_empty() && area(&r[0]) > MIN_AREA)
        .collect()
}

/// Pieces smaller than this (0.1 mm², i.e. 0.001 mm²·10) are numerical slivers.
const MIN_AREA: f64 = 0.1;

/// Unsigned area of a closed straight contour, (0.1 mm)².
pub fn area(c: &Contour) -> f64 {
    let p = c.outline_points();
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

/// Area of a region under the even-odd rule (outer minus holes).
pub fn region_area(r: &[Contour]) -> f64 {
    let mut it = r.iter();
    let outer = it.next().map(area).unwrap_or(0.0);
    outer - it.map(area).sum::<f64>()
}

/// `subject op clip`, as connected regions.
pub fn combine(subject: &[Contour], clip: &[Contour], op: BoolOp) -> Vec<Region> {
    let (s, c) = (paths(subject), paths(clip));
    if s.is_empty() && c.is_empty() {
        return Vec::new();
    }
    let rule = match op {
        BoolOp::Union => OverlayRule::Union,
        BoolOp::Difference => OverlayRule::Difference,
        BoolOp::Intersection => OverlayRule::Intersect,
        BoolOp::Xor => OverlayRule::Xor,
    };
    regions(s.overlay(&c, rule, FillRule::EvenOdd))
}

/// Union of several objects' regions (each even-odd within itself).
pub fn union_all<'a>(objects: impl IntoIterator<Item = &'a [Contour]>) -> Vec<Region> {
    let mut acc: Vec<Contour> = Vec::new();
    for o in objects {
        acc = contours(&combine(&acc, o, BoolOp::Union));
    }
    simplify(&acc)
}

/// The region of `contours` cleaned up: self-intersections resolved, split into connected
/// pieces.
pub fn simplify(contours: &[Contour]) -> Vec<Region> {
    let p = paths(contours);
    if p.is_empty() {
        return Vec::new();
    }
    regions(p.simplify_shape(FillRule::EvenOdd))
}

/// Outline of `contours` moved `distance` outward (negative = inward), corners mitred or
/// rounded. An inward offset larger than the region leaves nothing.
pub fn offset(contours: &[Contour], distance: f64, rounded: bool) -> Vec<Region> {
    let p = paths(contours);
    if p.is_empty() || distance == 0.0 || !distance.is_finite() {
        return Vec::new();
    }
    let clean = p.simplify_shape(FillRule::EvenOdd);
    let join = if rounded {
        // Arc segments no longer than a quarter of the radius, but at most 1 mm.
        LineJoin::Round((10.0 / distance.abs()).min(0.25))
    } else {
        LineJoin::Miter(std::f64::consts::PI / 18.0)
    };
    let style = OutlineStyle::new(distance).line_join(join);
    regions(clean.outline(&style))
}

/// Flattens regions into the model's contour list (even-odd keeps holes holes).
pub fn contours(regions: &[Region]) -> Vec<Contour> {
    regions.iter().flatten().cloned().collect()
}

/// Centre of the bounding box of a region's outer boundary.
pub fn centre(r: &[Contour]) -> Vec2 {
    crate::geometry::Rect::enclosing(r.iter().take(1).flat_map(|c| c.points.iter().copied()))
        .map(|b| b.center())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::contains;

    fn square(x: f64, y: f64, s: f64) -> Contour {
        Contour::rectangle(x, y, s, s)
    }

    fn total(r: &[Region]) -> f64 {
        r.iter().map(|r| region_area(r)).sum()
    }

    #[test]
    fn overlapping_squares() {
        let (a, b) = ([square(0.0, 0.0, 100.0)], [square(50.0, 50.0, 100.0)]);
        let u = combine(&a, &b, BoolOp::Union);
        assert_eq!(u.len(), 1);
        assert!((total(&u) - 17_500.0).abs() < 1.0);
        let i = combine(&a, &b, BoolOp::Intersection);
        assert!((total(&i) - 2_500.0).abs() < 1.0);
        let d = combine(&a, &b, BoolOp::Difference);
        assert!((total(&d) - 7_500.0).abs() < 1.0);
        assert!(!contains(v2(75.0, 75.0), &contours(&d)));
        assert!(contains(v2(25.0, 25.0), &contours(&d)));
        let x = combine(&a, &b, BoolOp::Xor);
        assert_eq!(x.len(), 2);
        assert!((total(&x) - 15_000.0).abs() < 1.0);
    }

    #[test]
    fn difference_can_make_a_hole_and_disjoint_union_stays_apart() {
        let d = combine(
            &[square(0.0, 0.0, 100.0)],
            &[square(25.0, 25.0, 50.0)],
            BoolOp::Difference,
        );
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].len(), 2, "outer + hole");
        assert!(!contains(v2(50.0, 50.0), &contours(&d)));
        let u = combine(
            &[square(0.0, 0.0, 10.0)],
            &[square(50.0, 0.0, 10.0)],
            BoolOp::Union,
        );
        assert_eq!(u.len(), 2);
        assert!(combine(&[], &[], BoolOp::Union).is_empty());
        let open = Contour::new(vec![v2(0.0, 0.0), v2(10.0, 10.0)], false);
        assert!(combine(&[open], &[], BoolOp::Union).is_empty());
    }

    #[test]
    fn union_all_does_not_cancel_overlaps() {
        let a = [square(0.0, 0.0, 100.0)];
        let b = [square(50.0, 50.0, 100.0)];
        let c = [square(140.0, 140.0, 20.0)];
        let u = union_all([&a[..], &b[..], &c[..]]);
        assert_eq!(u.len(), 1);
        assert!((total(&u) - (17_500.0 + 400.0 - 100.0)).abs() < 1.0);
        assert!(union_all(std::iter::empty::<&[Contour]>()).is_empty());
    }

    #[test]
    fn self_intersecting_bow_tie_is_split() {
        let bow = Contour::new(
            vec![
                v2(0.0, 0.0),
                v2(100.0, 100.0),
                v2(100.0, 0.0),
                v2(0.0, 100.0),
            ],
            true,
        );
        let r = simplify(&[bow]);
        assert_eq!(r.len(), 2);
        assert!((total(&r) - 5_000.0).abs() < 1.0);
    }

    #[test]
    fn offsets_grow_and_shrink() {
        let s = [square(0.0, 0.0, 100.0)];
        let out = offset(&s, 10.0, false);
        assert_eq!(out.len(), 1);
        assert!((total(&out) - 120.0 * 120.0).abs() < 2.0, "{}", total(&out));
        let round = offset(&s, 10.0, true);
        let expected = 100.0 * 100.0 + 4.0 * 100.0 * 10.0 + std::f64::consts::PI * 100.0;
        assert!((total(&round) - expected).abs() < 20.0, "{}", total(&round));
        let inward = offset(&s, -10.0, false);
        assert!((total(&inward) - 80.0 * 80.0).abs() < 2.0);
        assert!(offset(&s, -60.0, false).is_empty());
        assert!(offset(&s, 0.0, false).is_empty());
        // A ring grows on both sides of its hole.
        let ring = [square(0.0, 0.0, 100.0), square(30.0, 30.0, 40.0)];
        let grown = offset(&ring, 5.0, false);
        assert_eq!(grown[0].len(), 2);
        assert!(
            (total(&grown) - (110.0 * 110.0 - 30.0 * 30.0)).abs() < 2.0,
            "{}",
            total(&grown)
        );
    }

    #[test]
    fn curves_are_flattened() {
        let circle = crate::shapes::arc(
            v2(0.0, 0.0),
            100.0,
            100.0,
            0.0,
            360.0,
            crate::shapes::ArcMode::Arc,
        );
        let r = combine(&[circle], &[square(0.0, -200.0, 400.0)], BoolOp::Difference);
        let half = std::f64::consts::PI * 100.0 * 100.0 / 2.0;
        assert!((total(&r) - half).abs() < half * 0.01);
    }
}
