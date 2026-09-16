use std::{fs, path::Path};

use super::{
    copy_file, escape_html, optional, required_output, write_html, VerifiedEvidence, VerifiedOutput,
};

pub(super) fn write_conduitos_commit(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    let root = site_root
        .join("commits")
        .join(&evidence.commit)
        .join("conduitos/x86_64");
    fs::create_dir_all(&root)
        .map_err(|error| format!("cannot create ConduitOS commit gallery: {error}"))?;
    let output = required_output(evidence, "conduitos.x86_64.console")?;
    copy_file(
        &evidence_root.join("manifest.json"),
        &root.join("manifest.json"),
    )?;
    copy_file(&evidence_root.join(&output.path), &root.join("console.txt"))?;
    write_conduitos_page(
        &root.join("index.html"),
        "../../../../index.html",
        evidence,
        output,
        &evidence_root.join(&output.path),
    )
}

pub(super) fn write_conduitos_current(
    site_root: &Path,
    evidence_root: &Path,
    evidence: &VerifiedEvidence,
) -> Result<(), String> {
    let root = site_root.join("current/conduitos/x86_64");
    let current_family = site_root.join("current/conduitos");
    if current_family.exists() {
        fs::remove_dir_all(&current_family)
            .map_err(|error| format!("cannot replace current ConduitOS evidence: {error}"))?;
    }
    fs::create_dir_all(&root)
        .map_err(|error| format!("cannot create current ConduitOS gallery: {error}"))?;
    let output = required_output(evidence, "conduitos.x86_64.console")?;
    copy_file(
        &evidence_root.join("manifest.json"),
        &root.join("manifest.json"),
    )?;
    copy_file(&evidence_root.join(&output.path), &root.join("console.txt"))?;
    write_conduitos_page(
        &root.join("index.html"),
        "../../../index.html",
        evidence,
        output,
        &evidence_root.join(&output.path),
    )
}

fn write_conduitos_page(
    path: &Path,
    home: &str,
    evidence: &VerifiedEvidence,
    output: &VerifiedOutput,
    transcript_path: &Path,
) -> Result<(), String> {
    let transcript = fs::read_to_string(transcript_path)
        .map_err(|error| format!("cannot read verified ConduitOS transcript: {error}"))?;
    let provenance = &output.provenance;
    let bytes = output.bytes.to_string();
    let byte_limit = provenance
        .capture_byte_limit
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not recorded".into());
    let physical = provenance
        .physical_evidence
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not recorded".into());
    let rows = [
        ("Proof", evidence.proof_id.as_str()),
        ("Suite", evidence.suite_id.as_str()),
        ("Commit", evidence.commit.as_str()),
        ("Proof class", optional(&provenance.proof_class)),
        ("Architecture", optional(&provenance.architecture)),
        ("Accepted rung", optional(&provenance.architecture_rung)),
        ("Emulator", optional(&provenance.emulator)),
        ("Emulator version", optional(&provenance.emulator_version)),
        ("Machine", optional(&provenance.machine)),
        ("Firmware", optional(&provenance.firmware)),
        ("Host", optional(&provenance.host_id)),
        ("Boot", optional(&provenance.boot_id)),
        ("Kernel artifact", optional(&provenance.kernel_artifact_id)),
        (
            "Kernel artifact SHA-256",
            optional(&provenance.kernel_artifact_sha256),
        ),
        ("Plan", optional(&provenance.plan_id)),
        ("Active Play", optional(&provenance.active_play_id)),
        ("Capture trigger", optional(&provenance.capture_trigger)),
        ("Transcript bytes", bytes.as_str()),
        ("Transcript byte limit", byte_limit.as_str()),
        ("Evidence SHA-256", output.sha256.as_str()),
        ("Physical evidence", physical.as_str()),
    ]
    .into_iter()
    .map(|(name, value)| {
        format!(
            "<dt>{}</dt><dd><code>{}</code></dd>",
            escape_html(name),
            escape_html(value)
        )
    })
    .collect::<Vec<_>>()
    .join("\n");
    let body = format!(
        "<nav><a href=\"{home}\">All journeys</a></nav><p class=\"eyebrow\">A computer is born</p><h1>ConduitOS wakes a Body</h1><p class=\"lede\">One freestanding x86_64 machine boots, establishes its exact Host and kernel identities, and reaches semantic work.</p><p class=\"boundary\"><strong>QEMU evidence, not physical hardware evidence.</strong> Every moment below comes from one bounded validated console transcript; the story adds no simulated screen or event.</p><section class=\"story-path\" aria-label=\"Evidence story\"><article class=\"checkpoint\"><p class=\"step\">1 · The machine woke</p><h2>Boot crossed into the Conduit kernel</h2><dl><dt>What you see</dt><dd>The retained console bytes for the exact machine, firmware, and kernel artifact.</dd><dt>What happened</dt><dd>The reviewed QEMU profile booted the freestanding x86_64 image.</dd><dt>What Conduit established</dt><dd>Structured boot and kernel assertions passed before the transcript was admitted.</dd><dt>Concepts in view</dt><dd>Host · Boot · kernel artifact</dd><dt>Evidence</dt><dd><a href=\"console.txt\">Exact console transcript</a></dd></dl></article><article class=\"checkpoint\"><p class=\"step\">2 · The Body reached work</p><h2>One Plan and Play became observable</h2><dl><dt>What you see</dt><dd>The Observatory and semantic-presentation records later in the same transcript.</dd><dt>What happened</dt><dd>ConduitOS exposed admitted Host offers and carried semantic work to its validated terminal condition.</dd><dt>What Conduit established</dt><dd>The exact Plan and active Play identities recorded below belong to this accepted run.</dd><dt>Concepts in view</dt><dd>Body · Host · Plan · Play · Presentation</dd><dt>Evidence</dt><dd><a href=\"manifest.json\">Digest-bound manifest</a> · <a href=\"console.txt\">complete transcript</a></dd></dl></article></section><div class=\"proof-grid\"><section><h2>What this proves</h2><p>The exact freestanding QEMU profile completed its structured boot, kernel, Observatory, semantic-presentation, and terminal debug-exit conditions.</p></section><section><h2>What it does not prove</h2><p>It does not establish boot or output on physical x86_64 hardware.</p></section></div><h2>Reproduce</h2><p>Use the supported x86_64 ConduitOS journey command documented for this exact machine profile:</p><pre class=\"reproduce\"><code>cargo xtask conduitos journey-proof</code></pre><details><summary>Exact provenance and all checkpoints</summary><dl>{rows}</dl><h2>Validated console transcript</h2><pre>{}</pre></details>",
        escape_html(&transcript)
    );
    write_html(path, "x86_64 ConduitOS emulator console evidence", &body)
}
