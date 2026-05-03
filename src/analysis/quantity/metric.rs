use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

use super::Quantity;
use super::scalar::Scalar;

/// [Fossil Doc] `Metric`
/// -------------------------------------------------------------
/// Recursive tree of analysis output. Mirrors the shape of the
/// JSON an analysis script returns. Scalars get folded into
/// mean+stddev, maps and lists recurse, tags pass through.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum Metric {
    Scalar(Scalar),
    Map(BTreeMap<String, Metric>),
    List(Vec<Metric>),
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
                let mut out: Vec<Metric> = a
                    .iter()
                    .zip(b.iter())
                    .map(|(x, y)| x.combine(y))
                    .collect();
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
