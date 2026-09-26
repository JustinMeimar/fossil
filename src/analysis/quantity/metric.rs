use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

use super::scalar::Scalar;

/// [Fossil Doc] `Metric`
/// -------------------------------------------------------------
/// A recursive tree preserving the shape of an analysis script's JSON.
/// The first observation seeds the tree; subsequent observations merge
/// numeric leaves into mean and sample standard deviation. Distinct object
/// keys retain separate samples; shared keys merge recursively.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum Metric {
    /// Numeric leaf accumulated with Welford's algorithm.
    Scalar(Scalar),
    /// Named sub-metrics, merged by matching keys.
    /// ```json
    /// {
    ///     "cntA": 123,
    ///     "cntB": 567
    /// }
    /// ```
    Map(BTreeMap<String, Metric>),
    /// Positional sub-metrics, merged element-wise.
    /// ```json
    /// {
    ///     "cntA": [1.0, 2.0, 3.0],
    ///     "cntB": [1.0, 2.0, 3.0]
    /// }
    /// ```
    List(Vec<Metric>),
    /// Opaque string label, preserved unchanged across observations.
    /// ```json
    /// {
    ///     "benchmark": "speed",
    ///     "results": { ... }
    /// }
    /// ```
    Tag(String),
}

impl Metric {
    pub fn from_json(value: Value) -> Result<Self, String> {
        Self::parse_at(value, "$")
    }

    fn parse_at(value: Value, path: &str) -> Result<Self, String> {
        match value {
            Value::Number(number) => number
                .as_f64()
                .map(|value| Self::Scalar(Scalar::inject(value)))
                .ok_or_else(|| {
                    format!("{path}: number cannot be represented as f64")
                }),
            Value::String(value) => Ok(Self::Tag(value)),
            Value::Object(values) => values
                .into_iter()
                .map(|(key, value)| {
                    let metric =
                        Self::parse_at(value, &format!("{path}[{key:?}]"))?;
                    Ok((key, metric))
                })
                .collect::<Result<_, String>>()
                .map(Self::Map),
            Value::Array(values) => values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    Self::parse_at(value, &format!("{path}[{index}]"))
                })
                .collect::<Result<_, _>>()
                .map(Self::List),
            Value::Bool(_) => {
                Err(format!("{path}: booleans are not supported in metrics"))
            }
            Value::Null => {
                Err(format!("{path}: null is not supported in metrics"))
            }
        }
    }

    /// Discard the accumulator if merging fails; earlier leaves may have changed.
    pub fn merge(&mut self, other: Self) -> Result<(), String> {
        self.merge_at(other, "$")
    }

    fn merge_at(&mut self, other: Self, path: &str) -> Result<(), String> {
        match (self, other) {
            (Self::Scalar(left), Self::Scalar(right)) => left.merge(right),
            (Self::Map(left), Self::Map(right)) => {
                for (key, metric) in right {
                    let child_path = format!("{path}[{key:?}]");
                    match left.entry(key) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(metric);
                        }
                        std::collections::btree_map::Entry::Occupied(
                            mut entry,
                        ) => {
                            entry.get_mut().merge_at(metric, &child_path)?;
                        }
                    }
                }
            }
            (Self::List(left), Self::List(right)) => {
                if left.len() != right.len() {
                    return Err(format!(
                        "{path}: list lengths differ: {} versus {}",
                        left.len(),
                        right.len()
                    ));
                }
                for (index, (left, right)) in
                    left.iter_mut().zip(right).enumerate()
                {
                    left.merge_at(right, &format!("{path}[{index}]"))?;
                }
            }
            (Self::Tag(left), Self::Tag(right)) => {
                if *left != right {
                    return Err(format!(
                        "{path}: labels differ: {left:?} versus {right:?}"
                    ));
                }
            }
            (left, right) => {
                return Err(format!(
                    "{path}: metric types differ: {} versus {}",
                    left.kind(),
                    right.kind()
                ));
            }
        }
        Ok(())
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Scalar(_) => "number",
            Self::Map(_) => "object",
            Self::List(_) => "list",
            Self::Tag(_) => "string",
        }
    }
}
