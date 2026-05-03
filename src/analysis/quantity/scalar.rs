use serde::ser::SerializeMap;
use serde::Serialize;

use super::Quantity;

/// [Fossil Doc] `Scalar`
/// -------------------------------------------------------------
/// Online mean + variance via Welford's algorithm. Two Scalars
/// can be merged without revisiting the original samples, so we
/// can fold across iterations cheaply.
#[derive(Clone)]
pub(crate) struct Scalar {
    n: usize,
    mean: f64,
    m2: f64,
}

impl Scalar {
    pub fn inject(x: f64) -> Self {
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
