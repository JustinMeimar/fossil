use serde::Serialize;
use serde::ser::SerializeMap;

/// [Fossil Doc] `Scalar`
/// -------------------------------------------------------------
/// Welford's mean and sample variance accumulator. Accumulators merge
/// without revisiting their original samples.
#[derive(Clone)]
pub(crate) struct Scalar {
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

    pub fn merge(&mut self, other: Self) {
        let n = self.n + other.n;
        let delta = other.mean - self.mean;
        self.mean += delta * other.n as f64 / n as f64;
        self.m2 += other.m2
            + delta * delta * (self.n as f64 * other.n as f64) / n as f64;
        self.n = n;
    }

    fn stddev(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        (self.m2 / (self.n - 1) as f64).sqrt()
    }
}

impl Serialize for Scalar {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("mean", &self.mean)?;
        map.serialize_entry("stddev", &self.stddev())?;
        map.end()
    }
}
