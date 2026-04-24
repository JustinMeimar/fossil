use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use serde_json::{Value, json};

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
pub struct Scalar {
    n: usize,
    mean: f64,
    m2: f64,
}

impl Scalar {
    pub fn inject(x: f64) -> Self {
        Self {
            n: 1,
            mean: x,
            m2: 0.0,
        }
    }

    pub fn mean(&self) -> f64 {
        if self.n == 0 { 0.0 } else { self.mean }
    }

    pub fn stddev(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        (self.m2 / (self.n - 1) as f64).sqrt()
    }

    pub fn to_json(&self) -> Value {
        json!({ "mean": self.mean(), "stddev": self.stddev() })
    }

    fn delta(&self, baseline: &Self) -> String {
        let bm = baseline.mean();
        if bm == 0.0 {
            return "-".into();
        }
        format!("{:+.1}%", (self.mean() - bm) / bm * 100.0)
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1} ± {:.1}", self.mean(), self.stddev())
    }
}

impl Quantity for Scalar {
    fn identity() -> Self {
        Self { n: 0, mean: 0.0, m2: 0.0 }
    }

    fn combine(&self, other: &Self) -> Self {
        if self.n == 0 {
            return other.clone();
        }
        if other.n == 0 {
            return self.clone();
        }
        let n = self.n + other.n;
        let delta = other.mean - self.mean;
        let mean = self.mean + delta * other.n as f64 / n as f64;
        let m2 = self.m2
            + other.m2
            + delta * delta * (self.n as f64 * other.n as f64) / n as f64;
        Self { n, mean, m2 }
    }
}

#[derive(Clone)]
pub struct MetricSet(BTreeMap<String, Scalar>);

impl MetricSet {
    pub fn from_json(value: &Value) -> Self {
        let mut map = BTreeMap::new();
        if let Some(obj) = value.as_object() {
            for (k, v) in obj {
                if let Some(n) = v.as_f64() {
                    map.insert(k.clone(), Scalar::inject(n));
                }
            }
        }
        Self(map)
    }

    pub fn get(&self, key: &str) -> Option<&Scalar> {
        self.0.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    pub fn to_json(&self) -> Value {
        let map: serde_json::Map<String, Value> = self
            .0
            .iter()
            .map(|(k, v)| (k.clone(), v.to_json()))
            .collect();
        Value::Object(map)
    }
}

impl fmt::Display for MetricSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, scalar) in &self.0 {
            writeln!(f, "  {name}: {scalar}")?;
        }
        Ok(())
    }
}

impl Quantity for MetricSet {
    fn identity() -> Self {
        Self(BTreeMap::new())
    }

    fn combine(&self, other: &Self) -> Self {
        let mut map = self.0.clone();
        for (k, v) in &other.0 {
            map.entry(k.clone())
                .and_modify(|s| *s = s.combine(v))
                .or_insert_with(|| v.clone());
        }
        Self(map)
    }
}

pub struct Summary {
    pub columns: Vec<(String, MetricSet)>,
}

impl Summary {
    pub fn to_json(&self) -> Value {
        let map: serde_json::Map<String, Value> = self
            .columns
            .iter()
            .map(|(name, ms)| (name.clone(), ms.to_json()))
            .collect();
        Value::Object(map)
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.columns.is_empty() {
            return Ok(());
        }

        let all_keys: Vec<String> = {
            let mut seen = BTreeSet::new();
            for (_, ms) in &self.columns {
                seen.extend(ms.keys().cloned());
            }
            seen.into_iter().collect()
        };

        let mw = all_keys.iter().map(|k| k.len()).max().unwrap_or(6).max(6);

        let col_widths: Vec<usize> = self
            .columns
            .iter()
            .map(|(name, ms)| {
                let val_w = all_keys
                    .iter()
                    .filter_map(|k| ms.get(k))
                    .map(|s| format!("{:.1}", s.mean()).len())
                    .max()
                    .unwrap_or(0);
                name.len().max(val_w).max(8)
            })
            .collect();

        write!(f, "  {:<mw$}", "metric")?;
        for (i, (name, _)) in self.columns.iter().enumerate() {
            write!(f, "   {:>w$}", name, w = col_widths[i])?;
        }
        if self.columns.len() == 2 {
            write!(f, "   {:>8}", "delta")?;
        }
        writeln!(f)?;

        let total: usize = mw
            + col_widths.iter().map(|w| w + 3).sum::<usize>()
            + if self.columns.len() == 2 { 11 } else { 0 };
        writeln!(f, "  {}", "─".repeat(total))?;

        for key in &all_keys {
            write!(f, "  {:<mw$}", key)?;
            let vals: Vec<Option<&Scalar>> =
                self.columns.iter().map(|(_, ms)| ms.get(key)).collect();
            for (i, v) in vals.iter().enumerate() {
                let s = v
                    .map(|s| format!("{:.1}", s.mean()))
                    .unwrap_or_else(|| "-".into());
                write!(f, "   {:>w$}", s, w = col_widths[i])?;
            }
            if self.columns.len() == 2 {
                let delta = match (&vals[1], &vals[0]) {
                    (Some(c), Some(b)) => c.delta(b),
                    _ => "-".into(),
                };
                write!(f, "   {:>8}", delta)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
