use serde::ser::SerializeMap;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// [FossilDoc] `Quantity`
/// An analysis output can be combined and folded with other
/// analysis outputs of the same kind. The symmetry is given,
/// and Quantity can teach any value how to combine with others.
///
pub trait Quantity: Sized + Clone {
    fn identity() -> Self;
    fn combine(&self, other: &Self) -> Self;
}

pub fn fold<Q: Quantity>(items: impl IntoIterator<Item = Q>) -> Q {
    items
        .into_iter()
        .fold(Q::identity(), |acc, x| acc.combine(&x))
}

#[derive(Clone)]
pub(crate) struct Scalar {
    n: usize,
    mean: f64,
    m2: f64,
}

impl Scalar {
    fn inject(x: f64) -> Self {
        Self { n: 1, mean: x, m2: 0.0 }
    }

    fn mean(&self) -> f64 {
        if self.n == 0 { 0.0 } else { self.mean }
    }

    fn stddev(&self) -> f64 {
        if self.n < 2 { return 0.0; }
        (self.m2 / (self.n - 1) as f64).sqrt()
    }
}

impl Quantity for Scalar {
    fn identity() -> Self {
        Self { n: 0, mean: 0.0, m2: 0.0 }
    }

    /// Welford's parallel merge for online mean + variance.
    fn combine(&self, other: &Self) -> Self {
        if self.n == 0 { return other.clone(); }
        if other.n == 0 { return self.clone(); }
        let n = self.n + other.n;
        let delta = other.mean - self.mean;
        let mean = self.mean + delta * other.n as f64 / n as f64;
        let m2 = self.m2
            + other.m2
            + delta * delta * (self.n as f64 * other.n as f64) / n as f64;
        Self { n, mean, m2 }
    }
}

impl Serialize for Scalar {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("mean", &self.mean())?;
        map.serialize_entry("stddev", &self.stddev())?;
        map.end()
    }
}

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
            // NOTE: Maybe we should just panic here? If we can't deseraialize
            // the JSON Value into a metric, that is probably an error on the
            // analysis script, or a suprising asymmetry between two analyis
            // outputs.
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
