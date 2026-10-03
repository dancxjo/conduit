use std::{fs, path::Path};

use super::{safe_asset_path, Journey, OneBodyJourneyRequest, ValidatedChapter};
use crate::evidence::{
    verify, EvidenceKind, ExpectedEvidenceResult, VerificationRequest, VerifiedEvidence,
};

const CSS: &str = r#"
.journey{max-width:76rem;margin:0 auto;padding:2rem 1.25rem 5rem;color:var(--conduit-text-primary)}
.journey h1{font-size:clamp(2.2rem,5vw,4rem);line-height:1.1;max-width:18ch}
.journey h2{font-size:clamp(1.6rem,3vw,2.3rem);line-height:1.2}
.journey .lede{font-size:1.2rem;max-width:64ch}
.journey .chapter-links{display:flex;flex-wrap:wrap;gap:.65rem;margin:2rem 0;padding:0;list-style:none}
.journey .chapter-links a,.journey .next-link{display:inline-block;padding:.55rem .8rem;border:1px solid var(--conduit-structure-secondary);border-radius:.35rem}
.journey article{border:1px solid var(--conduit-structure-secondary);border-radius:.5rem;background:var(--conduit-surface);padding:clamp(1rem,3vw,2rem);margin:2rem 0}
.journey .step{font-weight:700;color:var(--conduit-emphasis)}
.journey .story{display:grid;grid-template-columns:minmax(9rem,12rem) minmax(0,1fr);gap:.5rem 1.25rem;max-width:65rem}
.journey .story dt{font-weight:700}.journey .story dd{margin:0}
.journey .media-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,26rem),1fr));gap:1.25rem;margin:1.5rem 0}
.journey figure{margin:0}.journey img{max-width:100%;height:auto;border:1px solid var(--conduit-structure-secondary)}
.journey figcaption{margin:.4rem 0;color:var(--conduit-text-secondary)}
.journey audio{width:100%}.journey pre{max-height:20rem;overflow:auto;white-space:pre-wrap;word-break:break-word;padding:1rem;border:1px solid var(--conduit-structure-secondary)}
.journey details{margin-top:1.25rem;padding:1rem;border:1px solid var(--conduit-structure-secondary);border-radius:.35rem}.journey summary{cursor:pointer;font-weight:700}
.journey .boundary{border-left:3px solid var(--conduit-emphasis);padding-left:1rem;max-width:68ch}
.journey code{overflow-wrap:anywhere}
@media(max-width:650px){.journey .story{grid-template-columns:1fr}.journey .story dd{margin-bottom:.75rem}}
"#;

