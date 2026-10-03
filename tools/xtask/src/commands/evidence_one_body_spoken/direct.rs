//! Mechanical full-Face reading through the ordinary terminal and spoken Masks.
//! Every WAV is a produced audio artifact; no speaker playback is asserted.

use super::{check_current, finish_manifest, LiveContext};
use crate::evidence::{EvidenceKind, EvidenceManifest, EvidenceResult};
use conduit_std_host::{
    hosted_speech_synthesis::EspeakDiscovery,
    spoken_face_mask::{ReaderCommand, SpokenBatchDelivery, SpokenFaceSession, SpokenTurnOutcome},
    spoken_face_stream_execution::execute_real_spoken_batch,
    terminal_face_mask::{TerminalFaceMask, TerminalMaskExecution},
    terminal_mask_execution::HostedTerminalMaskExecution,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::retention::{declare, retain, retain_json};

// Each Play remains below the stream Back's 30-second PCM admission. Twelve
// batches plus common receipts fit the evidence manifest's 64-output bound.
const MAX_BATCHES: usize = 12;
const SEGMENTS_PER_BATCH: usize = 8;
const TEXT_BYTES_PER_SEGMENT: usize = 256;

pub(super) fn run(
    context: &LiveContext<'_>,
    manifest: &mut EvidenceManifest,
    speech: EspeakDiscovery,
) -> Result<(), Box<dyn std::error::Error>> {
    let face = &context.face.presentation;
    let body_id = face
        .basis
        .body_id
        .as_ref()
        .ok_or("owner Face has no Body")?;
    let mut execution = HostedTerminalMaskExecution::new(&context.face.advertisement)
        .map_err(|error| format!("terminal Mask planning refused: {error:?}"))?;
    let terminal_plan = execution.planned_mask().clone();
    let mut mask = TerminalFaceMask::prepare_read_only(face.clone(), 80, 24)
        .map_err(|error| format!("terminal Mask refused Face: {error:?}"))?;
    let mut terminal_output = Vec::new();
    mask.present(&mut execution, &mut terminal_output)
        .map_err(|error| format!("terminal Mask Show failed: {error:?}"))?;
    if terminal_output.len() > 512 * 1024 {
        return Err("terminal Mask output exceeded retained bound".into());
    }
    let show = mask
        .show()
        .cloned()
        .ok_or("terminal Mask did not acknowledge a Show")?;
    execution
        .close_without_input()
        .map_err(|error| format!("terminal Mask close failed: {error:?}"))?;
    retain(
        manifest,
        "terminal-mask-output",
        "terminal-mask-output.txt",
        EvidenceKind::ConsoleTranscript,
        "text/plain; charset=utf-8",
        &terminal_output,
        context.run_id,
        context.face,
        None,
    )?;
    retain_json(
        manifest,
        "terminal-show",
        "terminal-show.json",
        &json!({
            "schema": "conduit.journey/direct-source-show@1",
            "source_commit": context.source_commit,
            "run_id": context.run_id,
            "action_id": context.action_id,
            "body_id": body_id.as_str(),
            "owner_host_id": context.face.advertisement.host_id,
            "owner_boot_id": context.face.advertisement.boot_id,
            "face_id": face.identity,
            "face_revision": face.revision,
            "terminal_plan": terminal_plan,
            "acknowledged_show": show,
            "rendered_frame_sha256": format!("{:x}", Sha256::digest(&terminal_output)),
        }),
        context.run_id,
        context.face,
        None,
    )?;

    let mut reader = SpokenFaceSession::new(face.clone(), show.clone())
        .map_err(|error| format!("spoken Face refused: {error:?}"))?;
    reader
        .command(face, &show, ReaderCommand::ReadAll, 1)
        .map_err(|error| format!("read-all refused: {error:?}"))?;
    let mut completed_turn = None;
    let mut batch_count = 0;
    let mut batch_receipt_ids = Vec::new();
    while let Some(batch) = reader
        .next_batch_with_limits(SEGMENTS_PER_BATCH, TEXT_BYTES_PER_SEGMENT)
        .map_err(|error| format!("spoken Face pressure/refusal: {error:?}"))?
    {
        if batch_count == MAX_BATCHES {
            manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
            return Err("full Face reading exceeded twelve admitted speech batches".into());
        }
        batch_count += 1;
        let stem = format!("direct-batch-{batch_count}");
        let wav_path = manifest.root().join(format!("{stem}.wav"));
        let output = execute_real_spoken_batch(face, &show, &batch, speech.clone(), &wav_path)
            .map_err(|error| format!("real streamed speech Play failed: {error:?}"))?;
        if output.receipt.wav_sha256 != super::hash_file(&wav_path)?
            || output.receipt.wav_bytes != std::fs::metadata(&wav_path)?.len()
        {
            manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
            return Err("streamed WAV differs from the completed speech receipt".into());
        }
        let words = batch
            .segments
            .iter()
            .map(|item| item.segment.text.as_str())
            .collect::<String>();
        let transcript = json!({
            "schema": "conduit.journey/speech-transcript@1",
            "source_commit": context.source_commit,
            "run_id": context.run_id,
            "body_id": body_id.as_str(),
            "chapter_id": "hear",
            "show_id": show.show_id.as_str(),
            "face_revision": face.revision.to_string(),
            "text": words,
        });
        let transcript_bytes = serde_json::to_vec_pretty(&transcript)?;
        retain(
            manifest,
            &format!("{stem}-transcript"),
            &format!("{stem}-transcript.json"),
            EvidenceKind::MachineReadableManifest,
            "application/json",
            &transcript_bytes,
            context.run_id,
            context.face,
            None,
        )?;
        retain_json(
            manifest,
            &format!("{stem}-segments"),
            &format!("{stem}-segments.json"),
            &json!({
                "schema": "conduit.journey/direct-source-segments@1",
                "source_commit": context.source_commit,
                "run_id": context.run_id,
                "action_id": context.action_id,
                "body_id": body_id.as_str(),
                "face_id": face.identity,
                "face_revision": face.revision,
                "source_show_id": batch.source_show_id,
                "stream_identity": batch.stream_identity,
                "source_segments_sha256": batch.source_segments_sha256,
                "segments": batch.segments.iter().map(|item| json!({
                    "sequence": item.segment.sequence,
                    "text": item.segment.text,
                    "text_sha256": item.text_sha256,
                    "commit_reason": format!("{:?}", item.segment.reason),
                    "clause_index": item.clause_index,
                    "clause_provenance": format!("{:?}", item.clause_provenance),
                })).collect::<Vec<_>>(),
            }),
            context.run_id,
            context.face,
            None,
        )?;
        declare(
            manifest,
            &format!("{stem}-wav"),
            &format!("{stem}.wav"),
            EvidenceKind::Audio,
            "audio/wav",
            context.run_id,
            context.face,
            None,
        )?;
        retain_json(
            manifest,
            &format!("{stem}-receipt"),
            &format!("{stem}-receipt.json"),
            &json!({
                "schema": "conduit.journey/direct-speech-batch@1",
                "speech_mode": "direct",
                "source_commit": context.source_commit,
                "run_id": context.run_id,
                "action_id": context.action_id,
                "body_id": body_id.as_str(),
                "chapter_id": "hear",
                "owner_host_id": context.face.advertisement.host_id,
                "owner_boot_id": context.face.advertisement.boot_id,
                "terminal_plan_id": terminal_plan.plan.plan_id,
                "source_show_id": show.show_id.as_str(),
                "face_id": face.identity,
                "face_revision": face.revision,
                "speech_host_id": output.host_id,
                "speech_boot_id": output.boot_id,
                "speech_source_document_id": output.source_document_id,
                "speech_checked_plot_id": output.checked_plot_id,
                "stream_identity": output.receipt.stream_identity,
                "source_segments_sha256": output.receipt.source_segments_sha256,
                "speech_plan_id": output.receipt.speech_plan_id,
                "speech_play_id": output.receipt.speech_play_id,
                "provider_sha256": output.receipt.provider_sha256,
                "wav_sha256": output.receipt.wav_sha256,
                "wav_bytes": output.receipt.wav_bytes,
                "pcm_bytes": output.receipt.pcm_bytes,
                "pcm_blocks": output.receipt.pcm_blocks,
                "transcript_id": format!("{stem}-transcript"),
                "transcript_sha256": format!("{:x}", Sha256::digest(&transcript_bytes)),
                "wav_artifact_id": format!("{stem}-wav"),
                "voice_id": context.voice,
                "playback_observed": false,
                "human_hearing_observed": false,
            }),
            context.run_id,
            context.face,
            None,
        )?;
        batch_receipt_ids.push(format!("{stem}-receipt"));
        if let Some(turn) = reader
            .acknowledge_batch(SpokenBatchDelivery::Completed(output.receipt))
            .map_err(|error| format!("spoken reader refused audio receipt: {error:?}"))?
        {
            completed_turn = Some(turn);
        }
    }
    let turn = completed_turn.ok_or("full Face reading produced no completed audio turn")?;
    if turn.outcome != SpokenTurnOutcome::Completed
        || turn.face_id != face.identity.as_str()
        || turn.face_revision != face.revision
        || turn.show_id != show.show_id.as_str()
        || batch_count == 0
    {
        manifest.finish(EvidenceResult::DiagnosticIncomplete)?;
        return Err("full Face reading did not complete on the exact Show".into());
    }
    check_current(context, manifest)?;
    retain_json(
        manifest,
        "speech-receipt",
        "speech-receipt.json",
        &json!({
            "schema": "conduit.journey/one-body-spoken-chapter@1",
            "chapter_id": "hear",
            "speech_mode": "direct",
            "proof_class": "installed-owner-direct-speech",
            "source_commit": context.source_commit,
            "run_id": context.run_id,
            "action_id": context.action_id,
            "installed_owner_executable": context.bin,
            "installed_owner_executable_sha256": context.bin_sha256,
            "installed_release_source_identity": context.installed_release.release_source_identity,
            "installed_release_bundle_sha256": context.installed_release.release_bundle_sha256,
            "owner_snapshot_before_after_equal": true,
            "owner_host_id": context.face.advertisement.host_id,
            "owner_boot_id": context.face.advertisement.boot_id,
            "body_id": body_id.as_str(),
            "face_id": face.identity,
            "face_revision": face.revision,
            "source_show_id": show.show_id.as_str(),
            "source_mask_kind": "terminal",
            "direct_spoken_mask_show_observed": false,
            "owner_sealed_spoken_mask_route_observed": false,
            "batch_count": batch_count,
            "batch_receipt_ids": batch_receipt_ids,
            "completed_segments": turn.completed_segments,
            "produced_pcm_bytes": turn.produced_pcm_bytes,
            "provider_sha256": turn.provider_sha256,
            "correlation_sha256": turn.correlation_sha256,
            "voice_id": context.voice,
            "playback_observed": false,
            "human_hearing_observed": false,
        }),
        context.run_id,
        context.face,
        None,
    )?;
    finish_manifest(manifest, context.source_commit, EvidenceResult::Complete)?;
    println!(
        "one current Body Face produced {batch_count} completed mechanical speech WAV batch(es) at {}",
        manifest.root().display()
    );
    Ok(())
}
