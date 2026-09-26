use maud::{DOCTYPE, Markup, html};

use super::Selection;
use crate::fossil::{ConfigurationKey, Fossil};
use crate::project::Project;
use crate::record::Record;

fn document(title: &str, class: &str, content: Markup) -> Markup {
    html! {
      (DOCTYPE)
      html lang="en" {
        head {
          meta charset="utf-8";
          meta name="viewport" content="width=device-width, initial-scale=1";
          title { (title) }
          link rel="stylesheet" href="/style.css";
          script src="/app.js" defer {}
        }
        body class=(class) { (content) }
      }
    }
}

fn link(label: &str, selection: Selection, output: bool) -> Markup {
    let target = if selection.artifact.is_some() {
        "artifacts"
    } else {
        "output"
    };
    let query = serde_urlencoded::to_string(selection).unwrap();
    let path = if output { "/output" } else { "/" };
    html! {
      a href=(format!("{path}?{query}")) target=[output.then_some(target)] {
        (label)
      }
    }
}

pub(super) fn render_chunk(
    selection: &Selection,
    text: &str,
    pages: u64,
) -> Markup {
    let page = selection.page.unwrap_or(0);
    document(
        "Output",
        "output",
        html! {
          nav aria-label="Output pages" {
            @if page > 0 {
              (link("Previous", Selection {
                page: Some(page - 1), ..selection.clone()
              }, true))
            }
            span { "Chunk " (page + 1) " of " (pages) }
            @if page + 1 < pages {
              (link("Next", Selection {
                page: Some(page + 1), ..selection.clone()
              }, true))
            }
          }
          pre { (text) }
        },
    )
}

pub(super) fn render(
    projects: &[Project],
    project: Option<&Project>,
    fossils: &[Fossil],
    fossil: Option<&Fossil>,
    records: &[Record],
    artifacts: &[(&ConfigurationKey, String)],
) -> Markup {
    document(
        "Fossil",
        "",
        html! {
          header {
            a href="/" { "fossil" }
            span { "records & artifacts" }
            span #busy role="status" hidden { span.spinner {} "Working…" }
          }
          main {
            aside {
              h2 { "Projects" }
              @for p in projects {
                (link(&p.config.name, Selection {
                  project: Some(p.config.name.clone()), ..Default::default()
                }, false))
              }
              @if projects.is_empty() { p { "No projects yet." } }
              h2 { "Fossils" }
              @for f in fossils {
                (link(&f.config.name, Selection {
                  project: project.map(|p| p.config.name.clone()),
                  fossil: Some(f.config.name.clone()), ..Default::default()
                }, false))
              }
              @if fossils.is_empty() { p { "No fossils yet." } }
            }
            section {
              @if let (Some(p), Some(f)) = (project, fossil) {
                (detail(p, f, records, artifacts))
              }
            }
          }
        },
    )
}

fn variant_color(name: &str) -> u32 {
    name.bytes().fold(0u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte.into())
    }) % 8
}

fn detail(
    project: &Project,
    fossil: &Fossil,
    records: &[Record],
    artifacts: &[(&ConfigurationKey, String)],
) -> Markup {
    let selection = || Selection {
        project: Some(project.config.name.clone()),
        fossil: Some(fossil.config.name.clone()),
        ..Default::default()
    };
    html! {
      div.workspace {
        div.records-panel #records-panel {
          p.muted { (&project.config.name) }
          h1 { (&fossil.config.name) }
          p { (fossil.config.desc()) }
          h2 { "Records" }
          form #analysis data-project=(&project.config.name) data-fossil=(&fossil.config.name) {
            div.controls {
              input #search type="search" aria-label="Search records" placeholder="Search records…";
              select #variant aria-label="Filter by variant" {
                option value="" { "All variants" }
                @for variant in records.iter().map(|r| r.manifest.variant.as_str()).collect::<std::collections::BTreeSet<_>>() {
                  option value=(variant) { (variant) }
                }
              }
              select name="analysis" aria-label="Analysis" required {
                option value="" { "Choose analysis…" }
                @for name in fossil.config.analyses.keys() {
                  option value=(name.as_str()) { (name.as_str()) }
                }
              }
              button #run type="submit" disabled { "Run analysis" }
              span #selection-count aria-live="polite" { "0 selected" }
              button #clear-selection type="button" { "Deselect all" }
            }
            @if fossil.config.analyses.is_empty() { p.muted { "No analyses configured." } }
          }
          p.muted { "Click column headings to sort. Filtering preserves selected records." }
          @if records.is_empty() {
            p { "No records yet." }
          } @else {
            div.scroll {
              table {
                thead {
                  tr {
                    th { input #select-visible type="checkbox" aria-label="Select all visible records"; }
                    @for (column, label) in [("recorded", "Recorded"), ("variant", "Variant"), ("commit", "Commit"), ("iterations", "Iterations")] {
                      th aria-sort=(if column == "recorded" { "descending" } else { "none" }) {
                        button.sort type="button" data-sort=(column) { (label) }
                      }
                    }
                  }
                }
                tbody #records {
                  @for r in records {
                    @let m = &r.manifest;
                    tr data-recorded=(m.timestamp) data-variant=(m.variant.as_str()) data-commit=(&m.git.commit) data-iterations=(m.iterations) {
                      td { input type="checkbox" name="records" form="analysis" value=(r.id()) aria-label=(format!("Select {}", r.id())); }
                      td { (link(&m.timestamp.to_string(), Selection {
                        record: Some(r.id()), ..selection()
                      }, true)) }
                      td { span.tag data-color=(variant_color(m.variant.as_str())) { (m.variant.as_str()) } }
                      td { code { (&m.git.commit) } }
                      td { (m.iterations) }
                    }
                  }
                }
              }
            }
          }
        }
        div #divider role="separator" tabindex="0" aria-label="Resize records and viewer"
          aria-orientation="vertical" aria-controls="records-panel" aria-valuemin="20" aria-valuemax="80" aria-valuenow="60" {}
        div.viewer-panel {
          nav.tabs role="tablist" aria-label="Viewer" {
            @for (id, label) in [("output", "Output"), ("artifacts", "Artifacts")] {
              button id=(format!("{id}-tab")) type="button" role="tab"
                aria-controls=(format!("{id}-panel")) aria-selected=(if id == "output" { "true" } else { "false" })
                tabindex=(if id == "output" { "0" } else { "-1" }) { (label) }
            }
          }
          div #output-panel role="tabpanel" aria-labelledby="output-tab" {
            p.muted { "View a record or run an analysis on selected records." }
            pre #analysis-output hidden role="status" {}
            iframe name="output" title="Record or analysis output" {}
          }
          div #artifacts-panel role="tabpanel" aria-labelledby="artifacts-tab" hidden {
            nav.artifacts {
              @for (name, file) in artifacts {
                (link(&format!("{name}/{file}"), Selection {
                  artifact: Some((*name).clone()), file: Some(file.clone()), ..selection()
                }, true))
              }
              @if artifacts.is_empty() { p { "No generated artifacts yet." } }
            }
            @if !artifacts.is_empty() {
              iframe name="artifacts" title="Artifact preview" {}
            }
          }
        }
      }
    }
}
