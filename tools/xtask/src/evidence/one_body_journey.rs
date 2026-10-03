//! Publication gate for #4807. The producer owns the live run; this module
//! checks its retained, digest-bound result and renders no partial journey.

mod media;
mod render;

use media::{digest, png, safe_asset_path, wav};

use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use super::{verify, EvidenceKind, ExpectedEvidenceResult, VerificationRequest, VerifiedOutput};

const SCHEMA: &str = "conduit.journey/one-body-five-masks@1";
const CHAPTERS: [&str; 8] = [
    "birth", "join", "start", "see", "hear", "loss", "return", "lull",
];
const MAX_TEXT: usize = 2048;
const MAX_JOURNEY_OUTPUT_BYTES: u64 = 128 * 1024 * 1024;

pub struct OneBodyJourneyRequest {
    pub evidence_root: PathBuf,
    pub output: PathBuf,
    pub commit: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Journey {
    schema: String,
    source_commit: String,
    run_id: String,
    body_id: String,
    chapters: Vec<Chapter>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chapter {
    id: String,
    title: String,
    intention: String,
    action: String,
    result: String,
    why: String,
    next: String,
    receipt_id: String,
    media: Vec<Media>,
    limitations: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Media {
    output_id: String,
    receipt_id: String,
    alt: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChapterReceipt {
    schema: String,
    source_commit: String,
    run_id: String,
    body_id: String,
    chapter_id: String,
    action_ids: Vec<String>,
    resulting_face_revision: String,
    outcome: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureReceipt {
    schema: String,
    source_commit: String,
    run_id: String,
    body_id: String,
    chapter_id: String,
    action_id: String,
    face_revision: String,
    media_output_id: String,
    media_sha256: String,
    capture_source: String,
    show_id: Option<String>,
    plan_id: Option<String>,
    play_id: Option<String>,
    transcript_id: Option<String>,
    transcript_sha256: Option<String>,
    speech_mode: Option<String>,
    voice_id: Option<String>,
    provider_id: Option<String>,
    model_id: Option<String>,
    validation_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeechTranscript {
    schema: String,
    source_commit: String,
    run_id: String,
    body_id: String,
    chapter_id: String,
    show_id: String,
    face_revision: String,
    text: String,
    original_model_output: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelValidation {
    schema: String,
    source_commit: String,
    run_id: String,
    body_id: String,
    show_id: String,
    face_revision: String,
    provider_id: String,
    model_id: String,
    original_output_sha256: String,
    validated_text_sha256: String,
    accepted: bool,
}

struct ValidatedMedia<'a> {
    output: &'a VerifiedOutput,
    receipt: &'a VerifiedOutput,
    alt: &'a str,
    transcript: Option<&'a VerifiedOutput>,
    transcript_text: Option<String>,
    mode: Option<&'a str>,
    validation: Option<&'a VerifiedOutput>,
}

struct ValidatedChapter<'a> {
    story: &'a Chapter,
    receipt: &'a VerifiedOutput,
    media: Vec<ValidatedMedia<'a>>,
}

pub fn render_one_body_journey(request: &OneBodyJourneyRequest) -> Result<(), String> {
    if fs::symlink_metadata(&request.output).is_ok() {
        return Err("journey output already exists; refusing to replace retained evidence".into());
    }
    let evidence = verify(&VerificationRequest {
        root: request.evidence_root.clone(),
        commit: request.commit.clone(),
        result: ExpectedEvidenceResult::Complete,
        proof_id: "journey-one-body-five-masks".into(),
        suite_id: "journey-gallery".into(),
    })?;
    if evidence
        .outputs
        .iter()
        .map(|output| output.bytes)
        .sum::<u64>()
        > MAX_JOURNEY_OUTPUT_BYTES
    {
        return Err("journey evidence exceeds the 128 MiB publication bundle limit".into());
    }
    let root = request
        .evidence_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let outputs: BTreeMap<_, _> = evidence
        .outputs
        .iter()
        .map(|output| (output.id.as_str(), output))
        .collect();
    let story_output = output(&outputs, "journey")?;
    json_kind(story_output)?;
    let journey: Journey = read_json(&root, story_output)?;
    if journey.schema != SCHEMA
        || journey.source_commit != evidence.commit
        || !token(&journey.run_id)
        || !identity(&journey.body_id)
        || journey.chapters.len() != CHAPTERS.len()
    {
        return Err("journey schema, source, run, Body, or chapter count does not match".into());
    }
    if evidence
        .outputs
        .iter()
        .any(|output| !output.required || output.provenance.scenario_id != journey.run_id)
    {
        return Err("journey evidence includes an optional or foreign-run output".into());
    }
    let mut used = BTreeSet::new();
    let mut all_sources = BTreeSet::new();
    let mut audio_modes = BTreeSet::new();
    let mut chapters = Vec::with_capacity(CHAPTERS.len());
    for (index, chapter) in journey.chapters.iter().enumerate() {
        if chapter.id != CHAPTERS[index]
            || chapter.media.is_empty()
            || chapter.media.len() > 12
            || chapter.limitations.is_empty()
            || chapter.limitations.len() > 8
            || [
                &chapter.title,
                &chapter.intention,
                &chapter.action,
                &chapter.result,
                &chapter.why,
                &chapter.next,
            ]
            .iter()
            .any(|value| !narrative(value))
            || chapter.limitations.iter().any(|value| !narrative(value))
        {
            return Err(format!(
                "chapter {} is absent, out of order, or lacks usable copy/media",
                index + 1
            ));
        }
        let chapter_output = output(&outputs, &chapter.receipt_id)?;
        json_kind(chapter_output)?;
        let receipt: ChapterReceipt = read_json(&root, chapter_output)?;
        if receipt.schema != "conduit.journey/chapter-receipt@1"
            || receipt.source_commit != journey.source_commit
            || receipt.run_id != journey.run_id
            || receipt.body_id != journey.body_id
            || receipt.chapter_id != chapter.id
            || receipt.outcome != "completed"
            || !identity(&receipt.resulting_face_revision)
            || receipt.action_ids.is_empty()
            || receipt.action_ids.len() > 24
            || receipt.action_ids.iter().any(|id| !identity(id))
            || receipt.action_ids.iter().collect::<BTreeSet<_>>().len() != receipt.action_ids.len()
        {
            return Err(format!(
                "chapter '{}' has no correlated completed action receipt",
                chapter.id
            ));
        }
        let mut media = Vec::with_capacity(chapter.media.len());
        for item in &chapter.media {
            if !narrative(&item.alt) || !used.insert(item.output_id.as_str()) {
                return Err(format!(
                    "chapter '{}' repeats or omits a media description",
                    chapter.id
                ));
            }
            let artifact = output(&outputs, &item.output_id)?;
            let capture_output = output(&outputs, &item.receipt_id)?;
            json_kind(capture_output)?;
            let capture: CaptureReceipt = read_json(&root, capture_output)?;
            if capture.schema != "conduit.journey/capture-receipt@1"
                || capture.source_commit != journey.source_commit
                || capture.run_id != journey.run_id
                || capture.body_id != journey.body_id
                || capture.chapter_id != chapter.id
                || !receipt.action_ids.contains(&capture.action_id)
                || capture.face_revision != receipt.resulting_face_revision
                || capture.media_output_id != artifact.id
                || capture.media_sha256 != artifact.sha256
            {
                return Err(format!(
                    "media '{}' does not match its run, action, Face, or digest",
                    artifact.id
                ));
            }
            let source = match artifact.kind {
                EvidenceKind::Screenshot
                    if artifact.media_type == "image/png" && png(&root.join(&artifact.path))? =>
                {
                    if !matches!(capture.capture_source.as_str(), "chromium" | "qmp") {
                        return Err("screenshot lacks live browser or QMP capture source".into());
                    }
                    capture.capture_source.as_str()
                }
                EvidenceKind::ConsoleTranscript
                    if artifact.media_type == "text/plain; charset=utf-8" =>
                {
                    if capture.capture_source != "terminal" {
                        return Err("terminal capture lacks terminal source".into());
                    }
                    "terminal"
                }
                EvidenceKind::Audio
                    if artifact.media_type == "audio/wav" && wav(&root.join(&artifact.path))? =>
                {
                    if capture.capture_source != "runtime-speech" {
                        return Err("audio lacks runtime speech source".into());
                    }
                    "runtime-speech"
                }
                _ => {
                    return Err(format!(
                        "media '{}' is not a supported real capture",
                        artifact.id
                    ))
                }
            };
            all_sources.insert(source.to_owned());
            let mut transcript = None;
            let mut transcript_text = None;
            let mut validation = None;
            if artifact.kind == EvidenceKind::Audio {
                let transcript_output = output(
                    &outputs,
                    capture
                        .transcript_id
                        .as_deref()
                        .ok_or("audio has no transcript")?,
                )?;
                json_kind(transcript_output)?;
                let words: SpeechTranscript = read_json(&root, transcript_output)?;
                if words.schema != "conduit.journey/speech-transcript@1"
                    || words.source_commit != journey.source_commit
                    || words.run_id != journey.run_id
                    || words.body_id != journey.body_id
                    || words.chapter_id != chapter.id
                    || Some(words.show_id.as_str()) != capture.show_id.as_deref()
                    || words.face_revision != capture.face_revision
                    || !narrative(&words.text)
                    || capture.transcript_sha256.as_deref()
                        != Some(transcript_output.sha256.as_str())
                    || capture.show_id.as_deref().is_none_or(|id| !identity(id))
                    || capture.plan_id.as_deref().is_none_or(|id| !identity(id))
                    || capture.play_id.as_deref().is_none_or(|id| !identity(id))
                    || capture.voice_id.as_deref().is_none_or(|id| !identity(id))
                {
                    return Err(format!(
                        "audio '{}' lacks exact speech/Show/Plan/Play correlation",
                        artifact.id
                    ));
                }
                if capture.speech_mode.as_deref() == Some("llm-assisted") {
                    let validation_output = output(
                        &outputs,
                        capture
                            .validation_id
                            .as_deref()
                            .ok_or("LLM speech lacks validation")?,
                    )?;
                    json_kind(validation_output)?;
                    let check: ModelValidation = read_json(&root, validation_output)?;
                    let original = words
                        .original_model_output
                        .as_deref()
                        .ok_or("LLM speech lacks original model output")?;
                    if check.schema != "conduit.journey/model-validation@1"
                        || check.source_commit != journey.source_commit
                        || check.run_id != journey.run_id
                        || check.body_id != journey.body_id
                        || check.show_id != words.show_id
                        || check.face_revision != words.face_revision
                        || !check.accepted
                        || Some(check.provider_id.as_str()) != capture.provider_id.as_deref()
                        || !identity(&check.provider_id)
                        || Some(check.model_id.as_str()) != capture.model_id.as_deref()
                        || !identity(&check.model_id)
                        || check.original_output_sha256 != digest(original.as_bytes())
                        || check.validated_text_sha256 != digest(words.text.as_bytes())
                    {
                        return Err(
                            "LLM original output or Face validation is not correlated".into()
                        );
                    }
                    validation = Some(validation_output);
                } else if capture.speech_mode.as_deref() != Some("direct")
                    || words.original_model_output.is_some()
                    || capture.validation_id.is_some()
                    || capture.provider_id.is_some()
                    || capture.model_id.is_some()
                {
                    return Err("direct speech carries a model claim or missing mode".into());
                }
                if chapter.id == "hear" {
                    audio_modes.insert(capture.speech_mode.clone().unwrap());
                }
                transcript = Some(transcript_output);
                transcript_text = Some(words.text);
            } else if capture.transcript_id.is_some()
                || capture.transcript_sha256.is_some()
                || capture.speech_mode.is_some()
                || capture.validation_id.is_some()
                || capture.provider_id.is_some()
                || capture.model_id.is_some()
                || capture.voice_id.is_some()
            {
                return Err("visual capture carries unsupported speech metadata".into());
            }
            media.push(ValidatedMedia {
                output: artifact,
                receipt: capture_output,
                alt: &item.alt,
                transcript,
                transcript_text,
                mode: capture.speech_mode.as_deref().map(|_| {
                    if capture.speech_mode.as_deref() == Some("direct") {
                        "direct"
                    } else {
                        "llm-assisted"
                    }
                }),
                validation,
            });
        }
        chapters.push(ValidatedChapter {
            story: chapter,
            receipt: chapter_output,
            media,
        });
    }
    if !["chromium", "qmp", "terminal", "runtime-speech"]
        .iter()
        .all(|source| all_sources.contains(*source))
        || !["direct", "llm-assisted"]
            .iter()
            .all(|mode| audio_modes.contains(*mode))
    {
        return Err("journey lacks browser, QMP, terminal, direct, or LLM runtime media".into());
    }
    render::write(request, &root, &evidence, &journey, &chapters)
}

fn output<'a>(
    outputs: &BTreeMap<&str, &'a VerifiedOutput>,
    id: &str,
) -> Result<&'a VerifiedOutput, String> {
    outputs
        .get(id)
        .copied()
        .ok_or_else(|| format!("missing digest-verified journey output '{id}'"))
}

fn json_kind(output: &VerifiedOutput) -> Result<(), String> {
    if output.kind != EvidenceKind::MachineReadableManifest
        || output.media_type != "application/json"
    {
        Err(format!("'{}' must be a JSON evidence output", output.id))
    } else {
        Ok(())
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(
    root: &Path,
    output: &VerifiedOutput,
) -> Result<T, String> {
    serde_json::from_slice(&fs::read(root.join(&output.path)).map_err(|error| error.to_string())?)
        .map_err(|error| format!("invalid '{}': {error}", output.id))
}

fn identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}
fn narrative(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests;
