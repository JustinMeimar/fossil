use std::collections::BTreeMap;
use std::fmt;
use serde_json::Value;

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
        let (bn, base) = self.baseline;
        let (cn, cand) = self.candidate;

        let mut all_keys: Vec<&String> = base.keys().collect();
        for k in cand.keys() {
            if !all_keys.contains(&k) {
                all_keys.push(k);
            }
        }

        let bw = bn.len().max(10);
        let cw = cn.len().max(10);

        writeln!(
            f,
            "  {:<20} {:>bw$}   {:>cw$}   {:>8}",
            "metric", bn, cn, "delta"
        )?;
        writeln!(f, "  {}", "─".repeat(20 + bw + cw + 14))?;

        for key in &all_keys {
            let b = base.get(key);
            let c = cand.get(key);
            let b_str = b
                .map(|s| format!("{:.1}", s.mean()))
                .unwrap_or_else(|| "-".into());
            let c_str = c
                .map(|s| format!("{:.1}", s.mean()))
                .unwrap_or_else(|| "-".into());
            let delta_str = match (c, b) {
                (Some(cv), Some(bv)) => cv.delta(bv),
                _ => "-".into(),
            };
            writeln!(
                f,
                "  {:<20} {:>bw$}   {:>cw$}   {:>8}",
                key, b_str, c_str, delta_str
            )?;
        }
        Ok(())
    }
}
