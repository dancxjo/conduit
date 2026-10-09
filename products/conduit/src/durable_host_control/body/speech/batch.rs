//! One acknowledged batch through either explicit speaker or artifact equipment.
use super::super::{AttachedEquipment, MaskShow, Presentation, RunControl, SpeechFailure, StdHost};
use conduit_std_host::{
    spoken_face_mask::{SpokenBatch, SpokenBatchDelivery},
    spoken_face_stream_execution::{
        execute_spoken_batch_on_attached_artifact_host,
        execute_spoken_batch_on_attached_host_with_capture, SpokenPlaybackOutcome,
        SpokenStreamExecutionRefusal,
    },
};
use serde_json::{json, Value};

pub(in crate::durable_host_control) fn play_batch(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    batch: &SpokenBatch,
    equipment: Option<&AttachedEquipment>,
    control: &RunControl,
) -> Result<(Value, SpokenBatchDelivery), SpeechFailure> {
    if let Some(equipment) = equipment {
        let result = execute_spoken_batch_on_attached_host_with_capture(
            face,
            show,
            batch,
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
        let receipt = json!({"stream_identity":result.stream_identity,
            "source_segments_sha256":result.source_segments_sha256,
            "spoken_segments":spoken_segments,
            "plan_id":result.playback_plan_id, "play_id":result.playback_play_id,
            "provider_sha256":result.provider_sha256,
            "speaker_blocks_committed":result.playback.metrics.blocks_committed,
            "speaker_frames_committed":result.playback.metrics.frames_committed,
            "wav_artifact_id":artifact_id, "wav_sha256":capture.wav_sha256,
            "wav_bytes":capture.wav_bytes, "pcm_sha256":capture.pcm_sha256,
            "pcm_bytes":capture.pcm_bytes, "pcm_blocks":capture.pcm_blocks,
            "outcome":outcome});
        Ok((receipt, result.delivery()))
    } else {
        let result = execute_spoken_batch_on_attached_artifact_host(
            face,
            show,
            batch,
            &conduit_language::LanguageRequest::new(
                conduit_language::LanguageId::new("language/english".into())
                    .expect("English mechanical Mask Language"),
                None,
                conduit_language::LanguageVarietyPolicy::LanguageSufficient,
            )
            .expect("explicit mechanical Mask request"),
            control,
            host,
        )
        .map_err(|error| {
            if error == SpokenStreamExecutionRefusal::Cancelled {
                SpeechFailure::Cancelled("selected artifact speech cancelled".into())
            } else {
                SpeechFailure::Refused(format!("selected artifact Play refused: {error:?}"))
            }
        })?;
        let artifact_id = result
            .wav_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| {
                name.len() == 73
                    && name.starts_with("play-")
                    && name.ends_with(".wav")
                    && name[5..69].bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| {
                SpeechFailure::Failed("selected artifact WAV locator is invalid".into())
            })?;
        let bytes = std::fs::read(&result.wav_path)
            .map_err(|error| SpeechFailure::Failed(format!("read completed artifact: {error}")))?;
        use sha2::{Digest, Sha256};
        if bytes.len() as u64 != result.receipt.wav_bytes
            || format!("{:x}", Sha256::digest(&bytes)) != result.receipt.wav_sha256
        {
            return Err(SpeechFailure::Failed(
                "completed artifact changed before its receipt was retained".into(),
            ));
        }
        let pcm = bytes.get(44..).ok_or_else(|| {
            SpeechFailure::Failed("completed artifact omitted its WAV header".into())
        })?;
        let receipt = json!({"output_mode":"wav-artifact", "stream_identity":result.receipt.stream_identity,
            "source_segments_sha256":result.receipt.source_segments_sha256,
            "spoken_segments":batch.segments.iter().map(|segment| segment.segment.text.as_str()).collect::<Vec<_>>(),
            "plan_id":result.receipt.speech_plan_id, "play_id":result.receipt.speech_play_id,
            "provider_sha256":result.receipt.provider_sha256,
            "speaker_blocks_committed":0, "speaker_frames_committed":0,
            "wav_artifact_id":artifact_id, "wav_sha256":result.receipt.wav_sha256,
            "wav_bytes":result.receipt.wav_bytes, "pcm_sha256":format!("{:x}", Sha256::digest(pcm)),
            "pcm_bytes":result.receipt.pcm_bytes, "pcm_blocks":result.receipt.pcm_blocks, "outcome":"completed"});
        Ok((receipt, SpokenBatchDelivery::Completed(result.receipt)))
    }
}
