//! Explicit push-to-talk Home command through ordinary Whisper and Piper Plans.

use crate::cli::GlobalOpts;
use clap::Args;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_home_model::{HomeAction, HomeModel, linearize};
use conduit_std_host::hosted_audio::{HostedPlaybackSelection, discover_alsa_playback};
use conduit_std_host::hosted_microphone::{AlsaMicrophoneDiscovery, MicrophoneLimits};
use conduit_std_host::hosted_speech::{PiperDiscovery, PiperLimits};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const FORMS: [&str; 4] = ["Hello", "Text Lab", "Clock", "Count"];

#[derive(Args, Debug)]
pub(super) struct HomeVoiceArgs {
    #[arg(long)]
    arecord_executable: PathBuf,
    #[arg(long)]
    microphone_card_id: String,
    #[arg(long)]
    microphone_device: u16,
    #[arg(long, default_value_t = 3_000)]
    capture_milliseconds: u32,
    #[arg(long)]
    whisper_executable: PathBuf,
    #[arg(long)]
    whisper_model: PathBuf,
    #[arg(long, default_value_t = 2)]
    whisper_threads: u8,
    #[arg(long)]
    piper_executable: PathBuf,
    #[arg(long)]
    piper_model: PathBuf,
    #[arg(long)]
    piper_config: PathBuf,
    #[arg(long)]
    piper_library_path: Option<PathBuf>,
    #[arg(long)]
    playback_card_id: String,
    #[arg(long)]
    playback_device: u16,
    #[arg(long, default_value_t = 30)]
    timeout_seconds: u64,
    #[arg(long)]
    authorize_capture: bool,
    #[arg(long)]
    authorize_output: bool,
    /// Retain a strict physical Voice face receipt in this new directory.
    #[arg(long)]
    evidence_root: Option<PathBuf>,
    /// Assert that a person is present for this exact capture and playback.
    #[arg(long)]
    attended: bool,
    /// Assert that playback is set to a safe, comfortable physical volume.
    #[arg(long)]
    safe_volume: bool,
}

pub(super) fn run(
    request: HomeVoiceArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate(&request)?;
    if opts.dry_run {
        if opts.json {
            println!("{{\"schema\":\"conduit.home/voice-face@1\",\"dry_run\":true}}");
        } else if !opts.quiet {
            println!("would capture one bounded Home command and speak its semantic presentation");
        }
        return Ok(());
    }
    if !request.authorize_capture || !request.authorize_output {
        return Err("Home Voice requires --authorize-capture and --authorize-output".into());
    }

    let microphone_discovery = AlsaMicrophoneDiscovery::inspect(&request.arecord_executable)?;
    let microphones = microphone_discovery
        .observations
        .iter()
        .filter(|item| {
            item.card_id == request.microphone_card_id && item.device == request.microphone_device
        })
        .cloned()
        .collect::<Vec<_>>();
    let [microphone] = microphones.as_slice() else {
        return Err(format!(
            "selected microphone matched {} current endpoints",
            microphones.len()
        )
        .into());
    };
    let microphone = microphone_discovery.initialize(
        microphone,
        MicrophoneLimits {
            capture_milliseconds: request.capture_milliseconds,
            timeout: Duration::from_secs(request.timeout_seconds),
        },
    )?;
    let whisper = WhisperDiscovery::inspect(&request.whisper_executable, &request.whisper_model)?
        .initialize(WhisperLimits {
        maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: request.whisper_threads,
        timeout: Duration::from_secs(request.timeout_seconds),
    })?;
    let input_boot_id = fresh_voice_boot_id("input");
    let (recognition, transcript) =
        conduit_std_host::microphone_whisper_proof::run_with_transcript(
            StdHostConfig {
                host_id: HostId::from("host/home-voice-input"),
                boot_id: BootId::from(input_boot_id.as_str()),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::reference(),
            microphone,
            whisper,
        )?;

    let (action_name, spoken) = spoken_for_home(transcript.trim())?;

    let playback_matches = discover_alsa_playback()?
        .into_iter()
        .filter(|item| {
            item.card_id == request.playback_card_id && item.device == request.playback_device
        })
        .collect::<Vec<_>>();
    let [playback] = playback_matches.as_slice() else {
        return Err(format!(
            "selected playback matched {} current endpoints",
            playback_matches.len()
        )
        .into());
    };
    let output_boot_id = fresh_voice_boot_id("output");
    let boot_id = BootId::from(output_boot_id.as_str());
    let playback =
        HostedPlaybackSelection::from_observation(playback.clone(), boot_id, OfferGeneration(1));
    let piper = PiperDiscovery::inspect(
        &request.piper_executable,
        &request.piper_model,
        &request.piper_config,
        request.piper_library_path,
    )?;
    let speech = conduit_std_host::piper_plan_play_proof::run_with_playback(
        piper,
        PiperLimits {
            maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
            maximum_frames: conduit_std_offers::PIPER_MAXIMUM_FRAMES,
            maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
            timeout: Duration::from_secs(request.timeout_seconds),
        },
        &spoken,
        playback,
        "grant/home-voice-explicit-playback",
    )?;
    let synthesis = speech
        .run
        .speech_synthesis
        .first()
        .ok_or("Home Voice retained no synthesis receipt")?;

    let intelligible = if request.evidence_root.is_some() {
        confirm_intelligible_after_playback()?
    } else {
        false
    };

    let report = serde_json::json!({
        "schema": "conduit.home/voice-face@1",
        "journey_step_ids": conduit_home_model::JOURNEY_STEP_IDS,
        "physical_scope": "one attended push-to-talk command and its semantic aural playback; deterministic model proof owns the complete shared journey",
        "input": {
            "host_id": "host/home-voice-input",
            "boot_id": input_boot_id,
            "plan_id": recognition.plan_id,
            "play_id": recognition.play_id,
            "recognized_text_sha256": recognition.recognized_text_sha256,
            "recognized_text_bytes": recognition.recognized_text_bytes,
        },
        "home_action": action_name,
        "output": {
            "host_id": "std-piper-playback-proof-host",
            "boot_id": output_boot_id,
            "plan_id": synthesis.plan_id.as_str(),
            "play_id": synthesis.active_play_id.as_str(),
            "spoken_text_sha256": synthesis.text_sha256,
            "pcm_sha256": synthesis.pcm_sha256,
            "frames": synthesis.frames,
            "blocks": synthesis.blocks,
            "alsa_target": speech.alsa_target,
        },
        "physical_acceptance": {
            "microphone": true,
            "playback": true,
            "intelligible": intelligible,
            "safe_volume": request.safe_volume,
            "attended": request.attended,
        },
    });
    if let Some(root) = &request.evidence_root {
        retain_physical_evidence(
            root,
            &report,
            synthesis.plan_id.as_str(),
            synthesis.active_play_id.as_str(),
            &output_boot_id,
        )?;
    }
    if opts.json {
        println!("{}", serde_json::to_string(&report)?);
    } else if !opts.quiet {
        println!(
            "HOME VOICE COMPLETE: {}",
            serde_json::to_string_pretty(&report)?
        );
    }
    Ok(())
}

fn action_name(action: &HomeAction) -> &'static str {
    match action {
        HomeAction::Unchanged => "unchanged",
        HomeAction::Changed => "presentation-changed",
        HomeAction::OpenTour => "open-tour",
        HomeAction::OpenPatchbay => "open-patchbay",
        HomeAction::OpenCreche => "open-creche",
        HomeAction::OpenForm(_) => "open-form",
        HomeAction::RunForm(_) => "run-form",
    }
}

