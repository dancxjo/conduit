//! Finite local-model wording for one authenticated installed owner Face.
use super::{
    check_current, finish_manifest,
    retention::{declare, retain, retain_json},
    LiveContext,
};
use crate::evidence::{EvidenceKind, EvidenceManifest, EvidenceResult};
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

pub(super) fn run(
    context: &LiveContext<'_>,
    manifest: &mut EvidenceManifest,
    speech: EspeakDiscovery,
    endpoint: &str,
    model: &str,
    admitted_memory_mib: u32,
    execution_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let first = context.face;
    let source_commit = context.source_commit;
    let run_id = context.run_id;
    let body_id = first
        .presentation
        .basis
        .body_id
        .as_ref()
        .ok_or("owner Face has no Body")?;
    let bin = context.bin;
    let bin_sha256 = context.bin_sha256;
    let request = GenerativePresenterRequest::from_presentation(
        format!(
            "request/journey-spoken/{}",
            &execution_id["journey-spoken-".len()..]
        ),
        finite_face_wording_presenter_policy(),
        first.presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .map_err(|error| format!("cannot prepare current Face Presenter request: {error:?}"))?;
    let discovery = OllamaDiscovery::discover_at(&endpoint, model)?;
    let adapter = discovery.initialize(
        admitted_memory_mib,
        vec![
            LocalModelKindProfile::Generate,
            LocalModelKindProfile::ClassifyFiniteLabels,
            LocalModelKindProfile::ExtractValidatedInfo,
            LocalModelKindProfile::InterpretSignEvidence,
            LocalModelKindProfile::PresentSemanticFront,
        ],
    )?;
    retain_json(
        manifest,
        "presenter-request",
        "presenter-request.json",
        &request,
        run_id,
        first,
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
        manifest,
        "model-proof",
        "model-proof.json",
        &model_proof,
        run_id,
        first,
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
        manifest,
        "candidate",
        "candidate.json",
        candidate,
        run_id,
        first,
        Some(&model_proof.proof_class),
    )?;
    let raw = candidate
        .raw_provider_output
        .as_ref()
        .ok_or("finite Presenter omitted original provider output")?;
    retain_json(
        manifest,
        "original-model-output",
        "original-model-output.json",
        &json!({
            "schema": "conduit.journey/provider-output@1",
            "request_id": request.request_identity,
            "candidate_id": candidate.candidate_identity,
            "output": raw,
            "sha256": format!("{:x}", Sha256::digest(raw.as_bytes())),
        }),
        run_id,
        first,
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
        "owner_host_id": first.advertisement.host_id,
        "owner_boot_id": first.advertisement.boot_id,
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
        manifest,
        "validation",
        "validation.json",
        &validation,
        run_id,
        first,
        Some(&model_proof.proof_class),
    )?;
    if !accepted {
        manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
        return Err(
            "finite Presenter refused the current owner Face; diagnostic evidence retained".into(),
        );
    }
    retain(
        manifest,
        "transcript",
        "transcript.txt",
        EvidenceKind::ConsoleTranscript,
        "text/plain; charset=utf-8",
        spoken.as_bytes(),
        run_id,
        first,
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
        manifest,
        "speech-transcript",
        "speech-transcript.json",
        EvidenceKind::MachineReadableManifest,
        "application/json",
        &transcript_bytes,
        run_id,
        first,
        Some(&model_proof.proof_class),
    )?;
    retain_json(
        manifest,
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
        run_id,
        first,
        Some(&model_proof.proof_class),
    )?;
    declare(
        manifest,
        "speech-wav",
        "speech.wav",
        EvidenceKind::Audio,
        "audio/wav",
        run_id,
        first,
        Some(&model_proof.proof_class),
    )?;
    check_current(&context, manifest)?;
    retain_json(
        manifest,
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
            "owner_host_id": first.advertisement.host_id,
            "owner_boot_id": first.advertisement.boot_id,
            "model_host_id": model_proof.host_id,
            "model_boot_id": model_proof.boot_id,
            "mask_host_id": mask.execution.shown.show.show.host_id,
            "mask_boot_id": mask.execution.shown.show.show.boot_id,
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
        "voice_id": context.voice,
            "speech_provider_sha256": speech.provider_sha256,
            "mask_plan": mask.execution.plan,
            "acknowledged_show": mask.execution.shown,
            "wav_artifact": mask.artifact,
            "playback_observed": false,
            "human_hearing_observed": false,
        }),
        run_id,
        first,
        Some(&model_proof.proof_class),
    )?;
    let result = if model_proof.proof_class == "live-local-model" {
        EvidenceResult::Complete
    } else {
        EvidenceResult::DiagnosticIncomplete
    };
    finish_manifest(manifest, source_commit, result)?;
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
