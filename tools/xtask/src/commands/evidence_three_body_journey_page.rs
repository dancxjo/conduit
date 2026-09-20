//! Human-readable projection of an already verified three-Body Journey index.

use std::fmt::Write;

use super::{BodyTrack, ContractStep, ThreeBodyJourneyIndex, TrackStep};

pub(super) fn render(index: &ThreeBodyJourneyIndex) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Three Bodies, one semantic Journey</title><style>\
         body{font:16px/1.5 system-ui,sans-serif;max-width:90rem;margin:auto;padding:2rem;color:#18212b}\
         nav a{margin-right:1rem}section{margin:3rem 0}.track,.step{border:1px solid #ccd5df;border-radius:.6rem;padding:1rem;margin:1rem 0}\
         table{border-collapse:collapse;width:100%}th,td{border:1px solid #ccd5df;padding:.7rem;text-align:left;vertical-align:top}\
         code{overflow-wrap:anywhere}.nonclaim{color:#694100}dt{font-weight:700}dd{margin:0 0 .7rem}</style></head><body>",
    );
    let _ = write!(
        html,
        "<header><p>Verified evidence projection</p><h1>Three Bodies, one semantic Journey</h1>\
         <p>Journey <code>{}</code> at commit <code>{}</code>. Semantic assertions come from verified receipts; documentary media show what a human could observe.</p>\
         <nav><a href=\"#by-body\">Follow one body</a><a href=\"#by-step\">Compare one semantic step</a></nav></header>",
        escape(&index.journey_id),
        escape(&index.git_commit)
    );
    html.push_str("<section id=\"by-body\"><h2>Follow one body</h2>");
    for track in &index.tracks {
        render_track(&mut html, track, &index.semantic_steps);
    }
    html.push_str("</section><section id=\"by-step\"><h2>Compare one semantic step</h2>");
    for (step_index, contract) in index.semantic_steps.iter().enumerate() {
        render_step_comparison(&mut html, contract, step_index, &index.tracks);
    }
    html.push_str("</section></body></html>");
    html
}

fn render_track(html: &mut String, track: &BodyTrack, contracts: &[ContractStep]) {
    let _ = write!(
        html,
        "<article class=\"track\"><h3>{}</h3><p>Body <code>{}</code> · embodiment <code>{}</code> · Presenter <code>{}</code> · {} Host(s)</p>",
        escape(&track.track_id),
        escape(&track.body_id),
        escape(&track.embodiment),
        escape(&track.presenter_id),
        track.hosts.len()
    );
    for (contract, observed) in contracts.iter().zip(&track.steps) {
        render_observation(html, contract, observed);
    }
    html.push_str("</article>");
}

fn render_step_comparison(
    html: &mut String,
    contract: &ContractStep,
    step_index: usize,
    tracks: &[BodyTrack],
) {
    let _ = write!(
        html,
        "<article class=\"step\"><h3>{}</h3><p>{}</p><p><strong>Conduit established:</strong> {}</p>\
         <p><strong>Authoritative assertion rung:</strong> <code>{}</code></p>\
         <p class=\"nonclaim\"><strong>Does not establish:</strong> {}</p><table><thead><tr><th>Body</th><th>Exact realization evidence</th></tr></thead><tbody>",
        escape(&contract.title),
        escape(&contract.what_happened),
        escape(&contract.what_conduit_established),
        contract.required_assertion_rung.label(),
        joined(&contract.non_claims)
    );
    for track in tracks {
        let observed = &track.steps[step_index];
        let _ = write!(
            html,
            "<tr><th><code>{}</code><br>{}</th><td>",
            escape(&track.body_id),
            escape(&track.embodiment)
        );
        render_evidence(html, observed);
        html.push_str("</td></tr>");
    }
    html.push_str("</tbody></table></article>");
}

fn render_observation(html: &mut String, contract: &ContractStep, observed: &TrackStep) {
    let _ = write!(
        html,
        "<section><h4>{}</h4><dl><dt>What happened</dt><dd>{}</dd><dt>What Conduit established</dt><dd>{}</dd>\
         <dt>Authoritative assertion rung</dt><dd><code>{}</code></dd><dt>Concepts in view</dt><dd>{}</dd><dt>Disposition</dt><dd><code>{}</code></dd></dl>",
        escape(&contract.title),
        escape(&contract.what_happened),
        escape(&contract.what_conduit_established),
        contract.required_assertion_rung.label(),
        joined(&contract.concepts),
        escape(&observed.disposition)
    );
    render_evidence(html, observed);
    let _ = write!(
        html,
        "<p class=\"nonclaim\"><strong>Does not establish:</strong> {}</p></section>",
        joined(&contract.non_claims)
    );
}

fn render_evidence(html: &mut String, observed: &TrackStep) {
    let _ = write!(
        html,
        "<p><strong>Assertion:</strong> <code>{}</code></p><ul>",
        escape(&observed.assertion)
    );
    for evidence in &observed.evidence {
        let _ = write!(
            html,
            "<li>{} — <code>{}</code> · class <code>{}</code> · assertion rung <code>{}</code> · <code>{}</code> · <code>{}</code></li>",
            escape(&evidence.documentary_description),
            escape(&evidence.artifact_id),
            escape(&evidence.evidence_class),
            evidence.assertion_rung.label(),
            escape(&evidence.path.to_string_lossy()),
            escape(&evidence.sha256)
        );
    }
    html.push_str("</ul><details><summary>Exact provenance</summary><pre>");
    let provenance = serde_json::to_string_pretty(&observed.provenance)
        .unwrap_or_else(|_| "provenance serialization failed".into());
    html.push_str(&escape(&provenance));
    html.push_str("</pre></details>");
}

fn joined(values: &[String]) -> String {
    values
        .iter()
        .map(|value| escape(value))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
