//! One live installed Body Face through finite Presenter and ordinary spoken Mask.
//! This is one producer chapter, not a complete journey or a publication route.

use crate::evidence::{
    verify, EvidenceKind, EvidenceManifest, EvidenceResult, ExpectedEvidenceResult,
    VerificationRequest,
};
use clap::Args as ClapArgs;
use conduit_presentation::{
    GeneratedContentRole, GeneratedManifestationDisposition, GenerativePresenterBounds,
    GenerativePresenterRequest,
};
use conduit_std_host::{
    hosted_local_model::{
        finite_face_wording_presenter_policy, LocalModelKindProfile, OllamaDiscovery,
    },
    hosted_speech_synthesis::EspeakDiscovery,
    local_model_proof, spoken_mask_journey,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const PROOF_ID: &str = "journey-one-body-spoken-chapter";
const SUITE_ID: &str = "journey-gallery";

#[path = "evidence_one_body_spoken/owner.rs"]
mod owner;
#[path = "evidence_one_body_spoken/retention.rs"]
mod retention;
use owner::{face_bytes, git, hash_file, parse_snapshot};
use retention::{declare, retain, retain_json};

#[derive(ClapArgs, Debug)]
pub(crate) struct Args {
    /// Newly built installed owner executable from this exact source.
    #[arg(long)]
    conduit_bin: PathBuf,
    /// Isolated state directory of a running installed owner.
    #[arg(long)]
    state_dir: PathBuf,
    /// New evidence directory; no existing output is replaced.
    #[arg(long)]
    output: PathBuf,
    /// Capture owner's preassigned run token, supplied before this action begins.
    #[arg(long)]
    run_id: Option<String>,
    /// Already-local Ollama model name.
    #[arg(long)]
    model: String,
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_endpoint: String,
    #[arg(long, default_value_t = 2048)]
    admitted_memory_mib: u32,
    /// Exact installed eSpeak executable, data tree, and engine closure.
    #[arg(long)]
    speech_executable: PathBuf,
    #[arg(long)]
    speech_data: PathBuf,
    #[arg(long, required = true, num_args = 1..)]
    speech_engine: Vec<PathBuf>,
    #[arg(long, default_value = "en-us")]
    speech_voice: String,
}

pub(super) fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = crate::workspace::workspace_root()?;
    let source_commit = git(&workspace, &["rev-parse", "HEAD"])?;
    let changed = git(&workspace, &["status", "--porcelain"])?;
    if !changed.is_empty() {
        return Err("live journey producer requires a clean exact source tree".into());
    }
    let bin = fs::canonicalize(&args.conduit_bin)?;
    let state_dir = fs::canonicalize(&args.state_dir)?;
    if !bin.is_file() || !state_dir.is_dir() {
        return Err("installed owner executable and state directory are required".into());
    }
    let bin_sha256 = hash_file(&bin)?;
    let first_bytes = face_bytes(&bin, &state_dir)?;
    let first = parse_snapshot(&first_bytes)?;
    let body_id = first
        .presentation
        .basis
        .body_id
        .as_ref()
        .ok_or("owner Face has no Body identity")?;
    let run_nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let run_seed = format!(
        "{}/{}/{}/{}",
        source_commit,
        body_id.as_str(),
        first.presentation.revision,
        run_nonce,
    );
    let run_digest = format!("{:x}", Sha256::digest(run_seed.as_bytes()));
    let run_id = args
        .run_id
        .unwrap_or_else(|| format!("one-body-spoken-{}", &run_digest[..32]));
    if run_id.is_empty()
        || run_id.len() > 128
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("run ID must be a bounded alphanumeric, dash, or underscore token".into());
    }
    let execution_id = format!("journey-spoken-{}", &run_digest[..24]);
    let request = GenerativePresenterRequest::from_presentation(
        format!("request/journey-spoken/{}", &run_digest[..24]),
        finite_face_wording_presenter_policy(),
        first.presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .map_err(|error| format!("cannot prepare current Face Presenter request: {error:?}"))?;
    let discovery = OllamaDiscovery::discover_at(&args.ollama_endpoint, &args.model)?;
    let adapter = discovery.initialize(
        args.admitted_memory_mib,
        vec![
            LocalModelKindProfile::Generate,
            LocalModelKindProfile::ClassifyFiniteLabels,
            LocalModelKindProfile::ExtractValidatedInfo,
            LocalModelKindProfile::InterpretSignEvidence,
            LocalModelKindProfile::PresentSemanticFront,
        ],
    )?;
    let speech = EspeakDiscovery::inspect(
        &args.speech_executable,
        &args.speech_data,
        &args.speech_voice,
        &args.speech_engine,
    )?;

    fs::create_dir(&args.output)?;
    let mut manifest = EvidenceManifest::new(&args.output, &workspace, PROOF_ID, SUITE_ID)?;
    retain(
        &mut manifest,
        "owner-face",
        "owner-face.json",
        EvidenceKind::MachineReadableManifest,
        "application/json",
        &first_bytes,
        &run_id,
        &first,
        None,
    )?;
    retain_json(
        &mut manifest,
        "presenter-request",
        "presenter-request.json",
        &request,
        &run_id,
        &first,
        None,
    )?;
    let model_proof = match local_model_proof::run(adapter, &[request.clone()]) {
        Ok(proof) => proof,
        Err(error) => {
            manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
            return Err(error);
        }
    };
    retain_json(
        &mut manifest,
        "model-proof",
        "model-proof.json",
        &model_proof,
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    let [presenter] = model_proof.presenter_requests.as_slice() else {
        return Err("model proof did not retain exactly one Presenter result".into());
    };
    if !presenter.play_completed
        || presenter.request_identity != request.request_identity
        || presenter.source_presentation_identity != first.presentation.identity.as_str()
        || presenter.source_presentation_revision != first.presentation.revision
        || presenter.policy_revision != request.policy.template_contract_revision
    {
        return Err("Presenter receipt does not match the current owner Face".into());
    }
    let candidate = &presenter.manifestation;
    retain_json(
        &mut manifest,
        "candidate",
        "candidate.json",
        candidate,
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    let raw = candidate
        .raw_provider_output
        .as_ref()
        .ok_or("finite Presenter omitted original provider output")?;
    retain_json(
        &mut manifest,
        "original-model-output",
        "original-model-output.json",
        &json!({
            "schema": "conduit.journey/provider-output@1",
            "request_id": request.request_identity,
            "candidate_id": candidate.candidate_identity,
            "output": raw,
            "sha256": format!("{:x}", Sha256::digest(raw.as_bytes())),
        }),
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    let accepted = candidate.disposition == GeneratedManifestationDisposition::Produced;
    let spoken = if accepted {
        accepted_wording(&request, candidate)?
    } else {
        String::new()
    };
    let validation = json!({
        "schema": "conduit.journey/one-body-presenter-validation@1",
        "chapter_id": "hear",
        "proof_class": model_proof.proof_class,
        "source_commit": source_commit,
        "run_id": run_id,
        "body_id": body_id.as_str(),
        "host_id": first.advertisement.host_id,
        "boot_id": first.advertisement.boot_id,
        "face_id": first.presentation.identity,
        "face_revision": first.presentation.revision,
        "request_id": request.request_identity,
        "presenter_plan_id": presenter.plan_id,
        "presenter_play_completed": presenter.play_completed,
        "candidate_id": candidate.candidate_identity,
        "provider_id": candidate.provider_identity,
        "model_id": candidate.model_identity,
        "original_model_output_sha256": format!("{:x}", Sha256::digest(raw.as_bytes())),
        "accepted": accepted,
        "accepted_wording_sha256": accepted.then(|| format!("{:x}", Sha256::digest(spoken.as_bytes()))),
        "accepted_wording": accepted.then_some(spoken.as_str()),
        "rejection": (!accepted).then_some(format!("{:?}", candidate.disposition)),
    });
    retain_json(
        &mut manifest,
        "validation",
        "validation.json",
        &validation,
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    if !accepted {
        manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
        return Err(
            "finite Presenter refused the current owner Face; diagnostic evidence retained".into(),
        );
    }
    retain(
        &mut manifest,
        "transcript",
        "transcript.txt",
        EvidenceKind::ConsoleTranscript,
        "text/plain; charset=utf-8",
        spoken.as_bytes(),
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    let wav = manifest.root().join("speech.wav");
    let mask = spoken_mask_journey::execute_retained_manifestation_mask_with_streaming_espeak(
        "one-body-journey-spoken",
        &execution_id,
        first.presentation.clone(),
        candidate.clone(),
        speech.clone(),
        &wav,
    )
    .map_err(|error| {
        let _ = manifest.finish(EvidenceResult::DiagnosticIncomplete);
        error
    })?;
    let show_id = mask.execution.shown.show.show_id.as_str();
    let transcript = json!({
        "schema": "conduit.journey/speech-transcript@1",
        "source_commit": source_commit,
        "run_id": run_id,
        "body_id": body_id.as_str(),
        "chapter_id": "hear",
        "show_id": show_id,
        "face_revision": first.presentation.revision.to_string(),
        "text": spoken,
        "original_model_output": raw,
    });
    let transcript_bytes = serde_json::to_vec_pretty(&transcript)?;
    retain(
        &mut manifest,
        "speech-transcript",
        "speech-transcript.json",
        EvidenceKind::MachineReadableManifest,
        "application/json",
        &transcript_bytes,
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    retain_json(
        &mut manifest,
        "model-validation",
        "model-validation.json",
        &json!({
            "schema": "conduit.journey/model-validation@1",
            "source_commit": source_commit,
            "run_id": run_id,
            "body_id": body_id.as_str(),
            "show_id": show_id,
            "face_revision": first.presentation.revision.to_string(),
            "provider_id": candidate.provider_identity,
            "model_id": candidate.model_identity,
            "original_output_sha256": format!("{:x}", Sha256::digest(raw.as_bytes())),
            "validated_text_sha256": format!("{:x}", Sha256::digest(spoken.as_bytes())),
            "accepted": true,
        }),
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    declare(
        &mut manifest,
        "speech-wav",
        "speech.wav",
        EvidenceKind::Audio,
        "audio/wav",
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    let last = match face_bytes(&bin, &state_dir).and_then(|bytes| parse_snapshot(&bytes)) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
            return Err(error.into());
        }
    };
    if last != first
        || hash_file(&bin)? != bin_sha256
        || git(&workspace, &["rev-parse", "HEAD"])? != source_commit
        || !git(&workspace, &["status", "--porcelain"])?.is_empty()
    {
        manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
        return Err(
            "source, owner executable, Face, or Host/Boot changed during Presenter and spoken Mask Play"
                .into(),
        );
    }
    retain_json(
        &mut manifest,
        "speech-receipt",
        "speech-receipt.json",
        &json!({
        "schema": "conduit.journey/one-body-spoken-chapter@1",
        "chapter_id": "hear",
        "speech_mode": "llm-assisted",
            "proof_class": model_proof.proof_class,
            "source_commit": source_commit,
            "run_id": run_id,
            "installed_owner_executable": bin,
            "installed_owner_executable_sha256": bin_sha256,
            "owner_snapshot_before_after_equal": true,
            "host_id": first.advertisement.host_id,
            "boot_id": first.advertisement.boot_id,
            "body_id": body_id.as_str(),
            "face_id": first.presentation.identity,
            "face_revision": first.presentation.revision,
            "request_id": request.request_identity,
            "presenter_plan_id": presenter.plan_id,
            "presenter_play_completed": presenter.play_completed,
            "candidate_id": candidate.candidate_identity,
            "provider_id": candidate.provider_identity,
            "model_id": candidate.model_identity,
            "original_model_output_sha256": format!("{:x}", Sha256::digest(raw.as_bytes())),
        "accepted_wording_sha256": format!("{:x}", Sha256::digest(spoken.as_bytes())),
        "transcript_id": "speech-transcript",
        "transcript_sha256": format!("{:x}", Sha256::digest(&transcript_bytes)),
        "validation_id": "model-validation",
        "voice_id": args.speech_voice,
            "speech_provider_sha256": speech.provider_sha256,
            "mask_plan": mask.execution.plan,
            "acknowledged_show": mask.execution.shown,
            "wav_artifact": mask.artifact,
            "playback_observed": false,
            "human_hearing_observed": false,
        }),
        &run_id,
        &first,
        Some(&model_proof.proof_class),
    )?;
    let result = if model_proof.proof_class == "live-local-model" {
        EvidenceResult::Complete
    } else {
        EvidenceResult::DiagnosticIncomplete
    };
    manifest.finish(result)?;
    verify(&VerificationRequest {
        root: manifest.root().to_path_buf(),
        commit: source_commit,
        result: if result == EvidenceResult::Complete {
            ExpectedEvidenceResult::Complete
        } else {
            ExpectedEvidenceResult::DiagnosticIncomplete
        },
        proof_id: PROOF_ID.into(),
        suite_id: SUITE_ID.into(),
    })?;
    println!("one current Body Face produced finite Presenter wording and an acknowledged spoken WAV ({}) at {}", model_proof.proof_class, manifest.root().display());
    Ok(())
}

fn accepted_wording(
    request: &GenerativePresenterRequest,
    candidate: &conduit_presentation::GeneratedManifestationCandidate,
) -> Result<String, String> {
    request
        .validate_candidate(candidate)
        .map_err(|error| format!("candidate validation failed: {error:?}"))?;
    let proposal = candidate
        .wording_proposal
        .as_ref()
        .ok_or("finite Presenter omitted wording proposal")?;
    let spoken = proposal
        .render_exact(&request.semantic_data.presentation)
        .map_err(|error| {
            format!("finite wording was not grounded in the current Face: {error:?}")
        })?;
    let [segment] = candidate.content.as_slice() else {
        return Err("finite Presenter did not return exactly one spoken segment".into());
    };
    if segment.role != GeneratedContentRole::Speech || segment.bytes != spoken.as_bytes() {
        return Err("accepted wording does not equal the retained spoken segment".into());
    }
    Ok(spoken)
}
