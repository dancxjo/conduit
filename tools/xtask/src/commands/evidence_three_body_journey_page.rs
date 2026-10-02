//! Human-facing documentary over verified journey evidence.
use super::{ConstructionStage, JourneyBodyCell, ThreeBodyJourneyIndex, TrackStep};
use std::fmt::Write;

pub(super) fn render(index: &ThreeBodyJourneyIndex) -> String {
    let mut html = String::from("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"dark\"><title>Start, change, and stop a body — Conduit</title><style>");
    html.push_str(&crate::site::styles());
    html.push_str(include_str!("evidence_three_body_journey.css"));
    html.push_str(
        "</style></head><body data-application-theme=\"conduit.presentation/phosphor@1\">",
    );
    html.push_str(&crate::site::navigation("journeys"));
    html.push_str("<header><p class=\"eyebrow\">Three Bodies · recorded walkthrough</p><h1>Start a body.<br>Change its interface.<br>Bring it to rest.</h1><p class=\"lede\">Follow a browser session from its first start to its end. Along the way, inspect its work, change how it presents itself, and see what happens when an interface becomes unavailable.</p><section class=\"setup\" aria-labelledby=\"setup-title\"><h2 id=\"setup-title\">Before you follow along</h2><p>This is a recorded walkthrough, not an interactive session. Read the steps in order; each screen or recording below comes from the linked evidence. No installation is needed to view it.</p><p>A body is the running participant whose work you are following. A host supplies ways to run that work. A mask determines how the body presents its interface. The browser track is shown first; you can also inspect the ConduitOS and conversational tracks.</p><p>ConduitOS captures run in QEMU and browser captures run in Chromium. A deterministic conversational fixture is labeled wherever it appears. Missing screenshots or audio are called out at the relevant step.</p></section><a class=\"button\" href=\"#by-step\">Follow the recorded session ↓</a></header><main id=\"by-step\"><p class=\"eyebrow\">Step by step</p><h2>Follow one session</h2><p>Start with Browser. The optional comparison aligns the same actions across independent bodies; it does not imply identical screens, words, or timing.</p><div class=\"view-controls\" role=\"group\" aria-label=\"Journey view\"><button data-view=\"all\" aria-pressed=\"false\">The same moment, three ways</button>");
    let bodies = index
        .actions
        .first()
        .map(|action| action.bodies.as_slice())
        .unwrap_or_default();
    for body in bodies {
        let _ = write!(
            html,
            "<button data-view=\"{}\" aria-pressed=\"{}\">{}</button>",
            escape(&body.track_id),
            body.track_id == "browser-graphical",
            label(body)
        );
    }
    let _ = write!(html, "</div><p class=\"recording-source\">Recorded source: <code>{}</code>. The journey index does not supply a recording date; these captures are not a live session.</p><nav class=\"chapter-nav\" aria-label=\"Journey chapters\">", escape(&index.git_commit));
    let chapters = [
        (
            "open",
            "Open",
            "Begin before a body exists. Inspect the host starting the session.",
        ),
        (
            "start",
            "Start",
            "Follow the creation of a body and its first usable wake.",
        ),
        (
            "use",
            "Use",
            "See the work this body performed. Optionally inspect how its interface was changed.",
        ),
        (
            "recover",
            "Recover",
            "Inspect the recorded failure. Look for separate evidence of recovery, if any.",
        ),
        (
            "finish",
            "Finish",
            "Bring the wake to rest, then inspect the separate end of the body's lifecycle.",
        ),
    ];
    for (i, (id, title, _)) in chapters.iter().enumerate() {
        let _ = write!(html, "<a href=\"#chapter-{id}\">{:02} {title}</a>", i + 1);
    }
    html.push_str("</nav>");
    for (chapter_index, (chapter_id, title, description)) in chapters.iter().enumerate() {
        let _ = write!(html, "<article class=\"chapter\" id=\"chapter-{chapter_id}\"><div class=\"chapter-heading\"><span class=\"number\">{:02}</span><div><h2>{title}</h2><p>{description}</p></div></div>", chapter_index + 1);
        let mut technical_open = false;
        for (i, action) in index
            .actions
            .iter()
            .enumerate()
            .filter(|(_, action)| chapter(&action.action.action_id) == *chapter_id)
        {
            let contract = &action.action;
            if contract.action_id.starts_with("mask.") && !technical_open {
                html.push_str("<details class=\"interface-details\"><summary>Inspect and change the interface: detailed recorded steps</summary><p>These optional steps retain the exact order of the interface experiment, including unavailability and replacement planning.</p>");
                technical_open = true;
            }
            let _ = write!(html, "<section class=\"recorded-action\" id=\"moment-{i}\"><h3>{}</h3><p>{}</p><div class=\"comparison single\">", escape(action_description(&contract.action_id).0), escape(action_description(&contract.action_id).1));
            for body in &action.bodies {
                let observed = &body.observed;
                let _ = write!(
                    html,
                    "<section class=\"body-panel\" data-body=\"{}\"{}><h4>{}</h4>",
                    escape(&body.track_id),
                    if body.track_id == "browser-graphical" {
                        ""
                    } else {
                        " hidden"
                    },
                    label(body)
                );
                let _ = write!(
                    html,
                    "<p class=\"concrete-event\"><strong>Recorded result.</strong> {}</p>",
                    escape(&observed.concrete_event)
                );
                render_receipts(
                    &mut html,
                    body,
                    &body.receipts,
                    &body.track_id,
                    &index.git_commit,
                    &body.mask_plot_id,
                );
                html.push_str("<details><summary>Contract and limits of this step</summary>");
                let _ = write!(
                    html,
                    "<p>{}</p><p>{}</p><ul>",
                    escape(&contract.what_happened),
                    escape(&contract.what_conduit_established)
                );
                for claim in &contract.non_claims {
                    let _ = write!(html, "<li>{}</li>", escape(claim));
                }
                html.push_str("</ul></details>");
                html.push_str("</section>");
            }
            html.push_str("</div></section>");
        }
        if technical_open {
            html.push_str("</details>");
        }
        html.push_str("</article>");
    }
    let _ = write!(html, "</main><footer><h2>Review what happened</h2><p>Each track records an independent body. Screens, recordings, and receipts support the individual steps above; they do not establish physical hardware operation or a live model conversation unless the evidence explicitly says so.</p><details><summary>Inspect the complete evidence</summary><p>Journey {} · source {}</p><a href=\"index.json\">Verified journey index</a></details><a href=\"../../\">Back to journeys</a></footer>", escape(&index.journey_id), escape(&index.git_commit));
    html.push_str("<script>");
    html.push_str(include_str!("evidence_three_body_journey.js"));
    html.push_str("</script></body></html>");
    html
}

