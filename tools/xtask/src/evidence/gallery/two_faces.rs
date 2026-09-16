//! Static exhibit for one Presentation through two distinct renderers.

use std::path::Path;

use super::{copy_file, required_output, write_html};
use crate::evidence::VerifiedEvidence;

const FILES: &[(&str, &str)] = &[
    ("two-faces.native-frame", "native.png"),
    ("two-faces.native-receipt", "native.json"),
    ("two-faces.browser-frame", "browser.png"),
    ("two-faces.browser-receipt", "browser.json"),
];

pub(super) fn write_two_faces_commit(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root
            .join("commits")
            .join(&evidence.commit)
            .join("one-form-two-faces"),
        evidence_root,
        evidence,
        "../../../index.html",
    )
}

pub(super) fn write_two_faces_current(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    write_exhibit(
        &site_root.join("current/one-form-two-faces"),
        evidence_root,
        evidence,
        "../../index.html",
    )
}

fn write_exhibit(
    destination: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
    home: &str,
) -> Result<(), String> {
    if destination.exists() {
        std::fs::remove_dir_all(destination)
            .map_err(|error| format!("cannot replace Two Faces exhibit: {error}"))?;
    }
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("cannot create Two Faces exhibit: {error}"))?;
    for (id, filename) in FILES {
        let output = required_output(evidence, id)?;
        copy_file(
            &evidence_root.join(&output.path),
            &destination.join(filename),
        )?;
    }
    copy_file(
        &evidence_root.join("manifest.json"),
        &destination.join("manifest.json"),
    )?;
    let presentation = required_output(evidence, "two-faces.native-frame")?
        .provenance
        .presentation_id
        .as_deref()
        .ok_or("Two Faces native frame lacks Presentation identity")?;
    let body = format!(
        "<nav><a href=\"{home}\">All journeys</a></nav>\n<p class=\"eyebrow\">One meaning, two manifestations</p><h1>One Form, Two Faces</h1><p class=\"lede\">The same semantic Presentation crossed two renderer boundaries and kept its identity.</p><div class=\"identity-flow\"><strong>one authored Morse Network Form</strong><span>↓</span><code>{presentation}</code><span>↙︎ &nbsp; ↘︎</span><span>native renderer &nbsp; pinned Chromium renderer</span></div><section class=\"story-path\" aria-label=\"Evidence story\"><article class=\"checkpoint\"><p class=\"step\">1 · One Presentation</p><h2>Conduit prepared one meaning</h2><dl><dt>What you see</dt><dd>The shared Presentation identity before either renderer manifests it.</dd><dt>What happened</dt><dd>The reviewed Morse Network Form ran through the ordinary presentation path.</dd><dt>What Conduit established</dt><dd>Both retained outputs name the same Presentation and revision.</dd><dt>Concepts in view</dt><dd>Form · Plan · Play · Presentation</dd><dt>Evidence</dt><dd><a href=\"manifest.json\">Digest-bound manifest</a></dd></dl></article><article class=\"checkpoint\"><p class=\"step\">2 · Two honest faces</p><h2>Each renderer spoke its own visual language</h2><div class=\"comparison\"><figure><img src=\"native.png\" alt=\"Morse Network Form manifested by the native software renderer\"><figcaption>Native software manifestation</figcaption></figure><figure><img src=\"browser.png\" alt=\"The same Morse Network Presentation manifested in pinned Chromium\"><figcaption>Pinned Chromium manifestation</figcaption></figure></div><dl><dt>What you see</dt><dd>Two recognizably related but deliberately non-identical renderings.</dd><dt>What happened</dt><dd>The native and browser renderers independently manifested the shared Presentation.</dd><dt>What Conduit established</dt><dd>Presentation identity survived the renderer boundary; manifestation identity did not collapse into it.</dd><dt>Concepts in view</dt><dd>Presentation · Manifestation · Host</dd><dt>Evidence</dt><dd><a href=\"native.json\">Native receipt</a> · <a href=\"browser.json\">browser receipt</a></dd></dl></article></section><div class=\"proof-grid\"><section><h2>What this proves</h2><p>One semantic Presentation and revision were manifested by two distinct admitted renderers.</p></section><section><h2>What it does not prove</h2><p>Pixel equality, physical display output, and human perception are not claimed.</p></section></div><h2>Reproduce</h2><pre class=\"reproduce\"><code>npm --prefix proof/browser ci --ignore-scripts --prefer-offline\ncargo xtask evidence one-form-two-faces</code></pre><details><summary>Exact evidence and provenance</summary><p>Accepted commit: <code>{}</code></p><ul><li><a href=\"native.json\">Native renderer receipt</a></li><li><a href=\"browser.json\">Pinned Chromium receipt</a></li><li><a href=\"manifest.json\">Digest-bound manifest</a></li></ul></details>",
        evidence.commit
    );
    write_html(
        destination.join("index.html").as_path(),
        "One Form, Two Faces",
        &body,
    )
}