fn spoken_for_home(command: &str) -> Result<(&'static str, String), Box<dyn std::error::Error>> {
    if command.trim().is_empty() {
        return Err("Home Voice recognized no committed command".into());
    }
    let mut home = HomeModel::new();
    let action = home.submit_text(command, &FORMS);
    let action_name = action_name(&action);
    let spoken = if matches!(action, HomeAction::Changed | HomeAction::Unchanged) {
        let view = home
            .presentation(2, &FORMS)
            .lower()
            .map_err(|error| format!("Home presentation refusal: {error:?}"))?;
        let aural =
            linearize(&view, None).map_err(|error| format!("Home aural refusal: {error:?}"))?;
        aural
            .utterances
            .into_iter()
            .map(|utterance| utterance.text)
            .collect::<Vec<_>>()
            .join(". ")
    } else {
        format!("{action_name} is unavailable on this Voice Host.")
    };
    Ok((action_name, spoken))
}

fn validate(request: &HomeVoiceArgs) -> Result<(), Box<dyn std::error::Error>> {
    if request.microphone_card_id.is_empty() || request.microphone_card_id.len() > 64 {
        return Err("microphone card id must contain 1 to 64 bytes".into());
    }
    if request.playback_card_id.is_empty() || request.playback_card_id.len() > 64 {
        return Err("playback card id must contain 1 to 64 bytes".into());
    }
    if request.capture_milliseconds == 0 || request.capture_milliseconds > 6_000 {
        return Err("capture duration must be between 1 and 6000 milliseconds".into());
    }
    if !(1..=32).contains(&request.whisper_threads) || !(1..=120).contains(&request.timeout_seconds)
    {
        return Err("Voice provider bounds are invalid".into());
    }
    if request.evidence_root.is_some() && (!request.attended || !request.safe_volume) {
        return Err("retained Home Voice evidence requires --attended and --safe-volume".into());
    }
    Ok(())
}

fn confirm_intelligible_after_playback() -> Result<bool, Box<dyn std::error::Error>> {
    eprintln!(
        "Playback finished. Type `intelligible` only if the exact response was intelligible, then press Enter:"
    );
    let mut stdin = std::io::stdin().lock();
    confirm_intelligible_from(&mut stdin)
}

