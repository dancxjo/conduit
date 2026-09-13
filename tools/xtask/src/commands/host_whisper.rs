use crate::cli::GlobalOpts;
use conduit_audio::{
    encode_pcm_clip, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation,
    MAXIMUM_PCM_CLIP_FRAMES, MAXIMUM_PCM_FRAMES_PER_BLOCK,
};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_tongues::SpeechRecognitionDisposition;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

const BYTES_PER_SAMPLE: usize = 2;
const MAXIMUM_RAW_PCM_BYTES: usize = MAXIMUM_PCM_CLIP_FRAMES as usize * BYTES_PER_SAMPLE;

pub(super) struct WhisperProofRequest {
    pub executable: PathBuf,
    pub model: PathBuf,
    pub pcm_s16le_16000_mono: PathBuf,
    pub threads: u8,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
struct WhisperProofReport {
    schema: &'static str,
    proof_class: &'static str,
    dry_run: bool,
    effects_performed: bool,
    executable_sha256: Option<String>,
    model_sha256: Option<String>,
    model_bytes: Option<u64>,
    sample_rate_hz: u32,
    channels: u8,
    sample_representation: &'static str,
    clip_blocks: Option<usize>,
    clip_frames: Option<u32>,
    clip_sha256: Option<String>,
    disposition: Option<&'static str>,
    text_sha256: Option<String>,
    text_bytes: Option<u16>,
    diagnostic_bytes: Option<u16>,
}

pub(super) fn prove(
    request: WhisperProofRequest,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.timeout_seconds == 0 || request.timeout_seconds > 120 {
        return Err("Whisper proof timeout must be between 1 and 120 seconds".into());
    }
    if !(1..=32).contains(&request.threads) {
        return Err("Whisper proof threads must be between 1 and 32".into());
    }
    if opts.dry_run {
        emit(
            WhisperProofReport {
                schema: "conduit.tools/xtask/whisper-recorded-clip-provider-proof@1",
                proof_class: "recorded-audio-provider-not-plan-play",
                dry_run: true,
                effects_performed: false,
                executable_sha256: None,
                model_sha256: None,
                model_bytes: None,
                sample_rate_hz: 16_000,
                channels: 1,
                sample_representation: "signed-16-little-endian",
                clip_blocks: None,
                clip_frames: None,
                clip_sha256: None,
                disposition: None,
                text_sha256: None,
                text_bytes: None,
                diagnostic_bytes: None,
            },
            opts,
        )?;
        return Ok(());
    }

    let raw = read_bounded_pcm(&request.pcm_s16le_16000_mono)?;
    let (clip, blocks, frames) = encode_recorded_pcm(&raw)?;
    let discovery = WhisperDiscovery::inspect(&request.executable, &request.model)?;
    let executable_sha256 = discovery.executable_sha256.clone();
    let model_sha256 = discovery.model_sha256.clone();
    let model_bytes = discovery.model_bytes;
    let mut adapter = discovery.initialize(WhisperLimits {
        maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: request.threads,
        timeout: Duration::from_secs(request.timeout_seconds),
    })?;
    let result = conduit_tongues::decode_speech_recognition_result(
        &adapter.recognize_clip(&clip, || false)?,
    )
    .map_err(|error| format!("decode Whisper recognition result: {error:?}"))?;
    let receipt = adapter
        .take_receipt()
        .ok_or("Whisper provider proof omitted its exact receipt")?;
    if result.audio_sha256 != receipt.audio_sha256 {
        return Err("Whisper result and provider receipt disagree on clip identity".into());
    }
    emit(
        WhisperProofReport {
            schema: "conduit.tools/xtask/whisper-recorded-clip-provider-proof@1",
            proof_class: "recorded-audio-provider-not-plan-play",
            dry_run: false,
            effects_performed: true,
            executable_sha256: Some(executable_sha256),
            model_sha256: Some(model_sha256),
            model_bytes: Some(model_bytes),
            sample_rate_hz: 16_000,
            channels: 1,
            sample_representation: "signed-16-little-endian",
            clip_blocks: Some(blocks),
            clip_frames: Some(frames),
            clip_sha256: Some(hex(&receipt.audio_sha256)),
            disposition: Some(match result.disposition {
                SpeechRecognitionDisposition::Recognized => "recognized",
                SpeechRecognitionDisposition::NoSpeech => "no-speech",
            }),
            text_sha256: receipt.text_sha256,
            text_bytes: Some(receipt.text_bytes),
            diagnostic_bytes: Some(receipt.diagnostic_bytes),
        },
        opts,
    )?;
    Ok(())
}

fn read_bounded_pcm(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err("recorded PCM input is not a regular file".into());
    }
    if metadata.len() == 0 || metadata.len() > MAXIMUM_RAW_PCM_BYTES as u64 {
        return Err("recorded PCM input is empty or exceeds six seconds".into());
    }
    let raw = std::fs::read(path)?;
    if !raw.len().is_multiple_of(BYTES_PER_SAMPLE) {
        return Err("recorded PCM input ends with a partial signed-16 sample".into());
    }
    Ok(raw)
}

