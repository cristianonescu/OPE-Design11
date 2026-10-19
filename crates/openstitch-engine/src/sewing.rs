//! Region sewing geometry set by pointing: centre point, pattern reference point and the
//! curved sewing direction (PE-DESIGN's "Moving ... the center point and stitch pattern
//! reference point" and "Specifying the curved sewing direction (round fill)").

use crate::commands::{CommandSpec, Effect, opt, req, spec};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_design::direction::{self, CurveError, Warp};
use openstitch_design::{Bezier, Contour, RegionSewing, RegionType, Vec2};
use serde_json::json;

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "object.setCenterPoint",
            "Centre of concentric circle, radial and spiral stitches (null = centre of the region)",
            Effect::Edit,
            vec![
                req("id", "int", "Object with region sewing"),
                req("point", "[x, y] mm | null", "Centre point"),
            ],
            |s, p| {
                let point = point_or_null(p)?;
                let r = region(
                    s,
                    p,
                    point.is_some().then_some(&direction::CENTER_TYPES[..]),
                )?;
                r.center = point;
                Ok(json!(null))
            },
        ),
        spec(
            "object.setReferencePoint",
            "Where the pattern grid of programmable, motif, cross, net, zigzag net and decorative fills starts (null = default)",
            Effect::Edit,
            vec![
                req("id", "int", "Object with region sewing"),
                req("point", "[x, y] mm | null", "Reference point"),
            ],
            |s, p| {
                let point = point_or_null(p)?;
                let r = region(
                    s,
                    p,
                    point.is_some().then_some(&direction::REFERENCE_TYPES[..]),
                )?;
                r.reference = point;
                Ok(json!(null))
            },
        ),
        spec(
            "object.setDirectionCurve",
            "Curved sewing direction (round fill) for fill, programmable fill, piping and motif fill: rows follow the curve (null = straight)",
            Effect::Edit,
            vec![
                req("id", "int", "Object with region sewing"),
                req(
                    "points",
                    "[[x, y]] mm | null",
                    "Curve vertices (2-1000), not turning back along the chord",
                ),
                opt(
                    "curves",
                    "[null | [[c1x, c1y], [c2x, c2y]]] mm",
                    "One entry per segment; null = straight",
                ),
            ],
            |s, p| {
                if p.is_null("points") {
                    region(s, p, None)?.direction_curve = None;
                    return Ok(json!(null));
                }
                let points = p.points("points", 2)?;
                if points.len() > 1000 {
                    return Err(p.invalid("points", "needs 2...1000 points"));
                }
                let mut curve = Contour::new(points, false);
                if p.has("curves") {
                    let curves = p.array("curves")?;
                    if curves.len() != curve.segment_count() {
                        return Err(p.invalid(
                            "curves",
                            format!("needs one entry per segment ({})", curve.segment_count()),
                        ));
                    }
                    for (i, v) in curves.iter().enumerate() {
                        let b = p
                            .bezier_value("curves", v)?
                            .map(|(c1, c2)| Bezier { c1, c2 });
                        curve.set_curve(i, b);
                    }
                }
                match Warp::new(&curve) {
                    Ok(_) => {}
                    Err(CurveError::TooShort) => {
                        return Err(
                            p.invalid("points", "the curve's ends must be at least 0.1 mm apart")
                        );
                    }
                    Err(CurveError::TurnsBack) => {
                        return Err(p.invalid(
                            "points",
                            "the curve must not turn back along the line between its ends",
                        ));
                    }
                }
                region(s, p, Some(&direction::CURVE_TYPES[..]))?.direction_curve = Some(curve);
                Ok(json!(null))
            },
        ),
    ]
}

/// `point` as `[x, y]` mm, or `None` for an explicit null.
fn point_or_null(p: &Params) -> Result<Option<Vec2>, CommandError> {
    if p.is_null("point") {
        Ok(None)
    } else {
        Ok(Some(p.point("point")?))
    }
}

/// The region sewing of object `id`; when `kinds` is given its type must be one of them.
fn region<'a>(
    s: &'a mut EngineState,
    p: &Params,
    kinds: Option<&[RegionType]>,
) -> Result<&'a mut RegionSewing, CommandError> {
    let id = p.int("id", None, 0, i64::MAX)? as u64;
    let i = s
        .design
        .index_of(id)
        .ok_or_else(|| p.not_found(format!("object {id}")))?;
    let r = s.design.objects[i]
        .region
        .as_mut()
        .ok_or_else(|| p.failed(format!("object {id} has no region sewing")))?;
    if let Some(k) = kinds
        && !k.contains(&r.kind)
    {
        return Err(p.failed(format!(
            "object {id}: its region sew type has no such setting"
        )));
    }
    Ok(r)
}
