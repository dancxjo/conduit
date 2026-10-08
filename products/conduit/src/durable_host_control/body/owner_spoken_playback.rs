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

/// The direct Show has already committed its short opening. Read that exact
/// wording through one selected speaker Play instead of serializing the full
/// Face's inspection clauses. Keep the established direct receipt shape so
/// each WAV still names its own Plan, Play, and committed speaker PCM.
pub(super) fn play_direct_opening_wording(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    wording: &str,
    equipment: &AttachedEquipment,
    control: &RunControl,
) -> Result<Value, String> {
    let played = play_accepted_wording(host, face, show, wording, equipment, control)?;
    direct_opening_receipt(
        face.identity.as_str(),
        face.revision,
        show.show_id.as_str(),
        wording,
        &played,
    )
}

fn direct_opening_receipt(
    face_id: &str,
    face_revision: u64,
    show_id: &str,
    wording: &str,
    played: &Value,
) -> Result<Value, String> {
    let segments = played["spoken_segments"]
        .as_array()
        .ok_or("direct spoken Play omitted its source segments")?;
    let revision = face_revision.to_string();
    if played["outcome"] != "completed"
        || played["source_face_id"] != face_id
        || played["source_face_revision_decimal"].as_str() != Some(revision.as_str())
        || played["source_show_id"] != show_id
        || segments.is_empty()
        || segments.iter().any(|segment| segment.as_str().is_none())
        || segments
            .iter()
            .filter_map(Value::as_str)
            .collect::<String>()
            != wording
    {
        return Err("direct spoken Play differs from the acknowledged opening".into());
    }
    let artifact_id = played["wav_artifact_locator"]
        .as_str()
        .and_then(|locator| std::path::Path::new(locator).file_name())
        .and_then(|name| name.to_str())
        .filter(|name| {
            name.len() == 73
                && name.starts_with("play-")
                && name.ends_with(".wav")
                && name[5..69].bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or("direct spoken same-Play WAV locator is invalid")?;
    let completed_segments = segments.len();
    Ok(json!({
        "schema":"conduit.body/selected-speech-terminal@1",
        "outcome":"completed",
        "face_id":face_id,
        "face_revision":face_revision,
        "face_revision_decimal":face_revision.to_string(),
        "source_show_id":show_id,
        "host_id":played["host_id"],
        "boot_id":played["boot_id"],
        "provider_sha256":played["provider_sha256"],
        "completed_segments":completed_segments,
        "produced_pcm_bytes":played["pcm_bytes"],
        "completed_batch_count":1,
        "batches":[{
            "stream_identity":played["stream_identity"],
            "source_segments_sha256":played["source_segments_sha256"],
            "spoken_segments":played["spoken_segments"],
            "plan_id":played["plan_id"],
            "play_id":played["play_id"],
            "provider_sha256":played["provider_sha256"],
            "speaker_blocks_committed":played["speaker_blocks_committed"],
            "speaker_frames_committed":played["speaker_frames_committed"],
            "wav_artifact_id":artifact_id,
            "wav_sha256":played["wav_sha256"],
            "wav_bytes":played["wav_bytes"],
            "pcm_sha256":played["pcm_sha256"],
            "pcm_bytes":played["pcm_bytes"],
            "pcm_blocks":played["pcm_blocks"],
            "outcome":"completed"
        }]
    }))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_receipt_keeps_only_the_acknowledged_opening_and_same_play() {
        let opening = "Groceries. One thing left: buy milk.";
        let played = json!({
            "outcome":"completed",
            "source_face_id":"face/todo",
            "source_face_revision_decimal":"15",
            "source_show_id":"show/todo",
            "spoken_segments":["Groceries. One thing left: ","buy milk."],
            "host_id":"host/owner", "boot_id":"boot/one",
            "stream_identity":"show/todo/accepted-wording",
            "source_segments_sha256":"source-digest",
            "plan_id":"plan/speech", "play_id":"play/speech",
            "provider_sha256":"provider-digest",
            "speaker_blocks_committed":2, "speaker_frames_committed":16,
            "wav_artifact_locator":format!("/artifacts/play-{}.wav", "a".repeat(64)),
            "wav_sha256":"wav-digest", "wav_bytes":108,
            "pcm_sha256":"pcm-digest", "pcm_bytes":64, "pcm_blocks":2,
        });
        let receipt =
            direct_opening_receipt("face/todo", 15, "show/todo", opening, &played).unwrap();
        assert_eq!(receipt["outcome"], "completed");
        assert_eq!(receipt["batches"].as_array().unwrap().len(), 1);
        assert_eq!(receipt["batches"][0]["play_id"], "play/speech");
        assert_eq!(
            receipt["batches"][0]["spoken_segments"],
            played["spoken_segments"]
        );
        assert!(
            direct_opening_receipt("face/todo", 15, "show/todo", "Groceries.", &played).is_err()
        );
        assert!(direct_opening_receipt("face/todo", 16, "show/todo", opening, &played).is_err());
    }
}
