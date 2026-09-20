//! Complete attended Voice enactment of the canonical Home journey.

use super::{
    confirm_intelligible_after_playback, fresh_voice_boot_id, retain_physical_evidence,
    HomeVoiceArgs,
};
use crate::cli::GlobalOpts;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_home_model::{linearize, HomeAction, HomeJourney, HomeModel};
use conduit_std_host::hosted_audio::{discover_alsa_playback, HostedPlaybackSelection};
use conduit_std_host::hosted_microphone::{AlsaMicrophoneDiscovery, MicrophoneLimits};
use conduit_std_host::hosted_speech::{PiperDiscovery, PiperLimits};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use std::time::Duration;

const FORMS: [&str; 4] = ["Hello", "Text Lab", "Clock", "Count"];
const TURN_GUIDANCE: [&str; 6] = [
    "Say `forms`.",
    "Say `inspect hello`.",
    "Say `open prompt`.",
    "Say `run hello`.",
    "Say `open patchbay`.",
    "Say `home`.",
];

struct TurnEvidence {
    value: serde_json::Value,
    output_boot_id: String,
    plan_id: String,
    play_id: String,
}

pub(super) fn run(
    request: &HomeVoiceArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if opts.json {
            println!(
                "{{\"schema\":\"conduit.home/voice-front@1\",\"complete_journey\":true,\"dry_run\":true}}"
            );
        } else if !opts.quiet {
            println!("would capture and speak six bounded turns of the complete Home journey");
        }
        return Ok(());
    }
    if !request.authorize_capture || !request.authorize_output {
        return Err("Home Voice requires --authorize-capture and --authorize-output".into());
    }
    if request.evidence_root.is_none() {
        return Err("complete Home Voice journey requires --evidence-root".into());
    }

    let mut home = HomeModel::new();
    let mut journey = HomeJourney::arrived(&home)
        .map_err(|error| format!("initialize Home journey: {error:?}"))?;
    let mut turns = Vec::with_capacity(TURN_GUIDANCE.len());
    for (index, guidance) in TURN_GUIDANCE.iter().enumerate() {
        eprintln!(
            "Home Voice journey turn {}/{}: {guidance}",
            index + 1,
            TURN_GUIDANCE.len()
        );
        turns.push(run_turn(request, &mut home, &mut journey, index)?);
    }
    if !journey.is_complete() {
        return Err(
            "physical Voice journey ended before the exact shared journey completed".into(),
        );
    }
    let final_turn = turns.last().ok_or("Home Voice retained no journey turns")?;
    let report = serde_json::json!({
        "schema": "conduit.home/voice-front@1",
        "journey_step_ids": conduit_home_model::JOURNEY_STEP_IDS,
        "observed_journey_step_ids": journey.observed_step_ids(),
        "physical_scope": "complete attended multi-turn Home journey through microphone recognition and semantic aural playback",
        "turns": turns.iter().map(|turn| &turn.value).collect::<Vec<_>>(),
        "physical_acceptance": {
            "microphone": true,
            "playback": true,
            "intelligible": true,
            "safe_volume": request.safe_volume,
            "attended": request.attended,
        },
    });
    retain_physical_evidence(
        request.evidence_root.as_ref().expect("checked above"),
        &report,
        &final_turn.plan_id,
        &final_turn.play_id,
        &final_turn.output_boot_id,
    )?;
    if opts.json {
        println!("{}", serde_json::to_string(&report)?);
    } else if !opts.quiet {
        println!(
            "HOME VOICE JOURNEY COMPLETE: {}",
            serde_json::to_string_pretty(&report)?
        );
    }
    Ok(())
}

fn run_turn(
    request: &HomeVoiceArgs,
    home: &mut HomeModel,
    journey: &mut HomeJourney,
    turn: usize,
) -> Result<TurnEvidence, Box<dyn std::error::Error>> {
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
    let input_boot_id = fresh_voice_boot_id(&format!("input-{}", turn + 1));
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
    if transcript.trim().is_empty() {
        return Err("Home Voice recognized no committed command".into());
    }
    let action = home.submit_text(transcript.trim(), &FORMS);
    journey
        .observe_action(home, &action)
        .map_err(|error| format!("Home Voice journey command was out of order: {error:?}"))?;
    let spoken = realize_and_speak(home, journey, &action)?;

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
    let output_boot_id = fresh_voice_boot_id(&format!("output-{}", turn + 1));
    let playback = HostedPlaybackSelection::from_observation(
        playback.clone(),
        BootId::from(output_boot_id.as_str()),
        OfferGeneration(1),
    );
    let piper = PiperDiscovery::inspect(
        &request.piper_executable,
        &request.piper_model,
        &request.piper_config,
        request.piper_library_path.clone(),
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
        "grant/home-voice-journey-playback",
    )?;
    let synthesis = speech
        .run
        .speech_synthesis
        .first()
        .ok_or("Home Voice retained no synthesis receipt")?;
    confirm_intelligible_after_playback()?;
    let plan_id = synthesis.plan_id.as_str().to_owned();
    let play_id = synthesis.active_play_id.as_str().to_owned();
    Ok(TurnEvidence {
        value: serde_json::json!({
            "turn": turn + 1,
            "recognized_text_sha256": recognition.recognized_text_sha256,
            "recognized_text_bytes": recognition.recognized_text_bytes,
            "input_boot_id": input_boot_id,
            "recognition_plan_id": recognition.plan_id,
            "recognition_play_id": recognition.play_id,
            "home_action": action_name(&action),
            "observed_journey_step_ids": journey.observed_step_ids(),
            "spoken_text_sha256": synthesis.text_sha256,
            "pcm_sha256": synthesis.pcm_sha256,
            "frames": synthesis.frames,
            "blocks": synthesis.blocks,
            "output_boot_id": output_boot_id,
            "synthesis_plan_id": plan_id,
            "synthesis_play_id": play_id,
            "alsa_target": speech.alsa_target,
            "intelligible_after_playback": true,
        }),
        output_boot_id,
        plan_id,
        play_id,
    })
}

fn realize_and_speak(
    home: &HomeModel,
    journey: &mut HomeJourney,
    action: &HomeAction,
) -> Result<String, Box<dyn std::error::Error>> {
    match action {
        HomeAction::RunForm(index) => {
            let execution = conduit_home_native::execute_installed_form(*index)?;
            journey
                .observe_play_completed()
                .map_err(|error| format!("observe completed Form Play: {error:?}"))?;
            Ok(format!(
                "Completed Form {} in Play {}.",
                FORMS[*index], execution.active_play_id
            ))
        }
        HomeAction::OpenPatchbay => {
            let opening = conduit_home_native::open_patchbay_presentation()?;
            journey
                .observe_patchbay_opened()
                .map_err(|error| format!("observe Patchbay opening: {error:?}"))?;
            Ok(format!(
                "Patchbay opened as presentation {}.",
                opening.presentation_id
            ))
        }
        HomeAction::OpenForm(index) => Ok(format!("Selected form {}.", FORMS[*index])),
        HomeAction::Changed | HomeAction::Unchanged => {
            let view = home
                .presentation(2, &FORMS)
                .lower()
                .map_err(|error| format!("Home presentation refusal: {error:?}"))?;
            let aural =
                linearize(&view, None).map_err(|error| format!("Home aural refusal: {error:?}"))?;
            Ok(aural
                .utterances
                .into_iter()
                .map(|utterance| utterance.text)
                .collect::<Vec<_>>()
                .join(". "))
        }
        _ => Err("Home Voice journey received an unsupported action".into()),
    }
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
