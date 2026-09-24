use maud::{DOCTYPE, Markup, html};

use super::Selection;
use crate::artifact::ArtifactFormat;
use crate::fossil::Fossil;
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
            }
            body class=(class) { (content) }
        }
    }
}

fn link(label: &str, selection: Selection, output: bool) -> Markup {
    let query = serde_urlencoded::to_string(selection).unwrap();
    let path = if output { "/output" } else { "/" };
    html! {
        a href=(format!("{path}?{query}")) target=[output.then_some("output")] {
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
    artifacts: &[(&str, ArtifactFormat)],
) -> Markup {
    document(
        "Fossil",
        "",
        html! {
            header {
                a href="/" { "fossil" }
                span { "records & artifacts" }
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

fn detail(
    project: &Project,
    fossil: &Fossil,
    records: &[Record],
    artifacts: &[(&str, ArtifactFormat)],
) -> Markup {
    let selection = || Selection {
        project: Some(project.config.name.clone()),
        fossil: Some(fossil.config.name.clone()),
        ..Default::default()
    };
    html! {
        p.muted { (&project.config.name) }
        h1 { (&fossil.config.name) }
        p { (fossil.config.desc()) }
        h2 { "Records" }
        @if records.is_empty() {
            p { "No records yet." }
        } @else {
            div.scroll {
                table {
                    thead {
                        tr {
                            th { "Variant" }
                            th { "Recorded" }
                            th { "Commit" }
                            th { "Iterations" }
                        }
                    }
                    tbody {
                        @for r in records {
                            @let m = &r.manifest;
                            tr {
                                td { (link(m.variant.as_str(), Selection {
                                    record: Some(r.id()), ..selection()
                                }, true)) }
                                td { (m.timestamp) }
                                td { code { (&m.git.commit) } }
                                td { (m.iterations) }
                            }
                        }
                    }
                }
            }
        }
        h2 { "Artifacts" }
        nav {
            @for (name, format) in artifacts {
                (link(&format!("{name}.{}", format.extension()), Selection {
                    artifact: Some((*name).into()), ..selection()
                }, true))
            }
            @if artifacts.is_empty() { p { "No generated artifacts yet." } }
        }
        h2 { "Output" }
        p.muted { "Select a record or artifact to view it below." }
        iframe name="output" title="Record or artifact output" {}
    }
}
