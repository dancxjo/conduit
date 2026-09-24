//! Human-facing documentary over verified journey evidence.
use super::{BodyTrack, ThreeBodyJourneyIndex, TrackStep};
use std::fmt::Write;

pub(super) fn render(index: &ThreeBodyJourneyIndex) -> String {
    let mut html = String::from("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"dark\"><title>One journey. Three bodies. — Conduit</title><style>");
    html.push_str(include_str!("evidence_three_body_journey.css"));
    html.push_str("</style></head><body><nav class=\"topbar\"><a href=\"../../\">Conduit / Journeys</a><a href=\"#by-step\">Explore the journey</a></nav><header><p class=\"eyebrow\">A life in three kinds of machinery</p><h1>One journey.<br><em>Three bodies.</em></h1><p class=\"lede\">A computer boots. A browser opens. A model finds its voice. Watch them wake, do useful work, welcome another host, recover from failure, and come to rest.</p><a class=\"button\" href=\"#by-step\">Begin the journey ↓</a></header><main id=\"by-step\"><p class=\"eyebrow\">The same moment, three ways</p><h2>Watch the meaning carry through.</h2><p>Compare the screens and words below. Select a body to follow its life from beginning to end.</p><div class=\"view-controls\" role=\"group\" aria-label=\"Journey view\"><button data-view=\"all\" aria-pressed=\"true\">Compare all three</button>");
    for track in &index.tracks {
        let _ = write!(
            html,
            "<button data-view=\"{}\" aria-pressed=\"false\">{}</button>",
            escape(&track.track_id),
            label(track)
        );
    }
    html.push_str("</div><nav class=\"chapter-nav\" aria-label=\"Journey moments\">");
    for (i, step) in index.semantic_steps.iter().enumerate() {
        let _ = write!(
            html,
            "<a href=\"#moment-{i}\">{:02} {}</a>",
            i + 1,
            escape(&step.title)
        );
    }
    html.push_str("</nav>");
    for (i, contract) in index.semantic_steps.iter().enumerate() {
        let _ = write!(html, "<article class=\"chapter\" id=\"moment-{i}\"><div class=\"chapter-heading\"><span class=\"number\">{:02}</span><div><h2>{}</h2><p>{}</p></div></div><div class=\"comparison\">", i+1, escape(&contract.title), escape(&contract.what_happened));
        for track in &index.tracks {
            let observed = &track.steps[i];
            let _ = write!(
                html,
                "<section class=\"body-panel\" data-body=\"{}\"><h3>{}</h3>",
                escape(&track.track_id),
                label(track)
            );
            if let Some(recording) = index
                .recorded_generative
                .as_ref()
                .filter(|_| track.track_id == "hosted-generative")
            {
                html.push_str(
                    "<p class=\"proof-mode\">Recorded live Gemma · voiced with Piper</p>",
                );
                render_media(
                    &mut html,
                    recording,
                    &recording.steps[i],
                    "live-conformance",
                );
                render_evidence(
                    &mut html,
                    recording,
                    &recording.steps[i],
                    "live-conformance",
                );
                html.push_str("<details><summary>Current release contract check</summary>");
                render_evidence(&mut html, track, observed, &track.track_id);
                html.push_str("</details>");
            } else {
                render_media(&mut html, track, observed, &track.track_id);
                render_evidence(&mut html, track, observed, &track.track_id);
            }
            html.push_str("<details><summary>What this does—and does not—prove</summary><ul>");
            for claim in &contract.non_claims {
                let _ = write!(html, "<li>{}</li>", escape(claim));
            }
            html.push_str("</ul></details>");
            html.push_str("</section>");
        }
        html.push_str("</div></article>");
    }
    let _ = write!(html, "</main><footer><h2>The machinery changes.<br>The meaning carries through.</h2><p>Three independent bodies, each with its own history. Native captures show ConduitOS in QEMU; browser captures show Chromium.</p><details><summary>Inspect the complete evidence</summary><p>Journey {} · source {}</p><a href=\"index.json\">Verified journey index</a></details><a href=\"../../\">Back to the journeys</a></footer>", escape(&index.journey_id), escape(&index.git_commit));
    html.push_str("<script>");
    html.push_str(include_str!("evidence_three_body_journey.js"));
    html.push_str("</script></body></html>");
    html
}

fn label(track: &BodyTrack) -> &'static str {
    match track.track_id.as_str() {
        "native-graphical" => "ConduitOS",
        "browser-graphical" => "Browser",
        _ => "Conversational",
    }
}

fn render_media(html: &mut String, track: &BodyTrack, observed: &TrackStep, root: &str) {
    if track.embodiment.contains("fixture") {
        html.push_str(
            "<p class=\"proof-mode\">Release contract recording · deterministic model fixture</p>",
        );
    }
    if track.track_id == "hosted-generative"
        && matches!(
            observed.step_id.as_str(),
            "body.absent" | "bootstrap.started"
        )
    {
        html.push_str(
            "<blockquote>No body has been born yet. There is no body voice to play.</blockquote>",
        );
    }
    let mut media: Vec<_> = observed.evidence.iter().collect();
    media.sort_by_key(|item| match item.evidence_class.as_str() {
        "screenshot" => 0,
        "waveform" => 1,
        "audio" => 2,
        "transcript" => 3,
        _ => 4,
    });
    for evidence in media {
        let url = escape(&format!("{root}/{}", evidence.path.display()));
        let caption = escape(&evidence.documentary_description);
        match evidence.evidence_class.as_str() {
            "screenshot" => {
                let _ = write!(html, "<figure><a href=\"{url}\" target=\"_blank\" rel=\"noopener\" aria-label=\"Open full-size screenshot\"><img src=\"{url}\" alt=\"{caption}\" loading=\"lazy\"></a><figcaption>{caption}</figcaption></figure>");
            }
            "audio" => {
                let _ = write!(html, "<figure><audio controls preload=\"none\" src=\"{url}\"></audio><figcaption>{caption}</figcaption></figure>");
            }
            "waveform" => {
                let _ = write!(
                    html,
                    "<img class=\"waveform\" src=\"{url}\" alt=\"{caption}\" loading=\"lazy\">"
                );
            }
            "video" => {
                let _ = write!(html, "<figure><video controls preload=\"metadata\" src=\"{url}\"></video><figcaption>{caption}</figcaption></figure>");
            }
            "transcript" => {
                let _ = write!(html, "<blockquote data-transcript=\"{url}\"><a href=\"{url}\">Read the recorded words</a></blockquote><p class=\"caption\">{caption}</p>");
            }
            _ => {}
        }
    }
}

fn render_evidence(html: &mut String, track: &BodyTrack, observed: &TrackStep, root: &str) {
    html.push_str("<details class=\"evidence\"><summary>Evidence</summary><ul>");
    let _ = write!(
        html,
        "<li>Recorded source: {} · Presenter: {}</li>",
        escape(&track.git_commit),
        escape(&track.presenter_id)
    );
    for evidence in &observed.evidence {
        let _ = write!(
            html,
            "<li><a href=\"{}/{}\">{}</a><br><small>{} · {}</small></li>",
            escape(root),
            escape(&evidence.path.to_string_lossy()),
            escape(&evidence.documentary_description),
            escape(&evidence.sha256),
            evidence.assertion_rung.label()
        );
    }
    let provenance = serde_json::to_string_pretty(&observed.provenance).unwrap_or_default();
    let _ = write!(html, "</ul><pre>{}</pre></details>", escape(&provenance));
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
