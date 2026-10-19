//! Tolerances and expected values of a measurement.
//!
//! A tolerance names its unit by its key: `{"mm": 0.05}`, `{"count": 1}`, `{"deg": 1}` (circular,
//! modulo 180° unless `modulo` says otherwise), `{"percent": 2}` (of the expected value), or a
//! combination (`{"mm": 0.1, "percent": 1}` allows the sum). No tolerance means exact.
//!
//! An expected value is any JSON value, compared structurally: numbers within the tolerance,
//! strings, booleans and nulls exactly, arrays element by element (a number expected against an
//! array applies to every element), objects key by key. `{"min": a, "max": b}` (either bound
//! optional) is a range.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tolerance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    /// Compare circularly modulo this value (degrees default to 180).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modulo: Option<f64>,
}

impl Tolerance {
    pub const EXACT: Tolerance = Tolerance {
        mm: None,
        count: None,
        deg: None,
        percent: None,
        modulo: None,
    };

    pub fn mm(v: f64) -> Self {
        Self {
            mm: Some(v),
            ..Self::EXACT
        }
    }

    pub fn count(v: f64) -> Self {
        Self {
            count: Some(v),
            ..Self::EXACT
        }
    }

    pub fn percent(v: f64) -> Self {
        Self {
            percent: Some(v),
            ..Self::EXACT
        }
    }

    pub fn deg(v: f64) -> Self {
        Self {
            deg: Some(v),
            ..Self::EXACT
        }
    }

    /// Absolute part of the allowance (the units are the measurement's).
    pub fn absolute(&self) -> f64 {
        self.mm.unwrap_or(0.0) + self.count.unwrap_or(0.0) + self.deg.unwrap_or(0.0)
    }

    pub fn period(&self) -> Option<f64> {
        self.modulo.or(self.deg.map(|_| 180.0)).filter(|m| *m > 0.0)
    }

    /// Every value must be finite and not negative.
    pub fn validate(&self) -> Result<(), String> {
        for (k, v) in [
            ("mm", self.mm),
            ("count", self.count),
            ("deg", self.deg),
            ("percent", self.percent),
            ("modulo", self.modulo),
        ] {
            if let Some(v) = v
                && !(v.is_finite() && v >= 0.0)
            {
                return Err(format!("tolerance {k} must be a finite number ≥ 0"));
            }
        }
        Ok(())
    }

    /// The allowance for `expected`.
    pub fn allowance(&self, expected: f64) -> f64 {
        self.absolute() + self.percent.unwrap_or(0.0) / 100.0 * expected.abs()
    }

    /// Difference between two numbers, circular when the tolerance has a period.
    pub fn delta(&self, expected: f64, actual: f64) -> f64 {
        let d = actual - expected;
        match self.period() {
            Some(m) => {
                let r = d.rem_euclid(m);
                if r > m / 2.0 { r - m } else { r }
            }
            None => d,
        }
    }

    pub fn accepts(&self, expected: f64, actual: f64) -> bool {
        // A hair of slack for values that are equal up to f64 rounding.
        self.delta(expected, actual).abs() <= self.allowance(expected) + 1e-9
    }
}

/// One way in which a measured value differs from its expected value.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Mismatch {
    /// JSON pointer into the value ("" for the whole value).
    pub at: String,
    pub expected: Value,
    pub actual: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

/// `{"min": a, "max": b}` with at least one numeric bound and nothing else.
fn as_range(v: &Value) -> Option<(Option<f64>, Option<f64>)> {
    let o = v.as_object()?;
    if o.is_empty() || o.keys().any(|k| k != "min" && k != "max") {
        return None;
    }
    let bound = |k: &str| match o.get(k) {
        None => Some(None),
        Some(x) => x.as_f64().map(Some),
    };
    Some((bound("min")?, bound("max")?))
}

/// Compares `actual` with `expected` under `tolerance`; empty when it matches.
pub fn check(expected: &Value, actual: &Value, tolerance: &Tolerance) -> Vec<Mismatch> {
    let mut out = Vec::new();
    walk(expected, actual, tolerance, "", &mut out);
    out
}