fn chapter(action: &str) -> &'static str {
    match action {
        "journey.bootstrap" => "open",
        "journey.birth" => "start",
        "journey.break-recover" => "recover",
        "journey.rest-finish" => "finish",
        _ => "use",
    }
}

// The contract fixes the action order; these descriptions explain its actions
// without inventing clicks, controls, or outcomes absent from producer receipts.
fn action_description(id: &str) -> (&'static str, &'static str) {
    match id {
        "journey.bootstrap" => ("Start the host", "Begin with no body running. Inspect the recorded start before looking for a usable interface."),
        "journey.birth" => ("Create and wake the body", "Follow creation through its first wake. Check the recorded result to see when this body became usable."),
        "journey.useful-work" => ("Use the work already available", "Run the work the body has already admitted. The result below identifies what this particular track performed."),
        "mask.inspect-initial-show" => ("Inspect the current interface", "Look at the body's first presentation and the recorded route that makes it available."),
        "mask.wear-alternate" => ("Make another interface available", "Add an alternate mask to the body's choices. Making it eligible does not yet mean it is selected."),
        "mask.prefer-alternate" => ("Choose the alternate interface", "Select the available alternate mask. Inspect the result under the existing execution plan."),
        "mask.withdraw-selected-route" => ("Remove the selected interface's route", "Observe what happens when the selected presentation can no longer be realized."),
        "mask.inspect-unavailable-show" => ("Inspect the unavailable interface", "Check the recorded absence of a presentation. The body must not present an unavailable route as still working."),
        "mask.add-face-host" => ("Make a replacement host available", "Introduce another possible way to present the interface. Availability alone does not authorize a new execution plan."),
        "mask.admit-replacement-plan" => ("Admit a replacement plan", "Follow the authorized change to a new plan before expecting the replacement presentation to work."),
        "mask.inspect-replanned-show" => ("Inspect the replacement presentation", "Review the interface produced by the new plan and its recorded relationship to the same body."),
        "mask.doff-alternate" => ("Remove the alternate choice", "Remove the alternate mask from the eligible choices. It must no longer remain selected."),
        "mask.inspect-restored-show" => ("Check the original interface again", "Inspect the original mask as realized under the replacement plan."),
        "journey.break-recover" => ("Inspect a failure and any recovery", "Read the failure first. Count recovery only when this track separately records it for the same obligation."),
        "journey.rest-finish" => ("Rest, then finish", "Follow the separate recorded actions that end the wake and then finish this body's lifecycle."),
        _ => ("Inspect the recorded action", "Read the producer result and its supporting evidence below."),
    }
}

