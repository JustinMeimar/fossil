use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// [Fossil Doc] `Quantity Trait`
/// -------------------------------------------------------------
/// a Quantity represents an abstract type which can can combined
/// with any other Quantity. A set of Quantities forms a monoid.
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

/// [Fossil Doc] `Scalar`
/// -------------------------------------------------------------
/// An analysis script for a Fossil is expected to produce key
/// value pairs. The values should be automatically convertible
/// to a Quantity derived type. Scalar is the simplest kind.
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
        Self {
            n: 0,
            mean: 0.0,
            m2: 0.0,
        }
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

/// [Fossil Doc] `MetricSet`
/// -------------------------------------------------------------
/// A flat collection of named Scalars. Produced from numeric
/// values in analysis script JSON output.
#[derive(Clone)]
pub struct MetricSet(BTreeMap<String, Scalar>);

impl MetricSet {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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

/// [Fossil Doc] `Table`
/// -------------------------------------------------------------
/// An ordered collection of rows, each identified by a key value.
/// Produced from array-of-objects values in analysis script JSON.
/// Each row has the same set of numeric columns.
#[derive(Clone)]
pub struct Table {
    pub key_column: String,
    pub value_columns: Vec<String>,
    pub rows: Vec<(String, Vec<Scalar>)>,
}

impl Table {
    pub fn from_json(arr: &[Value]) -> Option<Self> {
        if arr.is_empty() {
            return None;
        }
        let first = arr[0].as_object()?;
        let mut key_column = None;
        let mut value_columns = Vec::new();
        for (k, v) in first {
            if key_column.is_none() && v.is_string() {
                key_column = Some(k.clone());
            } else if v.is_f64() || v.is_i64() || v.is_u64() {
                value_columns.push(k.clone());
            }
        }
        let key_col = key_column?;
        if value_columns.is_empty() {
            return None;
        }

        let mut rows = Vec::new();
        for item in arr {
            let obj = item.as_object()?;
            let key = obj.get(&key_col)?.as_str()?.to_string();
            let cells: Vec<Scalar> = value_columns
                .iter()
                .map(|col| {
                    obj.get(col)
                        .and_then(|v| v.as_f64())
                        .map(Scalar::inject)
                        .unwrap_or_else(Scalar::identity)
                })
                .collect();
            rows.push((key, cells));
        }

        Some(Table {
            key_column: key_col,
            value_columns,
            rows,
        })
    }

    fn find_row(&self, key: &str) -> Option<&Vec<Scalar>> {
        self.rows.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn to_json(&self) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .map(|(key, cells)| {
                let mut obj = serde_json::Map::new();
                obj.insert(
                    self.key_column.clone(),
                    Value::String(key.clone()),
                );
                for (col, scalar) in self.value_columns.iter().zip(cells) {
                    obj.insert(col.clone(), scalar.to_json());
                }
                Value::Object(obj)
            })
            .collect();
        Value::Array(rows)
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.key_column);
        for col in &self.value_columns {
            out.push(',');
            out.push_str(col);
        }
        out.push('\n');
        for (key, cells) in &self.rows {
            out.push_str(key);
            for s in cells {
                out.push(',');
                out.push_str(&format!("{:.1}", s.mean()));
            }
            out.push('\n');
        }
        out
    }
}

