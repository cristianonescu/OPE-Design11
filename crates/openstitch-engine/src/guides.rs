//! Guidelines (View ▸ Guideline): lines dragged out of the rulers to line objects up. They are a
//! view aid saved with the design, never sewn; changing them is one undo step like any edit.

use crate::commands::{CommandSpec, Effect, MM, opt, spec};
use crate::params::{CommandError, Params};
use openstitch_design::Guides;
use serde_json::json;

fn positions(p: &Params, key: &str, now: &[f64]) -> Result<Vec<f64>, CommandError> {
    if !p.has(key) {
        return Ok(now.to_vec());
    }
    let list = p.array(key)?;
    if list.len() > Guides::MAX {
        return Err(p.invalid(key, format!("at most {} guidelines", Guides::MAX)));
    }
    list.iter()
        .map(|v| {
            v.as_f64()
                .filter(|n| (-5000.0..=5000.0).contains(n))
                .map(|n| n * MM)
                .ok_or_else(|| p.invalid(key, "positions must be numbers in -5000...5000 mm"))
        })
        .collect()
}

pub fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "guides.set",
            "Set the guidelines (lines dragged out of the rulers; saved with the design, never sewn)",
            Effect::Edit,
            vec![
                opt(
                    "vertical",
                    "[mm]",
                    "x of each vertical guideline, from the Design Page's left edge; at most 100 (default: unchanged)",
                ),
                opt(
                    "horizontal",
                    "[mm]",
                    "y of each horizontal guideline, from the Design Page's top edge; at most 100 (default: unchanged)",
                ),
            ],
            |s, p| {
                let g = &s.design.guides;
                let vertical = positions(p, "vertical", &g.vertical)?;
                let horizontal = positions(p, "horizontal", &g.horizontal)?;
                s.design.guides = Guides {
                    vertical,
                    horizontal,
                };
                let mm = |v: &[f64]| v.iter().map(|x| x / MM).collect::<Vec<_>>();
                Ok(json!({
                    "vertical": mm(&s.design.guides.vertical),
                    "horizontal": mm(&s.design.guides.horizontal),
                }))
            },
        ),
        spec(
            "guides.list",
            "The design's guidelines, mm",
            Effect::Read,
            vec![],
            |s, _| {
                let mm = |v: &[f64]| v.iter().map(|x| x / MM).collect::<Vec<_>>();
                Ok(json!({
                    "vertical": mm(&s.design.guides.vertical),
                    "horizontal": mm(&s.design.guides.horizontal),
                }))
            },
        ),
    ]
}