fn encode_recorded_pcm(raw: &[u8]) -> Result<(Vec<u8>, usize, u32), Box<dyn std::error::Error>> {
    if raw.is_empty()
        || raw.len() > MAXIMUM_RAW_PCM_BYTES
        || !raw.len().is_multiple_of(BYTES_PER_SAMPLE)
    {
        return Err("recorded PCM input extent is invalid".into());
    }
    let maximum_payload = usize::from(MAXIMUM_PCM_FRAMES_PER_BLOCK) * BYTES_PER_SAMPLE;
    let mut frames = Vec::with_capacity(raw.len().div_ceil(maximum_payload));
    let mut start_frame = 0_u64;
    for payload in raw.chunks(maximum_payload) {
        let frame_count = u16::try_from(payload.len() / BYTES_PER_SAMPLE)?;
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            16_000,
            PcmChannelLayout::Mono,
            frame_count,
            1,
            start_frame,
            false,
        )
        .map_err(|error| format!("build recorded PCM frame: {error:?}"))?;
        frames.push(
            header
                .encode_frame(payload)
                .map_err(|error| format!("encode recorded PCM frame: {error:?}"))?,
        );
        start_frame = start_frame
            .checked_add(u64::from(frame_count))
            .ok_or("frame extent overflow")?;
    }
    let borrowed = frames.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let clip = encode_pcm_clip(&borrowed).map_err(|error| format!("encode PCM clip: {error:?}"))?;
    Ok((clip, frames.len(), start_frame as u32))
}

fn emit(report: WhisperProofReport, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        println!("{}", serde_json::to_string(&report)?);
    } else if !opts.quiet {
        if report.dry_run {
            println!("would inspect and exercise one explicit bounded Whisper recorded clip");
        } else {
            println!(
                "WHISPER RECORDED CLIP PROVIDER PROVED (not Plan/Play): model={} frames={} blocks={} disposition={}",
                report.model_sha256.as_deref().unwrap_or(""),
                report.clip_frames.unwrap_or(0),
                report.clip_blocks.unwrap_or(0),
                report.disposition.unwrap_or("")
            );
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_pcm_is_split_into_one_contiguous_bounded_clip() {
        let raw = vec![1_u8; 5_000 * BYTES_PER_SAMPLE];
        let (encoded, blocks, frames) = encode_recorded_pcm(&raw).unwrap();
        let decoded = conduit_audio::decode_pcm_clip(&encoded).unwrap();
        assert_eq!(blocks, 3);
        assert_eq!(frames, 5_000);
        assert_eq!(decoded.frame_count, 5_000);
        assert_eq!(decoded.blocks[2].header.start_frame, 4_096);
    }

    #[test]
    fn empty_partial_and_oversized_pcm_refuse_before_clip_creation() {
        assert!(encode_recorded_pcm(&[]).is_err());
        assert!(encode_recorded_pcm(&[0]).is_err());
        assert!(encode_recorded_pcm(&vec![0; MAXIMUM_RAW_PCM_BYTES + 2]).is_err());
    }
}