impl Quantity for Table {
    fn identity() -> Self {
        Self {
            key_column: String::new(),
            value_columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    fn combine(&self, other: &Self) -> Self {
        if self.rows.is_empty() {
            return other.clone();
        }
        if other.rows.is_empty() {
            return self.clone();
        }
        let mut rows: Vec<(String, Vec<Scalar>)> = self.rows.clone();
        for (key, other_cells) in &other.rows {
            if let Some(pos) = rows.iter().position(|(k, _)| k == key) {
                let self_cells = &mut rows[pos].1;
                for (i, oc) in other_cells.iter().enumerate() {
                    if i < self_cells.len() {
                        self_cells[i] = self_cells[i].combine(oc);
                    }
                }
            } else {
                rows.push((key.clone(), other_cells.clone()));
            }
        }
        Table {
            key_column: self.key_column.clone(),
            value_columns: self.value_columns.clone(),
            rows,
        }
    }
}

/// [Fossil Doc] `AnalysisResult`
/// -------------------------------------------------------------
/// The composite output of a single analysis parse: flat scalars
/// plus zero or more named tables. This is the monoid that gets
/// folded across observations.
#[derive(Clone)]
pub struct AnalysisResult {
    pub scalars: MetricSet,
    pub tables: BTreeMap<String, Table>,
}

impl AnalysisResult {
    pub fn from_json(value: &Value) -> Self {
        let mut scalars = BTreeMap::new();
        let mut tables = BTreeMap::new();

        if let Some(obj) = value.as_object() {
            for (k, v) in obj {
                if let Some(n) = v.as_f64() {
                    scalars.insert(k.clone(), Scalar::inject(n));
                } else if let Some(arr) = v.as_array() {
                    if let Some(table) = Table::from_json(arr) {
                        tables.insert(k.clone(), table);
                    }
                }
            }
        }

        AnalysisResult {
            scalars: MetricSet(scalars),
            tables,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("scalars".to_string(), self.scalars.to_json());
        let tmap: serde_json::Map<String, Value> = self
            .tables
            .iter()
            .map(|(k, t)| (k.clone(), t.to_json()))
            .collect();
        map.insert("tables".to_string(), Value::Object(tmap));
        Value::Object(map)
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        for (name, table) in &self.tables {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("# {name}\n"));
            out.push_str(&table.to_csv());
        }
        if !self.scalars.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("metric,value\n");
            let keys: Vec<_> = self.scalars.keys().cloned().collect();
            for k in keys {
                if let Some(s) = self.scalars.get(&k) {
                    out.push_str(&format!("{},{:.1}\n", k, s.mean()));
                }
            }
        }
        out
    }
}

impl Quantity for AnalysisResult {
    fn identity() -> Self {
        Self {
            scalars: MetricSet::identity(),
            tables: BTreeMap::new(),
        }
    }

    fn combine(&self, other: &Self) -> Self {
        let mut tables = self.tables.clone();
        for (k, t) in &other.tables {
            tables
                .entry(k.clone())
                .and_modify(|s| *s = s.combine(t))
                .or_insert_with(|| t.clone());
        }
        AnalysisResult {
            scalars: self.scalars.combine(&other.scalars),
            tables,
        }
    }
}

pub struct Summary {
    pub columns: Vec<(String, AnalysisResult)>,
}

impl Summary {
    pub fn to_json(&self) -> Value {
        let map: serde_json::Map<String, Value> = self
            .columns
            .iter()
            .map(|(name, ar)| (name.clone(), ar.to_json()))
            .collect();
        Value::Object(map)
    }
    
    // NOTE: No csv crate? cant there be something as simple as serde? 
    pub fn to_csv(&self) -> String {
        if self.columns.len() == 1 {
            return self.columns[0].1.to_csv();
        }
        let mut out = String::new();
        let all_table_keys: BTreeSet<&String> = self
            .columns
            .iter()
            .flat_map(|(_, ar)| ar.tables.keys())
            .collect();
        for tname in &all_table_keys {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("# {tname}\n"));
            let tables: Vec<Option<&Table>> = self
                .columns
                .iter()
                .map(|(_, ar)| ar.tables.get(*tname))
                .collect();
            let ref_table = tables.iter().find_map(|t| *t);
            if let Some(rt) = ref_table {
                out.push_str(&rt.key_column);
                for col in &rt.value_columns {
                    for (cname, _) in &self.columns {
                        out.push(',');
                        out.push_str(&format!("{col}_{cname}"));
                    }
                }
                out.push('\n');
                let all_row_keys: Vec<&str> = rt
                    .rows
                    .iter()
                    .map(|(k, _)| k.as_str())
                    .collect();
                for rk in &all_row_keys {
                    out.push_str(rk);
                    for (ci, col) in rt.value_columns.iter().enumerate() {
                        for t in &tables {
                            out.push(',');
                            let val = t
                                .and_then(|t| t.find_row(rk))
                                .and_then(|cells| cells.get(ci))
                                .map(|s| format!("{:.1}", s.mean()))
                                .unwrap_or_default();
                            let _ = col;
                            out.push_str(&val);
                        }
                    }
                    out.push('\n');
                }
            }
        }

