use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

use super::Quantity;
use super::scalar::Scalar;

/// [Fossil Doc] `Metric`
/// -------------------------------------------------------------
/// Recursive tree of analysis output, constructed from the shape of
/// an analysis script output. By impl'ing the Quantity trait,
/// Scalars get folded into mean+stddev, maps and lists recurse,
/// tags pass through.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum Metric {
    /// Numeric leaf value, folded into mean+stddev across observations.
    Scalar(Scalar),

    /// Named sub-metrics, preserving the structure of the analysis JSON.
    /// ```json
    /// {
    ///     "cntA": 123,
    ///     "cntB": 567
    /// }
    /// ```
    Map(BTreeMap<String, Metric>),

    /// Positional sequence of sub-metrics, combined element-wise.
    /// ```json
    /// {
    ///     "cntA": [1.0, 2.0, 3.0],
    ///     "cntB": [1.0, 2.0, 3.0]
    /// }
    /// ```
    List(Vec<Metric>),

    /// Opaque string label, passed through without aggregation.
    /// ```json
    /// {
    ///     "benchmark": "speed",
    ///     "results": { ... }
    /// }
    /// ```
    Tag(String),
}

impl Metric {
    pub fn from_json(value: &Value) -> Self {
        match value {
            Value::Number(n) => {
                Metric::Scalar(Scalar::inject(n.as_f64().unwrap_or(0.0)))
            }
            Value::String(s) => Metric::Tag(s.clone()),
            Value::Object(obj) => Metric::Map(
                obj.iter()
                    .map(|(k, v)| (k.clone(), Metric::from_json(v)))
                    .collect(),
            ),
            Value::Array(arr) => {
                Metric::List(arr.iter().map(Metric::from_json).collect())
            }
            _ => Metric::Tag(String::new()),
        }
    }
}

impl Quantity for Metric {
    fn identity() -> Self {
        Metric::Map(BTreeMap::new())
    }
    fn combine(&self, other: &Self) -> Self {
        match (self, other) {
            (Metric::Scalar(a), Metric::Scalar(b)) => {
                Metric::Scalar(a.combine(b))
            }
            (Metric::Map(a), Metric::Map(b)) => {
                let mut map = a.clone();
                for (k, v) in b {
                    map.entry(k.clone())
                        .and_modify(|e| *e = e.combine(v))
                        .or_insert_with(|| v.clone());
                }
                Metric::Map(map)
            }
            (Metric::List(a), Metric::List(b)) => {
                // Zip shared indices, then extend with the longer tail.
                let mut out: Vec<Metric> =
                    a.iter().zip(b.iter()).map(|(x, y)| x.combine(y)).collect();
                if a.len() > b.len() {
                    out.extend_from_slice(&a[b.len()..]);
                } else if b.len() > a.len() {
                    out.extend_from_slice(&b[a.len()..]);
                }
                Metric::List(out)
            }
            _ => self.clone(),
        }
    }
}
