//! Project goals and explicit documentary availability.

use std::{fs, path::Path};

use serde::Serialize;

use super::{escape_html, write_html};

const SCHEMA: &str = "conduit.vertical-journey-catalogue/v1";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum State {
    Accepted,
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
    Vertical { slug: "pocket-theremin", title: "Pocket Theremin", issue: 4091, state: State::Accepted, summary: "A portable sound Body is controlled through exact quantity and safety bounds.", accepted_source: Some("319ab3121979da2e7eefdb0c622e74e9baa29386") },
    Vertical { slug: "two-ollamas", title: "Two Ollamas", issue: 4092, state: State::Accepted, summary: "An attended conversation crosses two independently realized model Bodies.", accepted_source: Some("24a33926c582aa6035aca88e9a1252083ce2dbce") },
    Vertical { slug: "three-bodies", title: "Three Bodies", issue: 4093, state: State::Accepted, summary: "One semantic journey is lived independently through native graphical, browser graphical, and screen-free Bodies.", accepted_source: Some("8c6f4a8bea743b733fa9de46aaeb6c6f119e1faa") },
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
        let proof = vertical.accepted_source.map_or_else(
            || "No historical source is recorded.".to_owned(),
            |source| format!("Historical accepted source: <a href=\"https://github.com/dancxjo/conduit/commit/{source}\"><code>{source}</code></a>. This source link is not a captured walkthrough."),
        );
        let goal = goal(vertical.slug);
        let capture = if vertical.slug == "three-bodies"
            && root.join("current/three-bodies/index.html").is_file()
        {
            "<p><a href=\"../../current/three-bodies/\">Follow the retained Three Bodies recording</a>. Its own source and fixture limitations apply.</p>"
        } else {
            "<p>No captured step-by-step walkthrough is included on this catalogue page. Use the source and issue below to inspect the implementation record.</p>"
        };
        let body = format!(
            "<nav><a href=\"../\">All goals</a> · <a href=\"../../\">Journeys</a></nav><p class=\"eyebrow\">Project documentation</p><h1>{}</h1><p>{}</p><h2>Available evidence</h2>{capture}<details><summary>Implementation history</summary><p>{proof}</p><p>Catalogue source: <code>{}</code></p><p><a href=\"https://github.com/dancxjo/conduit/issues/{}\">Issue #{}</a></p></details>",
            escape_html(vertical.title), escape_html(goal), escape_html(commit), vertical.issue, vertical.issue,
        );
        let directory = vertical_root.join(vertical.slug);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("cannot create vertical page: {error}"))?;
        write_html(&directory.join("index.html"), vertical.title, &body)?;
        cards.push_str(&format!("<article class=\"journey-card\"><p class=\"eyebrow\">Project documentation</p><h2>{}</h2><p>{}</p><p><a href=\"{}/\">Read the goal and available evidence</a></p></article>", escape_html(vertical.title), escape_html(goal), vertical.slug));
    }
    write_html(&vertical_root.join("index.html"), "Goals and evidence — Conduit", &format!("<nav><a href=\"../\">Journeys</a></nav><h1>What would you like to do?</h1><p>These projects cover different user goals. A completed issue or source link does not imply that a captured walkthrough is available. Each page identifies what you can actually inspect.</p><section class=\"cards\">{cards}</section>"))?;
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

fn goal(slug: &str) -> &'static str {
    match slug {
        "field-station-clock" => "Read a browser clock, reload it while preserving its identity, then put it to rest.",
        "durable-notebook" => "Keep authored notes across a restart and inspect what was retained.",
        "bare-metal-to-show" => "Start ConduitOS and follow it from boot to a usable presentation.",
        "pocket-theremin" => "Control pitch and volume through a portable sound interface with bounded output.",
        "two-ollamas" => "Follow an attended conversation between two separately running model participants.",
        "three-bodies" => "Start, use, inspect, and stop a body; compare the recorded actions across environments.",
        _ => "Inspect the project goal and its available evidence.",
    }
}
