//! Bounded acknowledged-Face reading through the existing selected audio Plan.
use super::{AttachedEquipment, MaskShow, Presentation, RunControl, SpeechFailure, StdHost};
use conduit_std_host::{
    spoken_face_mask::{ReaderCommand, SpokenFaceSession},
    spoken_face_stream_execution::{
        execute_spoken_batch_on_attached_host_with_capture, SpokenPlaybackOutcome,
        SpokenStreamExecutionRefusal,
    },
};
use serde_json::{json, Value};

#[derive(Clone, Copy)]
pub(super) enum ReadingScope {
    WholeFace,
    RemainingItems,
}
impl ReadingScope {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::WholeFace => "whole-face",
            Self::RemainingItems => "remaining-items",
        }
    }
    pub(super) fn command(self) -> ReaderCommand {
        match self {
            Self::WholeFace => ReaderCommand::ReadAll,
            Self::RemainingItems => ReaderCommand::ReadCurrentItems,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn play_selected(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    equipment: &AttachedEquipment,
    control: &RunControl,
    segments_per_batch: usize,
    maximum_batches: usize,
    scope: ReadingScope,
) -> Result<Value, SpeechFailure> {
    let mut receipts = Vec::with_capacity(maximum_batches);
    let result = play_selected_inner(
        host,
        face,
        show,
        equipment,
        control,
        segments_per_batch,
        maximum_batches,
        scope,
        &mut receipts,
    );
    result.map_err(|cause| {
        if receipts.is_empty() {
            cause
        } else {
            SpeechFailure::Partial {
                cause: Box::new(cause),
                completed_batches: receipts,
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn play_selected_inner(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    equipment: &AttachedEquipment,
    control: &RunControl,
    segments_per_batch: usize,
    maximum_batches: usize,
    scope: ReadingScope,
    receipts: &mut Vec<Value>,
) -> Result<Value, SpeechFailure> {
    if !equipment.matches(host) {
        return Err(SpeechFailure::Refused(
            "selected speech equipment changed before Play".into(),
        ));
    }
    let offered = host.advertisement().clone();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(|error| {
        SpeechFailure::Refused(format!("selected spoken Face refused: {error:?}"))
    })?;
    reader
        .command(face, show, scope.command(), 1)
        .map_err(|error| {
            SpeechFailure::Refused(format!(
                "selected spoken {} refused: {error:?}",
                scope.name()
            ))
        })?;
    let mut terminal_turn = None;
    for _ in 0..maximum_batches {
        if control.stop_requested() {
            return Err(SpeechFailure::Cancelled(
                "selected speech stop requested".into(),
            ));
        }
        let Some(batch) = reader
            .next_batch_with_limits(segments_per_batch, 64)
            .map_err(|error| {
                SpeechFailure::Refused(format!("selected speech batch refused: {error:?}"))
            })?
        else {
            break;
        };
        let result = execute_spoken_batch_on_attached_host_with_capture(
            face,
            show,
            &batch,
            &conduit_language::LanguageRequest::new(
                conduit_language::LanguageId::new("language/english".into())
                    .expect("English mechanical Mask Language"),
                None,
                conduit_language::LanguageVarietyPolicy::LanguageSufficient,
            )
            .expect("explicit mechanical Mask request"),
            &equipment.playback,
            &equipment.authorization,
            control,
            host,
        )
        .map_err(|error| match error {
            SpokenStreamExecutionRefusal::PlaybackPlay { detail, .. } => {
                SpeechFailure::PlayRefused(detail)
            }
            other => SpeechFailure::Refused(format!("selected speaker Play refused: {other:?}")),
        })?;
        let outcome = match &result.outcome {
            SpokenPlaybackOutcome::Completed => "completed",
            SpokenPlaybackOutcome::Cancelled => "cancelled",
            SpokenPlaybackOutcome::Failed => "failed",
        };
        let capture = match (&result.outcome, result.same_play_capture.as_ref()) {
            (SpokenPlaybackOutcome::Completed, Some(capture)) => capture,
            (SpokenPlaybackOutcome::Completed, None) => {
                return Err(SpeechFailure::Failed(
                    "selected speaker Play omitted same-Play WAV capture".into(),
                ))
            }
            (SpokenPlaybackOutcome::Cancelled, _) => {
                return Err(SpeechFailure::Cancelled(
                    "selected speaker Play cancelled".into(),
                ))
            }
            (SpokenPlaybackOutcome::Failed, _) => {
                return Err(SpeechFailure::Failed("selected speaker Play failed".into()))
            }
        };
        // Browser carriers receive only an exact file identity. A local proof
        // reader resolves it beneath this installation's spoken-artifacts root.
        let artifact_id = capture
            .wav_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| {
                name.len() == 73
                    && name.starts_with("play-")
                    && name.ends_with(".wav")
                    && name[5..69].bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| SpeechFailure::Failed("same-Play WAV locator is invalid".into()))?;
        // The admitted reader supplied bounded ordered segments; the playback
        // entrance validated their exact source digest.
        let spoken_segments: Vec<&str> = batch
            .segments
            .iter()
            .map(|segment| segment.segment.text.as_str())
            .collect();
        receipts.push(json!({"stream_identity":result.stream_identity,
            "source_segments_sha256":result.source_segments_sha256,
            "spoken_segments":spoken_segments,
            "plan_id":result.playback_plan_id, "play_id":result.playback_play_id,
            "provider_sha256":result.provider_sha256,
            "speaker_blocks_committed":result.playback.metrics.blocks_committed,
            "speaker_frames_committed":result.playback.metrics.frames_committed,
            "wav_artifact_id":artifact_id, "wav_sha256":capture.wav_sha256,
            "wav_bytes":capture.wav_bytes, "pcm_sha256":capture.pcm_sha256,
            "pcm_bytes":capture.pcm_bytes, "pcm_blocks":capture.pcm_blocks,
            "outcome":outcome}));
        let terminal = reader
            .acknowledge_batch(result.delivery())
            .map_err(|error| {
                SpeechFailure::Failed(format!("selected speaker receipt refused: {error:?}"))
            })?;
        if let Some(terminal) = terminal {
            terminal_turn = Some(terminal);
            break;
        }
    }
    let terminal = terminal_turn.ok_or_else(|| {
        SpeechFailure::Refused(format!(
            "selected Face reading exceeded {maximum_batches} admitted speech batches"
        ))
    })?;
    if receipts.is_empty()
        || terminal.outcome != conduit_std_host::spoken_face_mask::SpokenTurnOutcome::Completed
    {
        return Err(SpeechFailure::Failed(
            "selected speech ended without bounded complete readout".into(),
        ));
    }
    Ok(json!({"schema":"conduit.body/selected-speech-terminal@1",
        "outcome":"completed", "reader_scope":scope.name(), "face_id":face.identity.as_str(),
        "face_revision":face.revision,
        "face_revision_decimal":face.revision.to_string(),
        "source_show_id":show.show_id.as_str(),
        "host_id":offered.host_id.as_str(), "boot_id":offered.boot_id.as_str(),
        "offer_generation":offered.offer_generation.0,
        "provider_sha256":equipment.provider_sha256,
        "selected_resource_pool_id":equipment.playback.pool_id().as_str(),
        "authority_grant_id":equipment.authorization.grant_id(),
        "completed_segments":terminal.completed_segments,
        "produced_pcm_bytes":terminal.produced_pcm_bytes,
        "correlation_sha256":terminal.correlation_sha256,
        "batches":std::mem::take(receipts)}))
}
