//! Task-oriented entrance and shared framing for retained evidence pages.
use super::{escape_html, GalleryIndex, RETAINED_COMMITS};
use std::{fs, path::Path};

pub(super) fn write_root_index(
    root: &Path,
    index: &GalleryIndex,
    _has_conduitos: bool,
) -> Result<(), String> {
    let mut examples = String::new();
    for (path, title, description) in [
        ("current/one-plot-two-fronts", "Compare two renderers", "Inspect the same presentation in Chromium and a native software renderer. This is a rendering comparison, not a complete user workflow."),
        ("current/little-life", "Little Life simulation", "Inspect four moments of a deterministic scalar-field simulation. This technical example is not a guide to using a Conduit application."),
        ("current/conduitos/x86_64", "Inspect a ConduitOS emulator run", "Read the retained x86_64 QEMU run and its evidence. Emulator execution does not establish physical hardware operation."),
        ("current/patchbay", "Inspect Patchbay scenarios", "Browse individual captured interface scenarios and their exact receipts."),
    ] {
        if root.join(path).join("index.html").is_file() {
            examples.push_str(&format!("<article class=\"journey-card\"><h3>{title}</h3><p>{description}</p><a href=\"{path}/\">View technical example</a></article>"));
        }
    }
    if examples.is_empty() {
        examples.push_str("<p>No technical captures are included in this publication.</p>");
    }
    let mut history = String::new();
    for commit in &index.commits {
        let mut links = String::new();
        for (path, title) in [
            ("patchbay", "Patchbay"),
            ("conduitos/x86_64", "ConduitOS"),
            ("one-plot-two-fronts", "Two Fronts"),
            ("little-life", "Little Life"),
        ] {
            if root
                .join("commits")
                .join(commit)
                .join(path)
                .join("index.html")
                .is_file()
            {
                links.push_str(&format!(
                    " · <a href=\"commits/{commit}/{path}/\">{title}</a>"
                ));
            }
        }
        history.push_str(&format!(
            "<li><code>{}</code>{links}</li>",
            escape_html(commit)
        ));
    }
    let clock = if root
        .join("verticals/field-station-clock/index.html")
        .is_file()
    {
        "<a href=\"verticals/field-station-clock/\">View Field Station Clock and available evidence</a>"
    } else {
        "<p>Field Station Clock documentation is not included in this publication.</p>"
    };
    let handbook = if root.join("verticals/handbook/index.html").is_file() {
        "<article class=\"journey-card\"><p class=\"eyebrow\">Your local Handbook</p><h2>Try a clock, look inside, and keep your edit</h2><p>Follow actual browser actions from opening your own Body to checking source, inspecting resident Patchbay, changing the clock, and recovering it after reload.</p><a href=\"verticals/handbook/\">Follow the Handbook walkthrough</a></article>"
    } else {
        ""
    };
    let three_bodies = if root.join("current/three-bodies/index.html").is_file() {
        "<p><a href=\"current/three-bodies/\">Follow the retained Three Bodies recording</a>. Read its source and limitations before comparing tracks.</p>"
    } else {
        "<p>A verified Three Bodies walkthrough has not been included for this publication. No substitute recording is shown.</p>"
    };
    let body = format!("<header><p class=\"eyebrow\">Journeys</p><h1>Choose what you want to do</h1><p class=\"lede\">Follow a task in order, see the recorded result, and inspect the evidence when you need it. Each page says whether it includes a captured walkthrough or only project documentation.</p></header><main><section aria-labelledby=\"tasks-title\"><h2 id=\"tasks-title\">Start with a task</h2><div class=\"cards\"><article class=\"journey-card\"><p class=\"eyebrow\">Field Station Clock</p><h2>Read a clock, reload, then rest</h2><p>Follow a small browser clock through its first view, a reload, and an explicit lull. The linked page identifies which steps have captures available.</p>{clock}</article>{handbook}<!-- conduit-three-body-flagship@2 --><article class=\"journey-card\"><p class=\"eyebrow\">Three Bodies</p><h2>Start, use, inspect, and stop a body</h2><p>Follow a browser session and compare its actions with other environments.</p>{three_bodies}</article><!-- conduit-three-body-flagship:end --></div></section><section><h2>Explore other goals</h2><p>Keep notes across a restart, control sound, or inspect model conversations. These projects have different evidence coverage.</p><a href=\"verticals/\">Browse goals and capture availability</a></section><section><h2>Technical examples</h2><p>These focused examples explain runtime or rendering behavior. They are separate from the task walkthroughs above.</p><div class=\"cards\">{examples}</div><!-- conduit-conduitos-journey-card@1 --></section><details class=\"history\"><summary>Publication source and retained history</summary><p>Retained gallery source: <code>{}</code></p><ul>{history}</ul><p>The latest {RETAINED_COMMITS} published source commits are retained. Each archive keeps its own evidence identities and limitations.</p><a href=\"gallery.json\">Gallery metadata</a></details></main>", escape_html(&index.current_commit));
    write_html(&root.join("index.html"), "Journeys — Conduit", &body)
}

pub(super) fn write_html(path: &Path, title: &str, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create gallery page directory: {error}"))?;
    }
    let document = format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>{}\n{}</style></head><body data-application-theme=\"conduit.presentation/phosphor@1\">{}<div class=\"gallery-content\">{body}</div></body></html>", escape_html(title), crate::site::styles(), include_str!("landing.css"), crate::site::navigation("journeys"));
    fs::write(path, document).map_err(|error| format!("cannot write gallery page: {error}"))
}
