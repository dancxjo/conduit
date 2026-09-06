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
        "<nav><a href=\"{home}\">Gallery home</a></nav>\n<h1>x86_64 ConduitOS console evidence</h1>\n<p><strong>FREESTANDING EMULATOR EVIDENCE — NOT PHYSICAL HARDWARE EVIDENCE.</strong></p>\n<p>The transcript was retained only after the structured boot, kernel, Observatory, semantic presentation, and terminal debug-exit conditions passed.</p>\n<p><a href=\"console.txt\">Download exact transcript bytes</a></p>\n<h2>Exact provenance</h2>\n<dl>{rows}</dl>\n<h2>Validated console transcript</h2>\n<pre>{}</pre>",
        escape_html(&transcript)
    );
    write_html(path, "x86_64 ConduitOS emulator console evidence", &body)
}
