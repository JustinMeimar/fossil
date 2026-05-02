use serde::ser::SerializeMap;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub trait Quantity: Sized + Clone {
    fn identity() -> Self;
    fn combine(&self, other: &Self) -> Self;
}

pub fn fold<Q: Quantity>(items: impl IntoIterator<Item = Q>) -> Q {
    items
        .into_iter()
        .fold(Q::identity(), |acc, x| acc.combine(&x))
}

// ── Scalar ──────────────────────────────────────────────────────

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

    fn delta(&self, baseline: &Self) -> String {
        let bm = baseline.mean();
        if bm == 0.0 { return "-".into(); }
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

// ── Metric ───────────────────��──────────────────────────────────

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

pub struct Summary {
    pub columns: Vec<(String, Metric)>,
}

impl Serialize for Summary {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.columns.len()))?;
        for (name, metric) in &self.columns {
            map.serialize_entry(name, metric)?;
        }
        map.end()
    }
}

fn csv_row(fields: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut out = String::new();
    for (i, f) in fields.into_iter().enumerate() {
        if i > 0 { out.push(','); }
        out.push_str(f.as_ref());
    }
    out.push('\n');
    out
}
// At display time we "lower" the recursive Metric tree into flat
// and tabular views. A Map whose values are all Scalars renders as
// a key-value metric table; a List of Maps renders as rows.

fn scalar_entries(m: &Metric) -> BTreeMap<&str, &Scalar> {
    match m {
        Metric::Map(map) => map
            .iter()
            .filter_map(|(k, v)| match v {
                Metric::Scalar(s) => Some((k.as_str(), s)),
                _ => None,
            })
            .collect(),
        _ => BTreeMap::new(),
    }
}

fn list_entries(m: &Metric) -> BTreeMap<&str, &[Metric]> {
    match m {
        Metric::Map(map) => map
            .iter()
            .filter_map(|(k, v)| match v {
                Metric::List(l) => Some((k.as_str(), l.as_slice())),
                _ => None,
            })
            .collect(),
        _ => BTreeMap::new(),
    }
}

struct TableView<'a> {
    key_col: &'a str,
    val_cols: Vec<&'a str>,
    rows: Vec<(&'a str, Vec<&'a Scalar>)>,
}

impl<'a> TableView<'a> {
    fn from_list(list: &'a [Metric]) -> Option<Self> {
        let first = match list.first()? {
            Metric::Map(m) => m,
            _ => return None,
        };

        let mut key_col = None;
        let mut val_cols = Vec::new();
        for (k, v) in first {
            match v {
                Metric::Tag(_) if key_col.is_none() => key_col = Some(k.as_str()),
                Metric::Scalar(_) => val_cols.push(k.as_str()),
                _ => {}
            }
        }
        let key_col = key_col?;
        if val_cols.is_empty() { return None; }

        let mut rows = Vec::new();
        for item in list {
            if let Metric::Map(m) = item {
                let key = match m.get(key_col) {
                    Some(Metric::Tag(s)) => s.as_str(),
                    _ => continue,
                };
                let cells: Vec<&Scalar> = val_cols
                    .iter()
                    .filter_map(|col| match m.get(*col) {
                        Some(Metric::Scalar(s)) => Some(s),
                        _ => None,
                    })
                    .collect();
                if cells.len() == val_cols.len() {
                    rows.push((key, cells));
                }
            }
        }
        Some(TableView { key_col, val_cols, rows })
    }

    fn row(&self, key: &str) -> Option<&[&'a Scalar]> {
        self.rows
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, cells)| cells.as_slice())
    }
}