pub(super) fn write(
    request: &OneBodyJourneyRequest,
    source: &Path,
    evidence: &VerifiedEvidence,
    journey: &Journey,
    chapters: &[ValidatedChapter<'_>],
) -> Result<(), String> {
    let parent = request
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|error| format!("create journey output parent: {error}"))?;
    let staging = parent.join(format!(
        ".one-body-journey-{}-{}",
        std::process::id(),
        journey.run_id
    ));
    if staging.exists() {
        return Err("journey staging path already exists".into());
    }
    fs::create_dir(&staging)
        .map_err(|error| format!("create journey staging directory: {error}"))?;
    let result = (|| {
        fs::copy(source.join("manifest.json"), staging.join("manifest.json"))
            .map_err(|error| format!("copy journey evidence manifest: {error}"))?;
        for output in &evidence.outputs {
            let relative = safe_asset_path(&output.path)?;
            let destination = staging.join(&relative);
            if let Some(directory) = destination.parent() {
                fs::create_dir_all(directory)
                    .map_err(|error| format!("create journey asset directory: {error}"))?;
            }
            fs::copy(source.join(&output.path), destination)
                .map_err(|error| format!("copy verified journey asset '{}': {error}", output.id))?;
        }
        // A producer can still be writing the source while files are copied.
        // Recompute every copied digest before making the page visible.
        verify(&VerificationRequest {
            root: staging.clone(),
            commit: evidence.commit.clone(),
            result: ExpectedEvidenceResult::Complete,
            proof_id: evidence.proof_id.clone(),
            suite_id: evidence.suite_id.clone(),
        })?;
        let html = document(evidence, journey, chapters, &staging)?;
        fs::write(staging.join("index.html"), html)
            .map_err(|error| format!("write journey page: {error}"))?;
        fs::rename(&staging, &request.output)
            .map_err(|error| format!("publish complete journey output: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn document(
    evidence: &VerifiedEvidence,
    journey: &Journey,
    chapters: &[ValidatedChapter<'_>],
    source: &Path,
) -> Result<String, String> {
    let mut links = String::new();
    let mut content = String::new();
    for (index, chapter) in chapters.iter().enumerate() {
        let step = index + 1;
        links.push_str(&format!(
            "<li><a href=\"#{}\">{} · {}</a></li>",
            escape(&chapter.story.id),
            step,
            escape(&chapter.story.title)
        ));
        let mut media = String::new();
        let mut evidence_links = String::new();
        for item in &chapter.media {
            let href = safe_asset_path(&item.output.path)?;
            let receipt = safe_asset_path(&item.receipt.path)?;
            evidence_links.push_str(&format!(
                "<li><a href=\"{}\">Capture receipt for {}</a> · <code>{}</code></li>",
                escape(&receipt),
                escape(&item.output.id),
                escape(&item.output.sha256)
            ));
            match item.output.kind {
                EvidenceKind::Screenshot => media.push_str(&format!(
                    "<figure><img src=\"{}\" alt=\"{}\" loading=\"lazy\"><figcaption>{}</figcaption></figure>",
                    escape(&href), escape(item.alt), escape(item.alt))),
                EvidenceKind::ConsoleTranscript => {
                    let text = fs::read_to_string(source.join(&item.output.path))
                        .map_err(|error| format!("read terminal capture: {error}"))?;
                    let excerpt: String = text.chars().take(8000).collect();
                    let truncated = text.chars().count() > 8000;
                    media.push_str(&format!("<figure><figcaption>{}</figcaption><pre>{}</pre>{}</figure>",
                        escape(item.alt), escape(&excerpt),
                        if !truncated { format!("<a href=\"{}\">Open complete terminal capture</a>", escape(&href)) }
                        else { format!("<p>Excerpt shown. <a href=\"{}\">Open complete terminal capture</a></p>", escape(&href)) }));
                }
                EvidenceKind::Audio => {
                    let transcript = item.transcript.ok_or("validated audio lost transcript")?;
                    let transcript_href = safe_asset_path(&transcript.path)?;
                    let mode = item.mode.ok_or("validated audio lost speech mode")?;
                    media.push_str(&format!("<figure><figcaption>{}</figcaption><audio controls preload=\"none\" src=\"{}\"><a href=\"{}\">Download produced speech</a></audio><p><strong>Words in produced audio ({}):</strong> {}</p><p><a href=\"{}\">Transcript and source identity</a></p></figure>",
                        escape(item.alt), escape(&href), escape(&href), escape(mode),
                        escape(item.transcript_text.as_deref().unwrap_or("")), escape(&transcript_href)));
                    if let Some(validation) = item.validation {
                        evidence_links.push_str(&format!("<li><a href=\"{}\">Original model output and validation receipt</a></li>",
                            escape(&safe_asset_path(&validation.path)?)));
                    }
                }
                EvidenceKind::MachineReadableManifest => unreachable!("validated media is visible"),
            }
        }
        let limitations = chapter
            .story
            .limitations
            .iter()
            .map(|note| format!("<li>{}</li>", escape(note)))
            .collect::<String>();
        let receipt = safe_asset_path(&chapter.receipt.path)?;
        let next = if index + 1 < chapters.len() {
            format!(
                "<a class=\"next-link\" href=\"#{}\">Continue to {}</a>",
                escape(&chapters[index + 1].story.id),
                escape(&chapters[index + 1].story.title)
            )
        } else {
            "<a class=\"next-link\" href=\"/conduit/journeys/\">Explore other journeys</a>".into()
        };
        content.push_str(&format!("<article id=\"{}\" aria-labelledby=\"title-{}\"><p class=\"step\">Chapter {} of 8</p><h2 id=\"title-{}\">{}</h2><dl class=\"story\"><dt>You want to</dt><dd>{}</dd><dt>Do this</dt><dd>{}</dd><dt>What changes</dt><dd>{}</dd><dt>Why it matters</dt><dd>{}</dd><dt>Try next</dt><dd>{}</dd></dl><div class=\"media-grid\">{media}</div><details><summary>Evidence and limits</summary><p><a href=\"{}\">Chapter action receipt</a></p><ul>{evidence_links}</ul><h3>What this does not establish</h3><ul>{limitations}</ul></details><p>{next}</p></article>",
            escape(&chapter.story.id), escape(&chapter.story.id), step, escape(&chapter.story.id), escape(&chapter.story.title),
            escape(&chapter.story.intention), escape(&chapter.story.action), escape(&chapter.story.result),
            escape(&chapter.story.why), escape(&chapter.story.next), escape(&receipt)));
    }
    Ok(format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>One Body, five ways to meet it — Conduit</title><style>{}\n{CSS}</style></head><body data-application-theme=\"conduit.presentation/phosphor@1\">{}<main class=\"journey\"><header><p class=\"step\">A real user journey · eight chapters</p><h1>One Body, five ways to meet it</h1><p class=\"lede\">Start a clock, move between browser, ConduitOS and terminal, hear its current state, then see what happens when a place or provider disappears. Every capture below belongs to one recorded run.</p><p class=\"boundary\">The captured run proves only the actions and effects named in its receipts. QEMU is emulator evidence; audio production and playback are separate from attended human listening.</p></header><nav aria-label=\"Journey chapters\"><ol class=\"chapter-links\">{links}</ol></nav>{content}<details><summary>Source and complete evidence inventory</summary><p>Source commit: <code>{}</code></p><p>Run: <code>{}</code> · Body: <code>{}</code></p><p><a href=\"{}\">Journey document</a> · <a href=\"manifest.json\">Digest-bound evidence manifest</a></p></details></main></body></html>",
        crate::site::styles(), crate::site::navigation("journeys"), escape(&evidence.commit),
        escape(&journey.run_id), escape(&journey.body_id),
        escape(&safe_asset_path(&evidence.outputs.iter().find(|output| output.id == "journey").ok_or("missing journey document")?.path)?)))
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