        let all_scalar_keys: BTreeSet<String> = self
            .columns
            .iter()
            .flat_map(|(_, ar)| ar.scalars.keys().cloned())
            .collect();
        if !all_scalar_keys.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("metric");
            for (cname, _) in &self.columns {
                out.push(',');
                out.push_str(cname);
            }
            out.push('\n');
            for k in &all_scalar_keys {
                out.push_str(k);
                for (_, ar) in &self.columns {
                    out.push(',');
                    if let Some(s) = ar.scalars.get(k) {
                        out.push_str(&format!("{:.1}", s.mean()));
                    }
                }
                out.push('\n');
            }
        }
        out
    }
}

fn fmt_scalar_table(
    f: &mut fmt::Formatter<'_>,
    columns: &[(String, &MetricSet)],
) -> fmt::Result {
    let all_keys: Vec<String> = {
        let mut seen = BTreeSet::new();
        for (_, ms) in columns {
            seen.extend(ms.keys().cloned());
        }
        seen.into_iter().collect()
    };
    if all_keys.is_empty() {
        return Ok(());
    }

    let mw = all_keys.iter().map(|k| k.len()).max().unwrap_or(6).max(6);

    let col_widths: Vec<usize> = columns
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
        let vals: Vec<Option<&Scalar>> =
            columns.iter().map(|(_, ms)| ms.get(key)).collect();
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

fn fmt_data_table(
    f: &mut fmt::Formatter<'_>,
    name: &str,
    columns: &[(String, &Table)],
) -> fmt::Result {
    let ref_table = columns.iter().find_map(|(_, t)| Some(*t));
    let rt = match ref_table {
        Some(t) => t,
        None => return Ok(()),
    };

    writeln!(f, "  {name}")?;

    let kw = rt
        .rows
        .iter()
        .map(|(k, _)| k.len())
        .max()
        .unwrap_or(4)
        .max(rt.key_column.len());

    let ncols = columns.len();
    let nvals = rt.value_columns.len();
    let show_delta = ncols == 2;

    let cell_w = 10usize;

    write!(f, "  {:<kw$}", rt.key_column)?;
    for vcol in &rt.value_columns {
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
        + rt.value_columns.len()
            * (ncols * (cell_w + 3) + if show_delta { 11 } else { 0 });
    if ncols == 1 {
        let total_w = kw + nvals * (cell_w + 3);
        writeln!(f, "  {}", "\u{2500}".repeat(total_w))?;
    } else {
        writeln!(f, "  {}", "\u{2500}".repeat(total_w))?;
    }

    let all_row_keys: Vec<&str> = rt.rows.iter().map(|(k, _)| k.as_str()).collect();
    for rk in &all_row_keys {
        write!(f, "  {:<kw$}", rk)?;
        for (ci, _vcol) in rt.value_columns.iter().enumerate() {
            let vals: Vec<Option<&Scalar>> = columns
                .iter()
                .map(|(_, t)| t.find_row(rk).and_then(|cells| cells.get(ci)))
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
        if self.columns.is_empty() {
            return Ok(());
        }

        let all_table_keys: BTreeSet<&String> = self
            .columns
            .iter()
            .flat_map(|(_, ar)| ar.tables.keys())
            .collect();
        for tname in &all_table_keys {
            let cols: Vec<(String, &Table)> = self
                .columns
                .iter()
                .filter_map(|(name, ar)| {
                    ar.tables.get(*tname).map(|t| (name.clone(), t))
                })
                .collect();
            if !cols.is_empty() {
                fmt_data_table(f, tname, &cols)?;
                writeln!(f)?;
            }
        }

        let has_scalars = self
            .columns
            .iter()
            .any(|(_, ar)| !ar.scalars.is_empty());
        if has_scalars {
            let cols: Vec<(String, &MetricSet)> = self
                .columns
                .iter()
                .map(|(name, ar)| (name.clone(), &ar.scalars))
                .collect();
            fmt_scalar_table(f, &cols)?;
        }

        Ok(())
    }
}