fn fmt_scalars(
    f: &mut fmt::Formatter<'_>,
    columns: &[(String, BTreeMap<&str, &Scalar>)],
) -> fmt::Result {
    let all_keys: BTreeSet<&str> = columns
        .iter()
        .flat_map(|(_, m)| m.keys().copied())
        .collect();
    if all_keys.is_empty() { return Ok(()); }

    let mw = all_keys.iter().map(|k| k.len()).max().unwrap_or(6).max(6);

    let col_widths: Vec<usize> = columns
        .iter()
        .map(|(name, m)| {
            let val_w = all_keys
                .iter()
                .filter_map(|k| m.get(k))
                .map(|s| format!("{:.1}", s.mean()).len())
                .max()
                .unwrap_or(0);
            name.len().max(val_w).max(8)
        })
        .collect();

    write!(f, "  {:<mw$}", "metric")?;
    for (i, (name, _)) in columns.iter().enumerate() {
        write!(f, "   {:>w$}", name, w = col_widths[i])?;
    }
    if columns.len() == 2 {
        write!(f, "   {:>8}", "delta")?;
    }
    writeln!(f)?;

    let total: usize = mw
        + col_widths.iter().map(|w| w + 3).sum::<usize>()
        + if columns.len() == 2 { 11 } else { 0 };
    writeln!(f, "  {}", "\u{2500}".repeat(total))?;

    for key in &all_keys {
        write!(f, "  {:<mw$}", key)?;
        let vals: Vec<Option<&&Scalar>> =
            columns.iter().map(|(_, m)| m.get(key)).collect();
        for (i, v) in vals.iter().enumerate() {
            let s = v
                .map(|s| format!("{:.1}", s.mean()))
                .unwrap_or_else(|| "-".into());
            write!(f, "   {:>w$}", s, w = col_widths[i])?;
        }
        if columns.len() == 2 {
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

fn fmt_table(
    f: &mut fmt::Formatter<'_>,
    name: &str,
    columns: &[(String, TableView<'_>)],
) -> fmt::Result {
    let rt = &columns[0].1;

    writeln!(f, "  {name}")?;

    let kw = rt.rows.iter().map(|(k, _)| k.len()).max().unwrap_or(4).max(rt.key_col.len());
    let ncols = columns.len();
    let nvals = rt.val_cols.len();
    let show_delta = ncols == 2;
    let cell_w = 10usize;

    write!(f, "  {:<kw$}", rt.key_col)?;
    for vcol in &rt.val_cols {
        if ncols == 1 {
            write!(f, "   {:>w$}", vcol, w = cell_w)?;
        } else {
            for (cname, _) in columns {
                let header = if nvals > 1 {
                    format!("{vcol}/{cname}")
                } else {
                    cname.clone()
                };
                write!(f, "   {:>w$}", header, w = cell_w)?;
            }
            if show_delta {
                write!(f, "   {:>8}", "delta")?;
            }
        }
    }
    writeln!(f)?;

    let total_w = kw
        + rt.val_cols.len()
            * (ncols * (cell_w + 3) + if show_delta { 11 } else { 0 });
    if ncols == 1 {
        let total_w = kw + nvals * (cell_w + 3);
        writeln!(f, "  {}", "\u{2500}".repeat(total_w))?;
    } else {
        writeln!(f, "  {}", "\u{2500}".repeat(total_w))?;
    }

    for (rk, _) in &rt.rows {
        write!(f, "  {:<kw$}", rk)?;
        for (ci, _) in rt.val_cols.iter().enumerate() {
            let vals: Vec<Option<&Scalar>> = columns
                .iter()
                .map(|(_, tv)| tv.row(rk).and_then(|cells| cells.get(ci).copied()))
                .collect();
            for v in &vals {
                let s = v
                    .map(|s| format!("{:.1}", s.mean()))
                    .unwrap_or_else(|| "-".into());
                write!(f, "   {:>w$}", s, w = cell_w)?;
            }
            if show_delta {
                let delta = match (&vals[1], &vals[0]) {
                    (Some(c), Some(b)) => c.delta(b),
                    _ => "-".into(),
                };
                write!(f, "   {:>8}", delta)?;
            }
        }
        writeln!(f)?;
    }
    Ok(())
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.columns.is_empty() { return Ok(()); }

        let all_list_keys: BTreeSet<&str> = self
            .columns
            .iter()
            .flat_map(|(_, m)| list_entries(m).into_keys())
            .collect();

        for tname in &all_list_keys {
            let cols: Vec<(String, TableView<'_>)> = self
                .columns
                .iter()
                .filter_map(|(name, m)| {
                    let lists = list_entries(m);
                    let list = lists.get(tname)?;
                    TableView::from_list(list).map(|tv| (name.clone(), tv))
                })
                .collect();
            if !cols.is_empty() {
                fmt_table(f, tname, &cols)?;
                writeln!(f)?;
            }
        }

        let scalar_cols: Vec<(String, BTreeMap<&str, &Scalar>)> = self
            .columns
            .iter()
            .map(|(name, m)| (name.clone(), scalar_entries(m)))
            .collect();
        if scalar_cols.iter().any(|(_, m)| !m.is_empty()) {
            fmt_scalars(f, &scalar_cols)?;
        }

        Ok(())
    }
}

impl Summary {
    pub fn to_csv(&self) -> String {
        let mut out = String::new();

        let all_list_keys: BTreeSet<&str> = self
            .columns
            .iter()
            .flat_map(|(_, m)| list_entries(m).into_keys())
            .collect();

        for tname in &all_list_keys {
            if !out.is_empty() { out.push('\n'); }
            out.push_str(&format!("# {tname}\n"));

            let tables: Vec<Option<TableView<'_>>> = self
                .columns
                .iter()
                .map(|(_, m)| {
                    list_entries(m)
                        .get(tname)
                        .and_then(|list| TableView::from_list(list))
                })
                .collect();

            let ref_table = tables.iter().find_map(|t| t.as_ref());
            if let Some(rt) = ref_table {
                let mut header = vec![rt.key_col.to_string()];
                if self.columns.len() == 1 {
                    header.extend(rt.val_cols.iter().map(|s| s.to_string()));
                } else {
                    for col in &rt.val_cols {
                        for (cname, _) in &self.columns {
                            header.push(format!("{col}_{cname}"));
                        }
                    }
                }
                out.push_str(&csv_row(&header));

                for (rk, _) in &rt.rows {
                    let mut row = vec![rk.to_string()];
                    if self.columns.len() == 1 {
                        if let Some(cells) = rt.row(rk) {
                            row.extend(cells.iter().map(|s| format!("{:.1}", s.mean())));
                        }
                    } else {
                        for (ci, _) in rt.val_cols.iter().enumerate() {
                            for t in &tables {
                                row.push(
                                    t.as_ref()
                                        .and_then(|t| t.row(rk))
                                        .and_then(|cells| cells.get(ci))
                                        .map(|s| format!("{:.1}", s.mean()))
                                        .unwrap_or_default(),
                                );
                            }
                        }
                    }
                    out.push_str(&csv_row(&row));
                }
            }
        }

        let all_scalar_keys: BTreeSet<String> = self
            .columns
            .iter()
            .flat_map(|(_, m)| scalar_entries(m).into_keys().map(str::to_string))
            .collect();

        if !all_scalar_keys.is_empty() {
            if !out.is_empty() { out.push('\n'); }
            if self.columns.len() == 1 {
                out.push_str(&csv_row(["metric", "value"]));
            } else {
                let mut header = vec!["metric".to_string()];
                header.extend(self.columns.iter().map(|(c, _)| c.clone()));
                out.push_str(&csv_row(&header));
            }
            for k in &all_scalar_keys {
                let mut row = vec![k.clone()];
                for (_, m) in &self.columns {
                    let scalars = scalar_entries(m);
                    row.push(
                        scalars
                            .get(k.as_str())
                            .map(|s| format!("{:.1}", s.mean()))
                            .unwrap_or_default(),
                    );
                }
                out.push_str(&csv_row(&row));
            }
        }

        out
    }
}
