use std::collections::BTreeMap;
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
    sum: f64,
    sum_sq: f64,
    n: usize,
}

impl Scalar {
    pub fn inject(x: f64) -> Self {
        Self {
            sum: x,
            sum_sq: x * x,
            n: 1,
        }
    }

    pub fn mean(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            self.sum / self.n as f64
        }
    }

    pub fn stddev(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        let m = self.mean();
        ((self.sum_sq / self.n as f64) - m * m).max(0.0).sqrt()
    }

    pub fn to_json(&self) -> Value {
        json!({ "mean": self.mean(), "stddev": self.stddev() })
    }

    pub fn delta(&self, baseline: &Self) -> String {
        let bm = baseline.mean();
        if bm == 0.0 {
            return "-".into();
        }
        let pct = (self.mean() - bm) / bm * 100.0;
        format!("{:+.1}%", pct)
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1} ± {:.1}", self.mean(), self.stddev())
    }
}

impl Quantity for Scalar {
    fn identity() -> Self {
        Self {
            sum: 0.0,
            sum_sq: 0.0,
            n: 0,
        }
    }

    fn combine(&self, other: &Self) -> Self {
        Self {
            sum: self.sum + other.sum,
            sum_sq: self.sum_sq + other.sum_sq,
            n: self.n + other.n,
        }
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

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Scalar)> {
        self.0.iter()
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
        for (name, scalar) in self.iter() {
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


pub struct Comparison<'a> {
    pub baseline: (&'a str, &'a MetricSet),
    pub candidate: (&'a str, &'a MetricSet),
}

impl fmt::Display for Comparison<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let summary = Summary {
            columns: vec![
                (self.baseline.0.to_string(), self.baseline.1.clone()),
                (self.candidate.0.to_string(), self.candidate.1.clone()),
            ],
        };
        write!(f, "{summary}")
    }
}

pub struct Summary {
    pub columns: Vec<(String, MetricSet)>,
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.columns.is_empty() {
            return Ok(());
        }

        let mut all_keys: Vec<String> = Vec::new();
        for (_, ms) in &self.columns {
            for k in ms.keys() {
                if !all_keys.contains(k) {
                    all_keys.push(k.clone());
                }
            }
        }

        let mw = all_keys
            .iter()
            .map(|k| k.len())
            .max()
            .unwrap_or(6)
            .max(6);

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

        let total: usize =
            mw + col_widths.iter().map(|w| w + 3).sum::<usize>()
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
