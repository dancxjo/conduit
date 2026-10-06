//! Deliver one accepted spoken Mask wording to the installed listener. The
//! selected speaker and retained WAV are siblings in one audio Plan and Play.

use crate::durable_host::selected_speech::AttachedEquipment;
use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::{
    spoken_face_mask::SpokenBatch,
    spoken_face_stream_execution::{
        execute_spoken_batch_on_attached_host_with_capture, SpokenPlaybackOutcome,
    },
    RunControl, StdHost,
};
use serde_json::{json, Value};

pub(super) fn play_accepted_wording(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    wording: &str,
    equipment: &AttachedEquipment,
    control: &RunControl,
) -> Result<Value, String> {
    if !equipment.matches(host) {
        return Err("selected speaker or voice changed before spoken Mask playback".into());
    }
    let batch = SpokenBatch::from_accepted_wording(
        face,
        show,
        wording,
        format!("{}/accepted-wording", show.show_id.as_str()),
    )
    .map_err(|error| format!("commit accepted spoken Mask wording: {error:?}"))?;
    let language = conduit_language::LanguageRequest::new(
        conduit_language::LanguageId::new("language/english".into())
            .map_err(|error| format!("spoken Mask Language: {error:?}"))?,
        None,
        conduit_language::LanguageVarietyPolicy::LanguageSufficient,
    )
    .map_err(|error| format!("spoken Mask Language: {error:?}"))?;
    let played = execute_spoken_batch_on_attached_host_with_capture(
        face,
        show,
        &batch,
        &language,
        &equipment.playback,
        &equipment.authorization,
        control,
        host,
    )
    .map_err(|error| format!("selected spoken Mask speaker Play: {error:?}"))?;
    if played.outcome != SpokenPlaybackOutcome::Completed {
        return Err(format!(
            "selected spoken Mask speaker Play ended {:?}",
            played.outcome
        ));
    }
    let capture = played
        .same_play_capture
        .ok_or("selected spoken Mask speaker Play omitted same-Play WAV")?;
    if u32::from(capture.pcm_blocks) != played.playback.metrics.blocks_committed
        || u64::from(capture.pcm_bytes) / 4 != played.playback.metrics.frames_committed
    {
        return Err("selected spoken Mask WAV differs from committed speaker PCM".into());
    }
    Ok(json!({
        "schema":"conduit.body/owner-spoken-speaker-play@1",
        "source_show_id":show.show_id.as_str(),
        "source_face_id":face.identity.as_str(),
        "source_face_revision_decimal":face.revision.to_string(),
        "stream_identity":batch.stream_identity,
        "source_segments_sha256":batch.source_segments_sha256,
        "spoken_segments":batch.segments.iter().map(|item| item.segment.text.as_str()).collect::<Vec<_>>(),
        "host_id":played.host_id,
        "boot_id":played.boot_id,
        "plan_id":played.playback_plan_id,
        "play_id":played.playback_play_id,
        "provider_sha256":played.provider_sha256,
        "speaker_blocks_committed":played.playback.metrics.blocks_committed,
        "speaker_frames_committed":played.playback.metrics.frames_committed,
        "wav_artifact_locator":capture.wav_path,
        "wav_sha256":capture.wav_sha256,
        "wav_bytes":capture.wav_bytes,
        "pcm_sha256":capture.pcm_sha256,
        "pcm_bytes":capture.pcm_bytes,
        "pcm_blocks":capture.pcm_blocks,
        "outcome":"completed",
    }))
}
