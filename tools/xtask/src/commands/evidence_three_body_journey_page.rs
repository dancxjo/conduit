//! Human-readable projection of an already verified three-Body Journey index.

use std::fmt::Write;

use super::{BodyTrack, ContractStep, ThreeBodyJourneyIndex, TrackStep};

pub(super) fn render(index: &ThreeBodyJourneyIndex) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"dark\">\
         <title>Three Bodies, one semantic Journey</title><style>\
         :root{--ink:#f5f2e8;--muted:#aebbb3;--green:#77e6ad;--gold:#f5b95f;--paper:#090e0c;--card:#121d17;--line:#31473c}*{box-sizing:border-box}html{scroll-behavior:smooth}body{font:16px/1.55 system-ui,sans-serif;max-width:96rem;margin:auto;padding:clamp(1rem,4vw,4rem);color:var(--ink);background:radial-gradient(circle at 85% 0,#183b2a 0,transparent 28rem),var(--paper)}\
         header{min-height:70vh;display:flex;flex-direction:column;justify-content:center}header>p:first-child{color:var(--green);font-weight:850;letter-spacing:.15em;text-transform:uppercase;font-size:.76rem}h1{font-size:clamp(3.5rem,8vw,8rem);line-height:.86;letter-spacing:-.065em;max-width:12ch;margin:.15em 0}h2{font-size:clamp(2.2rem,5vw,4.5rem);line-height:1;letter-spacing:-.04em}h3{font-size:1.5rem}h4{font-size:1.15rem}a{color:var(--green);text-underline-offset:.2em}nav{display:flex;flex-wrap:wrap;gap:.75rem;margin-top:2rem}nav a{padding:.7rem 1rem;border:1px solid var(--green);border-radius:999px;text-decoration:none;font-weight:800}\
         section{margin:clamp(4rem,9vw,9rem) 0}.track,.step{border:1px solid var(--line);border-radius:1.2rem;padding:clamp(1rem,3vw,2.5rem);margin:1.25rem 0;background:linear-gradient(145deg,#15231c,var(--card))}.track>section{margin:2rem 0;padding:1.2rem;border-left:.25rem solid var(--green);background:#0d1511}.track:nth-of-type(2)>section{border-color:#73b9ff}.track:nth-of-type(3)>section{border-color:#ff806c}\
         table{border-collapse:separate;border-spacing:.45rem;width:100%}th,td{border:1px solid var(--line);border-radius:.6rem;padding:1rem;text-align:left;vertical-align:top;background:#0d1511}thead th{color:var(--green)}code,pre{overflow-wrap:anywhere;white-space:pre-wrap;color:#c7f7dc}.nonclaim{color:#e7c98e;border-left:.25rem solid var(--gold);padding-left:1rem}dt{font-weight:800;color:var(--muted)}dd{margin:0 0 .7rem}details{border-top:1px solid var(--line);padding-top:.8rem}summary{cursor:pointer;font-weight:800}@media(max-width:48rem){table,thead,tbody,tr,th,td{display:block}thead{display:none}tr{margin:1rem 0}h1{font-size:clamp(3.2rem,18vw,5.5rem)}}@media(prefers-reduced-motion:reduce){*{scroll-behavior:auto!important}}</style></head><body>",
    );
    let _ = write!(
        html,
        "<header><p>Verified evidence projection</p><h1>One Journey.<br>Three Bodies.</h1>\
         <p>One portable tutorial, lived independently through native ConduitOS, a browser, and a distributed conversational Body. Journey <code>{}</code> at exact commit <code>{}</code>.</p>\
         <p>Semantic assertions come from verified receipts. Documentary media show what a human could observe; they never become runtime truth.</p>\
         <nav aria-label=\"Choose how to read the Journey\"><a href=\"#by-body\">↓ Follow one Body's life</a><a href=\"#by-step\">→ Compare one semantic moment</a></nav></header>",
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
