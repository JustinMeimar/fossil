use std::io::{BufReader, Read};
use std::path::Path;

use serde::Deserialize;

use maud::{Markup, html};
use serde_json::Value;

use super::{Selection, view};
use crate::error::FossilError;

const TEXT_BYTES: usize = 64 * 1024;
const JSON_BYTES: usize = 1024 * 1024;
const STREAM_LINES: usize = 200;

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
    let content = if selection.record.is_some() {
        record_content(BufReader::new(std::fs::File::open(path)?))
            .or_else(|_| file_content(path, &extension, &raw))?
    } else {
        file_content(path, &extension, &raw)?
    };
    Ok(document(content, &download))
}

fn file_content(
    path: &Path,
    extension: &str,
    raw: &str,
) -> Result<Markup, FossilError> {
    Ok(match extension {
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
    })
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
    let json = serde_json::from_str::<Value>(text)
        .map(|value| serde_json::to_string_pretty(&value).unwrap())
        .unwrap_or_else(|_| text.to_owned());
    html! { pre { (json) } }
}

#[derive(Deserialize)]
struct RecordPreview {
    observations: Vec<ObservationPreview>,
}

#[derive(Deserialize)]
struct ObservationPreview {
    #[serde(flatten)]
    metadata: serde_json::Map<String, Value>,
    #[serde(deserialize_with = "preview_lines")]
    stdout: Vec<String>,
    #[serde(deserialize_with = "preview_lines")]
    stderr: Vec<String>,
}

fn preview_lines<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    struct Lines;
    impl<'de> serde::de::Visitor<'de> for Lines {
        type Value = Vec<String>;

        fn expecting(
            &self,
            formatter: &mut std::fmt::Formatter,
        ) -> std::fmt::Result {
            formatter.write_str("an array of output lines")
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut lines = Vec::new();
            while lines.len() <= STREAM_LINES {
                match sequence.next_element::<String>()? {
                    Some(line) => lines.push(line),
                    None => return Ok(lines),
                }
            }
            while sequence
                .next_element::<serde::de::IgnoredAny>()?
                .is_some()
            {}
            Ok(lines)
        }
    }
    deserializer.deserialize_seq(Lines)
}

fn stream_content(mut lines: Vec<String>) -> Value {
    let complete = lines.len() <= STREAM_LINES;
    lines.truncate(STREAM_LINES);
    if complete && let Ok(value) = serde_json::from_str(&lines.join("\n")) {
        return value;
    }
    Value::Array(
        lines
            .into_iter()
            .map(|line| {
                serde_json::from_str(&line).unwrap_or(Value::String(line))
            })
            .collect(),
    )
}

fn record_content(reader: impl Read) -> Result<Markup, FossilError> {
    let record: RecordPreview =
        serde_json::from_reader(reader).map_err(|error| {
            FossilError::InvalidConfig(format!("invalid recording: {error}"))
        })?;
    let truncated = record.observations.iter().any(|observation| {
        observation.stdout.len() > STREAM_LINES
            || observation.stderr.len() > STREAM_LINES
    });
    let observations: Vec<_> = record
        .observations
        .into_iter()
        .map(|observation| {
            let mut value = observation.metadata;
            value.insert("stdout".into(), stream_content(observation.stdout));
            value.insert("stderr".into(), stream_content(observation.stderr));
            Value::Object(value)
        })
        .collect();
    let text = serde_json::to_string_pretty(
        &serde_json::json!({"observations": observations}),
    )
    .unwrap();
    Ok(html! {
        p.muted { "JSON output is expanded for display. Downloads retain the original recording." }
        @if truncated {
            p.muted { "Preview shows the first " (STREAM_LINES) " lines per output stream. Download the full file for more." }
        }
        (text_content(text.as_bytes()))
    })
}
