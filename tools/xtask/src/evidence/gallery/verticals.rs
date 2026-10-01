//! Durable, bounded catalogue of completed and upcoming product verticals.

use std::{fs, path::Path};

use serde::Serialize;

use super::{escape_html, write_html};

const SCHEMA: &str = "conduit.vertical-journey-catalogue/v1";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum State {
    Accepted,
    Planned,
}

#[derive(Serialize)]
struct Vertical<'a> {
    slug: &'a str,
    title: &'a str,
    issue: u16,
    state: State,
    summary: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    accepted_source: Option<&'a str>,
}

const VERTICALS: &[Vertical<'static>] = &[
    Vertical { slug: "field-station-clock", title: "Field Station Clock", issue: 4090, state: State::Accepted, summary: "One tiny browser Body keeps its identity through reload and returns to a real lull.", accepted_source: Some("08c5ad83e8bebe9dc195c5d77772ffb8ee67aa30") },
    Vertical { slug: "durable-notebook", title: "Durable Notebook", issue: 4116, state: State::Accepted, summary: "A notebook Body preserves authored meaning durably across a fresh Boot.", accepted_source: Some("75bc7d7b535b8c7df8ad17a5fc27b33c3c3812a0") },
    Vertical { slug: "bare-metal-to-show", title: "Bare Metal to Show", issue: 4117, state: State::Accepted, summary: "A real ConduitOS Body travels from freestanding boot to its admitted Show.", accepted_source: Some("36c9be63f0d3dd6e7a8bfc09e7afe1a06d6d96cd") },
    Vertical { slug: "pocket-theremin", title: "Pocket Theremin", issue: 4091, state: State::Planned, summary: "An upcoming portable sound Body controlled through exact quantity and safety bounds.", accepted_source: None },
    Vertical { slug: "two-ollamas", title: "Two Ollamas", issue: 4092, state: State::Planned, summary: "An upcoming attended conversation across two independently realized model Bodies.", accepted_source: None },
    Vertical { slug: "three-bodies", title: "Three Bodies", issue: 4093, state: State::Planned, summary: "An upcoming same-meaning journey through native graphical, browser graphical, and screen-free Bodies.", accepted_source: None },
];

#[derive(Serialize)]
struct Catalogue<'a> {
    schema: &'static str,
    publication_commit: &'a str,
    verticals: &'static [Vertical<'static>],
}

pub(super) fn write_vertical_catalogue(root: &Path, commit: &str) -> Result<(), String> {
    let vertical_root = root.join("verticals");
    fs::create_dir_all(&vertical_root)
        .map_err(|error| format!("cannot create vertical catalogue: {error}"))?;
    let mut cards = String::new();
    for vertical in VERTICALS {
        let state = match vertical.state {
            State::Accepted => "Accepted",
            State::Planned => "Planned",
        };
        let proof = vertical.accepted_source.map_or_else(
            || "No accepted evidence is claimed yet.".to_owned(),
            |source| format!("Accepted source: <a href=\"https://github.com/dancxjo/conduit/commit/{source}\"><code>{source}</code></a>."),
        );
        let body = format!(
            "<nav><a href=\"../\">All verticals</a> · <a href=\"../../\">Journeys home</a></nav><p class=\"eyebrow\">{state} vertical</p><h1>{}</h1><p>{}</p><p>{proof}</p><p>Catalogue projection: <code>{}</code></p><p><a href=\"https://github.com/dancxjo/conduit/issues/{}\">Issue #{}</a></p>",
            escape_html(vertical.title), escape_html(vertical.summary), escape_html(commit), vertical.issue, vertical.issue,
        );
        let directory = vertical_root.join(vertical.slug);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("cannot create vertical page: {error}"))?;
        write_html(&directory.join("index.html"), vertical.title, &body)?;
        cards.push_str(&format!("<article class=\"journey-card\"><p class=\"eyebrow\">{state}</p><h2>{}</h2><p>{}</p><p><a href=\"{}/\">Open journey</a> · <a href=\"https://github.com/dancxjo/conduit/issues/{}\">Issue #{}</a></p></article>", escape_html(vertical.title), escape_html(vertical.summary), vertical.slug, vertical.issue, vertical.issue));
    }
    write_html(&vertical_root.join("index.html"), "Conduit verticals", &format!("<nav><a href=\"../\">Journeys home</a></nav><h1>Every vertical has a Journey</h1><p>Accepted work links to exact source identity. Upcoming work is plainly marked planned and carries no invented evidence.</p><section class=\"cards\">{cards}</section>"))?;
    let document = Catalogue {
        schema: SCHEMA,
        publication_commit: commit,
        verticals: VERTICALS,
    };
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("cannot encode vertical catalogue: {error}"))?;
    fs::write(root.join("catalogue.json"), bytes)
        .map_err(|error| format!("cannot write vertical catalogue: {error}"))
}
