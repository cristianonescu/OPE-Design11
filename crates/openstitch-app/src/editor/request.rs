use openstitch_design::Rect;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Model units (0.1 mm) → millimetres rounded to 0.01 mm, the unit commands take.
pub fn mm(units: f64) -> f64 {
    (units * 10.0).round() / 100.0
}

/// One command for [`openstitch_engine::Session::execute`]: the only way the UI changes a design.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandRequest {
    pub command: &'static str,
    pub params: Value,
}

impl CommandRequest {
    pub fn new(command: &'static str, params: Value) -> Self {
        CommandRequest { command, params }
    }

    pub fn ids(ids: &BTreeSet<u64>) -> Value {
        json!(ids.iter().collect::<Vec<_>>())
    }

    pub fn move_by(ids: &BTreeSet<u64>, dx: f64, dy: f64) -> Self {
        Self::new(
            "object.move",
            json!({ "ids": Self::ids(ids), "dx": mm(dx), "dy": mm(dy) }),
        )
    }

    pub fn rotate(ids: &BTreeSet<u64>, degrees: f64) -> Self {
        Self::new(
            "object.rotate",
            json!({ "ids": Self::ids(ids), "degrees": (degrees * 100.0).round() / 100.0 }),
        )
    }

    pub fn flip(ids: &BTreeSet<u64>, horizontal: bool) -> Self {
        Self::new(
            "object.flip",
            json!({ "ids": Self::ids(ids), "axis": if horizontal { "horizontal" } else { "vertical" } }),
        )
    }

    pub fn duplicate(ids: &BTreeSet<u64>) -> Self {
        Self::new("object.duplicate", json!({ "ids": Self::ids(ids) }))
    }

    pub fn delete(ids: &BTreeSet<u64>) -> Self {
        Self::new("object.delete", json!({ "ids": Self::ids(ids) }))
    }

    /// Scale by `sx`/`sy` keeping `origin` (model units) fixed. `object.scale` scales about the
    /// selection's centre; when the engine does not take `originX`/`originY` the fixed point is
    /// restored with a follow-up move (scaling about c and about o differ by (1 − s)(o − c)).
    pub fn scale_about(
        ids: &BTreeSet<u64>,
        sx: f64,
        sy: f64,
        origin: openstitch_design::Vec2,
        bounds: Rect,
        engine_takes_origin: bool,
    ) -> Vec<Self> {
        let round = |f: f64| (f * 10_000.0).round() / 10_000.0;
        let (sx, sy) = (round(sx), round(sy));
        let mut params = json!({ "ids": Self::ids(ids), "sx": sx, "sy": sy });
        if engine_takes_origin {
            params["originX"] = json!(mm(origin.x));
            params["originY"] = json!(mm(origin.y));
            return vec![Self::new("object.scale", params)];
        }
        let c = bounds.center();
        let (dx, dy) = ((1.0 - sx) * (origin.x - c.x), (1.0 - sy) * (origin.y - c.y));
        let mut out = vec![Self::new("object.scale", params)];
        if mm(dx) != 0.0 || mm(dy) != 0.0 {
            out.push(Self::move_by(ids, dx, dy));
        }
        out
    }

    /// The object id(s) a command reports creating (`id` or `ids`).
    pub fn created_ids(result: &Value) -> BTreeSet<u64> {
        if let Some(id) = result.get("id").and_then(Value::as_u64) {
            return BTreeSet::from([id]);
        }
        result
            .get("ids")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default()
    }
}
