use std::io::Read;
use std::path::Path;

use maud::{Markup, html};
use serde_json::Value;

use super::{Selection, view};
use crate::error::FossilError;

const TEXT_BYTES: usize = 64 * 1024;
const JSON_BYTES: usize = 1024 * 1024;
const TABLE_ROWS: usize = 200;

pub(super) fn file(
    path: &Path,
    selection: &Selection,
) -> Result<Markup, FossilError> {
    let query = serde_urlencoded::to_string(Selection {
        download: None,
        ..selection.clone()
    })
    .unwrap();
    let raw = format!("/file?{query}");
    let download = format!("{raw}&download=true");
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let content = match extension.as_str() {
        "pdf" => html! { iframe src=(&raw) title="PDF preview" {} },
        "png" | "jpg" | "jpeg" | "svg" | "webp" => {
            html! { img src=(&raw) alt="Artifact preview"; }
        }
        "json" | "txt" | "csv" | "log" | "md" => {
            let limit = if extension == "json" {
                JSON_BYTES
            } else {
                TEXT_BYTES
            };
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take((limit + 1) as u64)
                .read_to_end(&mut bytes)?;
            if extension == "json" && bytes.len() <= JSON_BYTES {
                json_content(&String::from_utf8_lossy(&bytes))
            } else {
                text_content(&bytes)
            }
        }
        _ => html! { p { "Preview unavailable for this file." } },
    };
    Ok(document(content, &download))
}

fn document(content: Markup, download: &str) -> Markup {
    view::document(
        "Output",
        "output",
        html! {
          nav { a href=(download) download { "Download full file" } }
          (content)
        },
    )
}

pub(super) fn json_document(text: &str, download: &str) -> Markup {
    document(json_content(text), download)
}

fn text_content(bytes: &[u8]) -> Markup {
    let truncated = bytes.len() > TEXT_BYTES;
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(TEXT_BYTES)]);
    html! {
      @if truncated { p.muted { "Preview limited to the first 64 KiB. Download the full file for more." } }
      pre { (text) }
    }
}

fn json_content(text: &str) -> Markup {
    if text.len() > JSON_BYTES {
        return text_content(text.as_bytes());
    }
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return text_content(text.as_bytes());
    };
    let mut rows = Vec::new();
    json_rows(&value, "", &mut rows);
    html! {
      @if rows.len() > TABLE_ROWS { p.muted { "Showing the first 200 values. Download the full file for more." } }
      p.muted { "Long values are shortened to 512 characters." }
      table {
        thead { tr { th { "Path" } th { "Value" } } }
        tbody {
          @for (path, value) in rows.iter().take(TABLE_ROWS) {
            tr { td { (path) } td { (value) } }
          }
        }
      }
    }
}

fn json_rows(value: &Value, path: &str, rows: &mut Vec<(String, String)>) {
    if rows.len() > TABLE_ROWS {
        return;
    }
    match value {
        Value::Object(fields) if !fields.is_empty() => {
            for (key, value) in fields {
                if rows.len() > TABLE_ROWS {
                    break;
                }
                json_rows(
                    value,
                    &format!(
                        "{path}/{}",
                        key.replace('~', "~0").replace('/', "~1")
                    ),
                    rows,
                );
            }
        }
        Value::Array(values) if !values.is_empty() => {
            for (index, value) in values.iter().enumerate() {
                if rows.len() > TABLE_ROWS {
                    break;
                }
                json_rows(value, &format!("{path}/{index}"), rows);
            }
        }
        _ => {
            let text = match value {
                Value::String(text) => text.clone(),
                _ => value.to_string(),
            };
            let mut shortened: String = text.chars().take(512).collect();
            if shortened.len() < text.len() {
                shortened.push('…');
            }
            rows.push((path.to_owned(), shortened));
        }
    }
}
