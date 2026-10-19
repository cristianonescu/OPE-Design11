use openstitch_design::{Vec2, v2};
use serde_json::{Map, Value};
use std::fmt;

/// Every way a command can fail. Commands never panic on bad input.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandError {
    UnknownCommand(String),
    MissingParameter {
        command: String,
        param: String,
    },
    InvalidParameter {
        command: String,
        param: String,
        reason: String,
    },
    NotFound {
        command: String,
        what: String,
    },
    Failed {
        command: String,
        reason: String,
    },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand(c) => write!(f, "unknown command '{c}'"),
            Self::MissingParameter { command, param } => {
                write!(f, "{command}: missing parameter '{param}'")
            }
            Self::InvalidParameter {
                command,
                param,
                reason,
            } => write!(f, "{command}: parameter '{param}' {reason}"),
            Self::NotFound { command, what } => write!(f, "{command}: {what} not found"),
            Self::Failed { command, reason } => write!(f, "{command}: {reason}"),
        }
    }
}

impl std::error::Error for CommandError {}

pub type CommandResult = Result<Value, CommandError>;

/// Typed, validating access to a command's JSON parameters.
pub struct Params {
    pub command: String,
    values: Map<String, Value>,
}

impl Params {
    pub fn new(command: &str, value: Option<&Value>) -> Result<Self, CommandError> {
        let values = match value {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(o)) => o.clone(),
            Some(_) => return Err(Self::invalid_for(command, "params", "must be an object")),
        };
        Ok(Params {
            command: command.to_string(),
            values,
        })
    }

    fn invalid_for(command: &str, param: &str, reason: impl Into<String>) -> CommandError {
        CommandError::InvalidParameter {
            command: command.into(),
            param: param.into(),
            reason: reason.into(),
        }
    }

    pub fn invalid(&self, param: &str, reason: impl Into<String>) -> CommandError {
        Self::invalid_for(&self.command, param, reason)
    }

    pub fn failed(&self, reason: impl Into<String>) -> CommandError {
        CommandError::Failed {
            command: self.command.clone(),
            reason: reason.into(),
        }
    }

    pub fn not_found(&self, what: impl Into<String>) -> CommandError {
        CommandError::NotFound {
            command: self.command.clone(),
            what: what.into(),
        }
    }

    fn missing(&self, param: &str) -> CommandError {
        CommandError::MissingParameter {
            command: self.command.clone(),
            param: param.into(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key).filter(|v| !v.is_null())
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Whether `key` is present with an explicit JSON `null` (to clear a setting).
    pub fn is_null(&self, key: &str) -> bool {
        self.values.get(key).is_some_and(Value::is_null)
    }

    pub fn number(
        &self,
        key: &str,
        default: Option<f64>,
        range: Option<(f64, f64)>,
    ) -> Result<f64, CommandError> {
        let n = match self.get(key) {
            None => return default.ok_or_else(|| self.missing(key)),
            Some(v) => v
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| self.invalid(key, "must be a finite number"))?,
        };
        match range {
            Some((lo, hi)) if !(lo..=hi).contains(&n) => {
                Err(self.invalid(key, format!("must be in {lo}...{hi}")))
            }
            _ => Ok(n),
        }
    }

    pub fn req(&self, key: &str, lo: f64, hi: f64) -> Result<f64, CommandError> {
        self.number(key, None, Some((lo, hi)))
    }

    pub fn opt(&self, key: &str, default: f64, lo: f64, hi: f64) -> Result<f64, CommandError> {
        self.number(key, Some(default), Some((lo, hi)))
    }

    pub fn int(
        &self,
        key: &str,
        default: Option<i64>,
        lo: i64,
        hi: i64,
    ) -> Result<i64, CommandError> {
        let n = self.number(key, default.map(|d| d as f64), Some((lo as f64, hi as f64)))?;
        if n.fract() != 0.0 {
            return Err(self.invalid(key, "must be an integer"));
        }
        Ok(n as i64)
    }

    pub fn string(&self, key: &str, default: Option<&str>) -> Result<String, CommandError> {
        match self.get(key) {
            None => default.map(str::to_string).ok_or_else(|| self.missing(key)),
            Some(Value::String(s)) => Ok(s.clone()),
            Some(_) => Err(self.invalid(key, "must be a string")),
        }
    }

    pub fn bool(&self, key: &str, default: Option<bool>) -> Result<bool, CommandError> {
        match self.get(key) {
            None => default.ok_or_else(|| self.missing(key)),
            Some(Value::Bool(b)) => Ok(*b),
            Some(_) => Err(self.invalid(key, "must be true or false")),
        }
    }

    pub fn array(&self, key: &str) -> Result<&Vec<Value>, CommandError> {
        match self.get(key) {
            None => Err(self.missing(key)),
            Some(Value::Array(a)) => Ok(a),
            Some(_) => Err(self.invalid(key, "must be an array")),
        }
    }

    /// A single `id` or an `ids` array.
    pub fn ids(&self) -> Result<Vec<u64>, CommandError> {
        if self.has("ids") {
            return self
                .array("ids")?
                .iter()
                .map(|v| {
                    v.as_u64()
                        .ok_or_else(|| self.invalid("ids", "must contain non-negative integers"))
                })
                .collect();
        }
        Ok(vec![self.int("id", None, 0, i64::MAX)? as u64])
    }

    /// `[[x, y], ...]` in mm → points in 0.1 mm.
    pub fn points(&self, key: &str, minimum: usize) -> Result<Vec<Vec2>, CommandError> {
        let arr = self.array(key)?;
        if arr.len() < minimum || arr.len() > 100_000 {
            return Err(self.invalid(key, format!("needs {minimum}...100000 points")));
        }
        arr.iter().map(|v| self.point_value(key, v)).collect()
    }

    /// One `[x, y]` in mm → a point in 0.1 mm.
    pub fn point_value(&self, key: &str, v: &Value) -> Result<Vec2, CommandError> {
        let xy = v.as_array().filter(|a| a.len() == 2);
        let x = xy
            .and_then(|a| a[0].as_f64())
            .filter(|x| x.is_finite() && x.abs() < 10_000.0);
        let y = xy
            .and_then(|a| a[1].as_f64())
            .filter(|y| y.is_finite() && y.abs() < 10_000.0);
        match (x, y) {
            (Some(x), Some(y)) => Ok(v2(x * 10.0, y * 10.0)),
            _ => Err(self.invalid(key, "each point must be [x, y] in mm within ±10000")),
        }
    }

    /// Required `[x, y]` parameter.
    pub fn point(&self, key: &str) -> Result<Vec2, CommandError> {
        let v = self.get(key).ok_or_else(|| self.missing(key))?;
        self.point_value(key, v)
    }

    /// `[[c1x, c1y], [c2x, c2y]]` in mm → cubic control points, or `None` for JSON `null`.
    pub fn bezier_value(&self, key: &str, v: &Value) -> Result<Option<(Vec2, Vec2)>, CommandError> {
        if v.is_null() {
            return Ok(None);
        }
        match v.as_array() {
            Some(a) if a.len() == 2 => Ok(Some((
                self.point_value(key, &a[0])?,
                self.point_value(key, &a[1])?,
            ))),
            _ => Err(self.invalid(
                key,
                "each curve must be null or [[c1x, c1y], [c2x, c2y]] in mm",
            )),
        }
    }
}
