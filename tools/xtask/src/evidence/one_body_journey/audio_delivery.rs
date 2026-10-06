//! A playable journey WAV must be the output of the claimed listener Play.
//! These checks correlate retained product/QEMU receipts; live acceptance still
//! owns the observation that the producer ran the asserted action.

use super::{
    digest, json_kind, output, read_json, CaptureReceipt, SpeechTranscript, VerifiedOutput,
};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Deserialize)]
struct SpeakerTerminal {
    schema: String,
    outcome: String,
    face_revision: u64,
    source_show_id: String,
    source_show_still_current: bool,
    host_id: String,
    boot_id: String,
    provider_sha256: String,
    batches: Vec<SpeakerBatch>,
}

#[derive(Deserialize)]
struct SpeakerBatch {
    plan_id: String,
    play_id: String,
    outcome: String,
    wav_sha256: String,
    wav_bytes: u64,
    pcm_sha256: String,
    pcm_bytes: u64,
    pcm_blocks: u64,
    speaker_blocks_committed: u64,
    speaker_frames_committed: u64,
    spoken_segments: Vec<String>,
}

#[derive(Deserialize)]
struct GuestAudioProof {
    schema: String,
    status: String,
    boot_id: String,
    plan_id: String,
    active_play_id: String,
    qemu_audio: GuestAudio,
}

#[derive(Deserialize)]
struct GuestAudio {
    sha256: String,
    bytes: u64,
    source: String,
    nonzero_samples: u64,
}

struct SpeakerPcm {
    sha256: String,
    bytes: u64,
    frames: u64,
}

fn speaker_pcm(path: &Path) -> Result<SpeakerPcm, String> {
    let wav = fs::read(path).map_err(|error| error.to_string())?;
    if wav.len() < 48
        || &wav[0..4] != b"RIFF"
        || &wav[8..12] != b"WAVE"
        || &wav[12..16] != b"fmt "
        || u32::from_le_bytes(wav[16..20].try_into().unwrap()) != 16
        || u16::from_le_bytes(wav[20..22].try_into().unwrap()) != 1
        || u16::from_le_bytes(wav[22..24].try_into().unwrap()) != 2
        || u32::from_le_bytes(wav[24..28].try_into().unwrap()) != 48_000
        || u32::from_le_bytes(wav[28..32].try_into().unwrap()) != 192_000
        || u16::from_le_bytes(wav[32..34].try_into().unwrap()) != 4
        || u16::from_le_bytes(wav[34..36].try_into().unwrap()) != 16
        || &wav[36..40] != b"data"
        || u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize != wav.len() - 44
        || (wav.len() - 44) % 4 != 0
    {
        return Err("speaker WAV is not exact PCM16 stereo 48 kHz".into());
    }
    Ok(SpeakerPcm {
        sha256: digest(&wav[44..]),
        bytes: (wav.len() - 44) as u64,
        frames: ((wav.len() - 44) / 4) as u64,
    })
}

pub(super) fn validate(
    root: &Path,
    outputs: &BTreeMap<&str, &VerifiedOutput>,
    capture: &CaptureReceipt,
    words: &SpeechTranscript,
    artifact: &VerifiedOutput,
) -> Result<(), String> {
    let provenance = output(
        outputs,
        capture
            .audio_provenance_id
            .as_deref()
            .ok_or("playable audio lacks the selected speaker or QEMU delivery receipt")?,
    )?;
    json_kind(provenance)?;
    match capture.capture_source.as_str() {
        "speaker-play" => {
            if capture.qemu_boot_id.is_some() {
                return Err("speaker Play carries a guest Boot claim".into());
            }
            let terminal: SpeakerTerminal = read_json(root, provenance)?;
            let pcm = speaker_pcm(&root.join(&artifact.path))?;
            let matching: Vec<_> = terminal
                .batches
                .iter()
                .filter(|batch| Some(batch.play_id.as_str()) == capture.play_id.as_deref())
                .collect();
            if terminal.schema != "conduit.body/selected-speech-terminal@1"
                || terminal.outcome != "completed"
                || !terminal.source_show_still_current
                || terminal.face_revision.to_string() != capture.face_revision
                || Some(terminal.source_show_id.as_str()) != capture.show_id.as_deref()
                || terminal.host_id.is_empty()
                || terminal.boot_id.is_empty()
                || terminal.provider_sha256.len() != 64
                || terminal.batches.is_empty()
                || terminal.batches.len() > 64
                || matching.len() != 1
            {
                return Err("audio is not bound to a completed selected speaker Play".into());
            }
            let batch = matching[0];
            if batch.outcome != "completed"
                || Some(batch.plan_id.as_str()) != capture.plan_id.as_deref()
                || batch.wav_sha256 != artifact.sha256
                || batch.wav_bytes != artifact.bytes
                || batch.pcm_sha256 != pcm.sha256
                || batch.pcm_bytes != pcm.bytes
                || batch.pcm_blocks == 0
                || batch.pcm_blocks != batch.speaker_blocks_committed
                || batch.speaker_frames_committed != pcm.frames
                || batch.spoken_segments.is_empty()
                || batch.spoken_segments.join(" ") != words.text
            {
                return Err("audio WAV differs from the selected speaker Play".into());
            }
        }
        "qemu-audio" => {
            let guest: GuestAudioProof = read_json(root, provenance)?;
            if guest.schema != "conduit.conduitos.opl2-proof/v1"
                || guest.status != "completed"
                || Some(guest.boot_id.as_str()) != capture.qemu_boot_id.as_deref()
                || Some(guest.plan_id.as_str()) != capture.plan_id.as_deref()
                || Some(guest.active_play_id.as_str()) != capture.play_id.as_deref()
                || guest.qemu_audio.source != "same-run-qemu-wav-output"
                || guest.qemu_audio.nonzero_samples == 0
                || guest.qemu_audio.sha256 != artifact.sha256
                || guest.qemu_audio.bytes != artifact.bytes
            {
                return Err("audio WAV differs from the guest run's QEMU output".into());
            }
        }
        _ => return Err("audio has no admitted delivery source".into()),
    }
    Ok(())
}
