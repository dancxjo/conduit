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
        "<p><a href=\"current/three-bodies/\">View the retained Three Bodies recording</a>. It compares separate Bodies from an older source; its screenshots and speech evidence do not prove today's shared-Body experience.</p>"
    } else {
        "<p>A verified Three Bodies walkthrough has not been included for this publication. No substitute recording is shown.</p>"
    };
    let one_body_development = fs::read(root.join("current/one-body-five-masks/manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|manifest| manifest["result"] == "diagnostic-incomplete");
    let one_body = if root
        .join("current/one-body-five-masks/index.html")
        .is_file()
    {
        if one_body_development {
            "<article class=\"journey-card\"><p class=\"eyebrow\">One Body · development recording</p><h2>Keep one clock with you across three hosts</h2><p>Follow the retained browser, ConduitOS, terminal, and speech actions that have been captured so far. This is a partial local proof; the complete eight-chapter journey is still in progress.</p><a href=\"current/one-body-five-masks/\">Inspect the current recording</a></article>"
        } else {
            "<article class=\"journey-card\"><p class=\"eyebrow\">One Body · five Masks</p><h2>Keep one clock with you across three hosts</h2><p>Follow eight real actions through browser, ConduitOS, terminal, and speech, then inspect what happens when a host or model goes away.</p><a href=\"current/one-body-five-masks/\">Follow the captured journey</a></article>"
        }
    } else {
        ""
    };
    let todo = if root.join("current/todo/index.html").is_file() {
        "<article class=\"journey-card\"><p class=\"eyebrow\">One Todo Body · captured journey</p><h2>Keep a list across Masks</h2><p>Add and complete items through different interfaces, hear what remains, and return to the same Body. Each action and capture has a retained receipt.</p><a href=\"current/todo/\">Follow the Todo journey</a></article>"
    } else {
        ""
    };
    let body = format!("<header><p class=\"eyebrow\">Journeys</p><h1>Choose what you want to do</h1><p class=\"lede\">Follow a task in order, see the recorded result, and inspect the evidence when you need it. Each page says whether it includes a captured walkthrough or only project documentation.</p></header><main><section aria-labelledby=\"tasks-title\"><h2 id=\"tasks-title\">Start with a task</h2><div class=\"cards\">{todo}{one_body}<article class=\"journey-card\"><p class=\"eyebrow\">Field Station Clock</p><h2>Read a clock, reload, then rest</h2><p>Follow a small browser clock through its first view, a reload, and an explicit lull. The linked page identifies which steps have captures available.</p>{clock}</article>{handbook}<!-- conduit-three-body-flagship@2 --><article class=\"journey-card\"><p class=\"eyebrow\">Retained Three Bodies recording</p><h2>Compare three separate Bodies</h2><p>This historical recording follows independent Bodies in different environments. Its captures have their own source and limitations.</p>{three_bodies}</article><!-- conduit-three-body-flagship:end --><!-- conduit-conduitos-journey-card@1 --></div></section><section><h2>Explore other goals</h2><p>Keep notes across a restart, control sound, or inspect model conversations. These projects have different evidence coverage.</p><a href=\"verticals/\">Browse goals and capture availability</a></section><section><h2>Technical examples</h2><p>These focused examples explain runtime or rendering behavior. They are separate from the task walkthroughs above.</p><div class=\"cards\">{examples}</div></section><details class=\"history\"><summary>Publication source and retained history</summary><p>Retained gallery source: <code>{}</code></p><ul>{history}</ul><p>The latest {RETAINED_COMMITS} published source commits are retained. Each archive keeps its own evidence identities and limitations.</p><a href=\"gallery.json\">Gallery metadata</a></details></main>", escape_html(&index.current_commit));
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