fn confirm_intelligible_from(
    input: &mut impl std::io::BufRead,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut confirmation = String::new();
    input.read_line(&mut confirmation)?;
    if confirmation.trim() != "intelligible" {
        return Err(
            "physical Voice evidence refused: attendee did not confirm intelligibility after playback"
                .into(),
        );
    }
    Ok(true)
}

#[derive(Serialize)]
struct PhysicalAcceptance {
    microphone: bool,
    playback: bool,
    intelligible: bool,
    safe_volume: bool,
    attended: bool,
}

#[derive(Serialize)]
struct HomeFaceReceipt {
    schema: &'static str,
    face_id: &'static str,
    proof_class: &'static str,
    step_ids: [&'static str; 8],
    host_id: &'static str,
    boot_id: String,
    plan_id: String,
    active_play_id: String,
    renderer_id: &'static str,
    manifestation_id: String,
    artifact_path: &'static str,
    artifact_sha256: String,
    voice_physical: PhysicalAcceptance,
}

fn retain_physical_evidence(
    root: &Path,
    report: &serde_json::Value,
    plan_id: &str,
    play_id: &str,
    boot_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir(root)?;
    let artifact = serde_json::to_vec_pretty(report)?;
    create_new(&root.join("voice-run.json"), &artifact)?;
    let artifact_sha256 = format!("sha256:{:x}", Sha256::digest(&artifact));
    let receipt = HomeFaceReceipt {
        schema: "conduit.evidence/home-face@1",
        face_id: "voice-physical",
        proof_class: "attended-physical-voice",
        step_ids: conduit_home_model::JOURNEY_STEP_IDS,
        host_id: "std-piper-playback-proof-host",
        boot_id: boot_id.to_owned(),
        plan_id: plan_id.to_owned(),
        active_play_id: play_id.to_owned(),
        renderer_id: "presentation/renderer-aural-piper-alsa@1",
        manifestation_id: format!(
            "manifestation/home/voice/{}",
            &artifact_sha256["sha256:".len()..]
        ),
        artifact_path: "voice-run.json",
        artifact_sha256,
        voice_physical: PhysicalAcceptance {
            microphone: true,
            playback: true,
            intelligible: true,
            safe_volume: true,
            attended: true,
        },
    };
    create_new(
        &root.join("home-face-voice-physical.json"),
        &serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(())
}

fn create_new(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

fn fresh_voice_boot_id(role: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("boot/home-voice-{role}/{nanos:x}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_input_uses_the_same_home_grammar_and_semantic_linearizer() {
        let (action, spoken) = spoken_for_home("help").unwrap();
        assert_eq!(action, "presentation-changed");
        assert!(spoken.contains("Conduit Prompt"));
        assert!(spoken.contains("open tour|patchbay|forms|body|prompt|creche"));
        assert!(spoken.contains("unavailable on this face"));
        assert!(!spoken.contains("pixel"));
    }

    #[test]
    fn unavailable_voice_host_action_is_refused_instead_of_faked() {
        let (action, spoken) = spoken_for_home("open patchbay").unwrap();
        assert_eq!(action, "open-patchbay");
        assert_eq!(spoken, "open-patchbay is unavailable on this Voice Host.");
    }

    #[test]
    fn silence_does_not_become_a_home_command_or_spoken_success() {
        assert_eq!(
            spoken_for_home("  ").unwrap_err().to_string(),
            "Home Voice recognized no committed command"
        );
    }

    #[test]
    fn physical_receipt_binds_the_shared_semantics_without_plaintext() {
        let root = std::env::temp_dir().join(format!(
            "conduit-home-voice-evidence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let report = serde_json::json!({
            "schema":"conduit.home/voice-face@1",
            "journey_step_ids":conduit_home_model::JOURNEY_STEP_IDS,
            "recognized_text_sha256":"sha256:recognized",
            "spoken_text_sha256":"sha256:spoken"
        });
        retain_physical_evidence(&root, &report, "plan/voice", "play/voice", "boot/voice").unwrap();
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("home-face-voice-physical.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            receipt["step_ids"],
            serde_json::json!(conduit_home_model::JOURNEY_STEP_IDS)
        );
        assert_eq!(receipt["voice_physical"]["safe_volume"], true);
        let artifact = std::fs::read_to_string(root.join("voice-run.json")).unwrap();
        assert!(!artifact.contains("transcript"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn intelligibility_is_confirmed_only_after_playback() {
        assert!(confirm_intelligible_from(&mut "intelligible\n".as_bytes()).unwrap());
        assert_eq!(
            confirm_intelligible_from(&mut "not clear\n".as_bytes())
                .unwrap_err()
                .to_string(),
            "physical Voice evidence refused: attendee did not confirm intelligibility after playback"
        );
    }
}