fn walk(expected: &Value, actual: &Value, t: &Tolerance, at: &str, out: &mut Vec<Mismatch>) {
    let miss = |out: &mut Vec<Mismatch>, delta: Option<f64>| {
        out.push(Mismatch {
            at: at.to_string(),
            expected: expected.clone(),
            actual: actual.clone(),
            delta,
        })
    };
    if let Some((lo, hi)) = as_range(expected) {
        match actual {
            Value::Array(items) => {
                for (i, a) in items.iter().enumerate() {
                    walk(expected, a, t, &format!("{at}/{i}"), out);
                }
            }
            _ => match actual.as_f64() {
                Some(a)
                    if lo.is_none_or(|lo| a + t.allowance(lo) + 1e-9 >= lo)
                        && hi.is_none_or(|hi| a <= hi + t.allowance(hi) + 1e-9) => {}
                Some(a) => {
                    let d = match (lo, hi) {
                        (Some(lo), _) if a < lo => a - lo,
                        (_, Some(hi)) => a - hi,
                        _ => 0.0,
                    };
                    miss(out, Some(d));
                }
                None => miss(out, None),
            },
        }
        return;
    }
    match (expected, actual) {
        (Value::Number(e), Value::Number(a)) => {
            let (e, a) = (
                e.as_f64().unwrap_or(f64::NAN),
                a.as_f64().unwrap_or(f64::NAN),
            );
            if !t.accepts(e, a) {
                miss(out, Some(t.delta(e, a)));
            }
        }
        (Value::Number(_), Value::Array(items)) => {
            for (i, a) in items.iter().enumerate() {
                walk(expected, a, t, &format!("{at}/{i}"), out);
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                out.push(Mismatch {
                    at: format!("{at}/length"),
                    expected: json!(e.len()),
                    actual: json!(a.len()),
                    delta: Some(a.len() as f64 - e.len() as f64),
                });
            }
            for (i, (e, a)) in e.iter().zip(a).enumerate() {
                walk(e, a, t, &format!("{at}/{i}"), out);
            }
        }
        (Value::Object(e), Value::Object(a)) => {
            for (k, ev) in e {
                let key = format!("{at}/{}", k.replace('~', "~0").replace('/', "~1"));
                match a.get(k) {
                    Some(av) => walk(ev, av, t, &key, out),
                    None => out.push(Mismatch {
                        at: key,
                        expected: ev.clone(),
                        actual: Value::Null,
                        delta: None,
                    }),
                }
            }
            for (k, av) in a {
                if !e.contains_key(k) {
                    out.push(Mismatch {
                        at: format!("{at}/{}", k.replace('~', "~0").replace('/', "~1")),
                        expected: Value::Null,
                        actual: av.clone(),
                        delta: None,
                    });
                }
            }
        }
        (e, a) if e == a => {}
        _ => miss(out, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerance_edges_are_inclusive() {
        let t = Tolerance::mm(0.5);
        assert!(t.accepts(15.0, 15.5));
        assert!(t.accepts(15.0, 14.5));
        assert!(!t.accepts(15.0, 15.500_1));
        let p = Tolerance::percent(2.0);
        assert!(p.accepts(100.0, 102.0));
        assert!(!p.accepts(100.0, 102.01));
        assert!(Tolerance::EXACT.accepts(3.0, 3.0));
        assert!(!Tolerance::EXACT.accepts(3.0, 3.001));
    }

    #[test]
    fn degrees_are_circular() {
        let t = Tolerance::deg(1.0);
        assert!(t.accepts(0.0, 179.5));
        assert!(t.accepts(179.0, 0.0));
        assert!(!t.accepts(45.0, 135.0));
        assert!((t.delta(1.0, 179.0) + 2.0).abs() < 1e-12);
    }

    #[test]
    fn structural_checks_report_every_mismatch() {
        assert!(check(&json!(15), &json!(16), &Tolerance::count(1.0)).is_empty());
        let m = check(&json!([1, 2, 3]), &json!([1, 5]), &Tolerance::EXACT);
        assert_eq!(m.len(), 2, "{m:?}");
        assert_eq!(m[0].at, "/length");
        assert_eq!(m[1].at, "/1");
        let m = check(
            &json!({"a": 1}),
            &json!({"a": 1, "b": 2}),
            &Tolerance::EXACT,
        );
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].at, "/b");
        // A scalar against an array checks every element.
        let m = check(&json!(15), &json!([15.2, 14.9, 16.0]), &Tolerance::mm(0.5));
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].at, "/2");
    }

    #[test]
    fn ranges_bound_values() {
        let r = json!({"max": 10});
        assert!(check(&r, &json!(10), &Tolerance::EXACT).is_empty());
        assert_eq!(check(&r, &json!(10.5), &Tolerance::EXACT).len(), 1);
        assert!(check(&json!({"min": 60}), &json!(60), &Tolerance::EXACT).is_empty());
        assert_eq!(
            check(&json!({"min": 60}), &json!(59), &Tolerance::EXACT).len(),
            1
        );
        // A range against an array bounds every element.
        assert_eq!(
            check(
                &json!({"min": 0, "max": 1}),
                &json!([0, 1, 2]),
                &Tolerance::EXACT
            )
            .len(),
            1
        );
        // Not a range: another key.
        assert_eq!(
            check(&json!({"min": 1, "x": 2}), &json!(1), &Tolerance::EXACT).len(),
            1
        );
    }

    #[test]
    fn invalid_tolerances_are_refused() {
        assert!(Tolerance::mm(-1.0).validate().is_err());
        assert!(Tolerance::percent(f64::NAN).validate().is_err());
        assert!(Tolerance::count(0.0).validate().is_ok());
    }
}
