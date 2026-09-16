//! Explicit push-to-talk Home command through ordinary Whisper and Piper Plans.

use crate::cli::GlobalOpts;
use clap::Args;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_home_model::{linearize, HomeAction, HomeModel};
use conduit_std_host::hosted_audio::{discover_alsa_playback, HostedPlaybackSelection};
use conduit_std_host::hosted_microphone::{AlsaMicrophoneDiscovery, MicrophoneLimits};
use conduit_std_host::hosted_speech::{PiperDiscovery, PiperLimits};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
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
    let (recognition, transcript) =
        conduit_std_host::microphone_whisper_proof::run_with_transcript(
            StdHostConfig {
                host_id: HostId::from("host/home-voice-input"),
                boot_id: BootId::from("boot/home-voice-input"),
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
    let boot_id = BootId::from("boot/home-voice-output");
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

    let report = serde_json::json!({
        "schema": "conduit.home/voice-face@1",
        "input": {
            "plan_id": recognition.plan_id,
            "play_id": recognition.play_id,
            "recognized_text_sha256": recognition.recognized_text_sha256,
            "recognized_text_bytes": recognition.recognized_text_bytes,
        },
        "home_action": action_name,
        "output": {
            "plan_id": synthesis.plan_id.as_str(),
            "play_id": synthesis.active_play_id.as_str(),
            "spoken_text_sha256": synthesis.text_sha256,
            "pcm_sha256": synthesis.pcm_sha256,
            "frames": synthesis.frames,
            "blocks": synthesis.blocks,
            "alsa_target": speech.alsa_target,
        },
    });
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
    Ok(())
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
}
