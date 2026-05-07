use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::record::Record;

pub struct VariantColumn {
    pub name: String,
    pub record_indices: Vec<usize>,
}

pub struct VariantGrid {
    pub columns: Vec<VariantColumn>,
    pub col: usize,
    pub row: usize,
    pub col_offset: usize,
    pub scroll_offsets: Vec<usize>,
}

impl VariantGrid {
    pub fn from_records(records: &[Record]) -> Self {
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, r) in records.iter().enumerate() {
            let v = r
                .manifest
                .variant
                .as_deref()
                .unwrap_or("untagged")
                .to_string();
            groups.entry(v).or_default().push(i);
        }
        let columns: Vec<VariantColumn> = groups
            .into_iter()
            .map(|(name, record_indices)| VariantColumn {
                name,
                record_indices,
            })
            .collect();
        let n = columns.len();
        Self {
            columns,
            col: 0,
            row: 0,
            col_offset: 0,
            scroll_offsets: vec![0; n],
        }
    }

    pub fn current_record_idx(&self) -> Option<usize> {
        self.columns
            .get(self.col)
            .and_then(|c| c.record_indices.get(self.row))
            .copied()
    }

    pub fn ensure_visible(&mut self, visible_rows: usize) {
        if let Some(off) = self.scroll_offsets.get_mut(self.col) {
            if self.row < *off {
                *off = self.row;
            } else if self.row >= *off + visible_rows {
                *off = self.row - visible_rows + 1;
            }
        }
    }

    pub fn ensure_col_visible(&mut self, visible_cols: usize) {
        if self.col < self.col_offset {
            self.col_offset = self.col;
        } else if self.col >= self.col_offset + visible_cols {
            self.col_offset = self.col - visible_cols + 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.columns.is_empty() {
            return false;
        }
        let col_len = self.columns[self.col]
            .record_indices
            .len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.row + 1 < col_len {
                    self.row += 1;
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.row = self.row.saturating_sub(1);
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.col > 0 {
                    self.col -= 1;
                    let n = self.columns[self.col]
                        .record_indices
                        .len();
                    self.row = self.row.min(n.saturating_sub(1));
                }
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.col + 1 < self.columns.len() {
                    self.col += 1;
                    let n = self.columns[self.col]
                        .record_indices
                        .len();
                    self.row = self.row.min(n.saturating_sub(1));
                }
                true
            }
            KeyCode::Char('g') => {
                self.row = 0;
                true
            }
            KeyCode::Char('G') => {
                self.row = col_len.saturating_sub(1);
                true
            }
            KeyCode::Char('d')
                if key
                    .modifiers
                    .contains(KeyModifiers::CONTROL) =>
            {
                self.row = (self.row + 6).min(col_len.saturating_sub(1));
                true
            }
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(KeyModifiers::CONTROL) =>
            {
                self.row = self.row.saturating_sub(6);
                true
            }
            _ => false,
        }
    }
}
