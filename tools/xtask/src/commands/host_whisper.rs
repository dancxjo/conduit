use crate::cli::GlobalOpts;
use conduit_audio::{
    encode_pcm_clip, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation,
    MAXIMUM_PCM_CLIP_FRAMES, MAXIMUM_PCM_FRAMES_PER_BLOCK,
};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use serde::Serialize;
use sha2::{Digest, Sha256};
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
    plan_id: Option<String>,
    play_id: Option<String>,
    implementation_id: Option<String>,
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
                schema: "conduit.tools/xtask/whisper-recorded-clip-plan-play-proof@1",
                proof_class: "recorded-audio-ordinary-plan-play",
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
                plan_id: None,
                play_id: None,
                implementation_id: None,
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
    let adapter = discovery.initialize(WhisperLimits {
        maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: request.threads,
        timeout: Duration::from_secs(request.timeout_seconds),
    })?;
    let clip_sha256 = format!("{:x}", Sha256::digest(&clip));
    let receipt = conduit_std_host::whisper_clip_proof::run(
        StdHostConfig {
            host_id: conduit_core::HostId::from("xtask-whisper-recorded-clip-host"),
            boot_id: conduit_core::BootId::from("xtask-whisper-recorded-clip-boot"),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::reference(),
        adapter,
        clip,
    )?;
    if receipt.audio_sha256 != clip_sha256 {
        return Err("Whisper result and provider receipt disagree on clip identity".into());
    }
    emit(
        WhisperProofReport {
            schema: "conduit.tools/xtask/whisper-recorded-clip-plan-play-proof@1",
            proof_class: "recorded-audio-ordinary-plan-play",
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
            clip_sha256: Some(receipt.audio_sha256.clone()),
            disposition: Some(if receipt.text_sha256.is_some() {
                "recognized"
            } else {
                "no-speech"
            }),
            text_sha256: receipt.text_sha256,
            text_bytes: Some(receipt.text_bytes),
            diagnostic_bytes: Some(receipt.diagnostic_bytes),
            plan_id: Some(receipt.plan_id),
            play_id: Some(receipt.play_id),
            implementation_id: Some(receipt.implementation_id),
        },
        opts,
    )?;
    Ok(())
}

pub(super) fn read_bounded_pcm(
    path: &std::path::Path,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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

pub(super) fn encode_recorded_pcm(
    raw: &[u8],
) -> Result<(Vec<u8>, usize, u32), Box<dyn std::error::Error>> {
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
                "WHISPER RECORDED CLIP PLAN/PLAY PROVED: model={} frames={} blocks={} disposition={}",
                report.model_sha256.as_deref().unwrap_or(""),
                report.clip_frames.unwrap_or(0),
                report.clip_blocks.unwrap_or(0),
                report.disposition.unwrap_or("")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

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

    #[cfg(unix)]
    #[test]
    fn supported_entrance_runs_the_supplied_clip_through_plan_play() {
        let root = std::env::temp_dir().join(format!(
            "conduit-xtask-whisper-plan-play-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let executable = root.join("whisper-cli");
        let model = root.join("model.bin");
        let pcm = root.join("recorded.pcm");
        std::fs::write(
            &executable,
            "#!/bin/sh\nout=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = --output-file ]; then out=$2; shift 2; else shift; fi\ndone\nprintf 'Rosehip House, test status\\n' > \"${out}.txt\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(&model, b"bounded model").unwrap();
        std::fs::write(&pcm, vec![0_u8; 5_000 * BYTES_PER_SAMPLE]).unwrap();

        prove(
            WhisperProofRequest {
                executable,
                model,
                pcm_s16le_16000_mono: pcm,
                threads: 2,
                timeout_seconds: 2,
            },
            &GlobalOpts {
                quiet: true,
                ..GlobalOpts::default()
            },
        )
        .unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