fn label(body: &JourneyBodyCell) -> &'static str {
    match body.track_id.as_str() {
        "native-graphical" => "ConduitOS",
        "browser-graphical" => "Browser",
        _ => "Conversational",
    }
}

fn render_receipts(
    html: &mut String,
    body: &JourneyBodyCell,
    receipts: &[TrackStep],
    root: &str,
    git_commit: &str,
    mask_plot_id: &str,
) {
    for receipt in receipts {
        render_media(html, body, receipt, root);
        render_evidence(html, body, receipt, root, git_commit, mask_plot_id);
    }
}

fn render_media(html: &mut String, body: &JourneyBodyCell, observed: &TrackStep, root: &str) {
    if body.embodiment.contains("fixture") {
        html.push_str(
            "<p class=\"proof-mode\">Deterministic model fixture · not a live model conversation</p>",
        );
    }
    let has = |class: &str| {
        observed
            .evidence
            .iter()
            .any(|item| item.evidence_class == class)
    };
    if !has("screenshot") {
        html.push_str(
            "<p class=\"missing-media\">No screenshot was recorded for this part of the step.</p>",
        );
    }
    if body.track_id == "hosted-generative" && !has("audio") {
        html.push_str("<p class=\"missing-media\">No audio was recorded. Text or fixture evidence does not demonstrate a spoken conversation.</p>");
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

fn render_evidence(
    html: &mut String,
    body: &JourneyBodyCell,
    observed: &TrackStep,
    root: &str,
    git_commit: &str,
    mask_plot_id: &str,
) {
    html.push_str("<details class=\"evidence\"><summary>Evidence</summary><ul>");
    let _ = write!(
        html,
        "<li>Recorded source: {} · Mask Plot: {}</li>",
        escape(git_commit),
        escape(mask_plot_id)
    );
    for construction in &body.construction {
        let _ = write!(
            html,
            "<li>Host {} construction: profile {} · build {} · image {}</li>",
            escape(&construction.host_id),
            escape(&construction_label(&construction.profile)),
            escape(&construction_label(&construction.build)),
            escape(&construction_label(&construction.image)),
        );
    }
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

fn construction_label(stage: &ConstructionStage) -> String {
    match stage {
        ConstructionStage::Exact { identity } => format!("exact {identity}"),
        ConstructionStage::Omitted { reason } => format!("omitted: {reason}"),
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
