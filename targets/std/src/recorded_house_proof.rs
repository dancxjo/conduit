//! Recorded speech through address-gated House generation in one ordinary Plan/Play.

use crate::hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal};
use crate::hosted_microphone::{AlsaMicrophoneAdapter, MicrophoneCaptureReceipt};
use crate::hosted_speech_recognition::WhisperSpeechAdapter;
use crate::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub use crate::recorded_house_receipt::{
    MicrophoneHouseProofReceipt, RecordedHouseProofReceipt, SpokenMicrophoneHouseProofReceipt,
};

struct SpokenOutput {
    speech: crate::hosted_speech::PiperSpeechAdapter,
    playback: crate::hosted_audio::HostedPlaybackSelection,
}

struct SpokenReceipt {
    speech_implementation_id: String,
    source_pcm_frames: u32,
    target_pcm_frames: u64,
    playback_resource_pool_id: String,
    playback_blocks_committed: u64,
    playback_frames_committed: u64,
}

struct HouseRunReceipt {
    house: RecordedHouseProofReceipt,
    microphone: Option<MicrophoneCaptureReceipt>,
    spoken: Option<SpokenReceipt>,
}

enum HouseAudioSource {
    Recorded(Vec<u8>),
    Microphone(Box<AlsaMicrophoneAdapter>),
}

struct CapturingModel {
    inner: Box<dyn HostedLocalModelAdapter>,
    capture: Arc<Mutex<ModelCapture>>,
}

#[derive(Default)]
struct ModelCapture {
    invocations: u16,
    response: Option<Vec<u8>>,
}

impl HostedLocalModelAdapter for CapturingModel {
    fn offer(&self) -> &conduit_ai::LocalModelOffer {
        self.inner.offer()
    }

    fn execute(
        &mut self,
        placement: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        let terminal = self.inner.execute(placement, input, output);
        if placement.kind_id.as_str() == conduit_ai::LLM_GENERATE_KIND {
            let Ok(mut capture) = self.capture.lock() else {
                return LocalModelAdapterTerminal::Failed;
            };
            capture.invocations = capture.invocations.saturating_add(1);
            if matches!(
                terminal,
                LocalModelAdapterTerminal::Produced | LocalModelAdapterTerminal::Truncated
            ) {
                if let Ok(result) = serde_json::from_slice::<conduit_ai::ModelDerivedResult>(output)
                {
                    if result.payload_kind == conduit_ai::GENERATED_RESULT_VALUE_KIND {
                        capture.response = Some(result.payload);
                    }
                }
            }
        }
        terminal
    }
}

pub fn run(
    config: StdHostConfig,
    composition: StdHostComposition,
    local_model: Box<dyn HostedLocalModelAdapter>,
    whisper: WhisperSpeechAdapter,
    clip: Vec<u8>,
) -> Result<RecordedHouseProofReceipt, Box<dyn std::error::Error>> {
    conduit_audio::decode_pcm_clip(&clip)
        .map_err(|error| format!("decode admitted House proof clip: {error:?}"))?;
    Ok(run_with_source(
        config,
        composition,
        local_model,
        whisper,
        HouseAudioSource::Recorded(clip),
        None,
    )?
    .house)
}

pub fn run_microphone(
    config: StdHostConfig,
    composition: StdHostComposition,
    local_model: Box<dyn HostedLocalModelAdapter>,
    whisper: WhisperSpeechAdapter,
    microphone: AlsaMicrophoneAdapter,
) -> Result<MicrophoneHouseProofReceipt, Box<dyn std::error::Error>> {
    let run = run_with_source(
        config,
        composition,
        local_model,
        whisper,
        HouseAudioSource::Microphone(Box::new(microphone)),
        None,
    )?;
    let microphone = run
        .microphone
        .ok_or("microphone House Play omitted capture receipt")?;
    Ok(microphone_receipt(run.house, microphone))
}

pub fn run_spoken_microphone(
    config: StdHostConfig,
    composition: StdHostComposition,
    local_model: Box<dyn HostedLocalModelAdapter>,
    whisper: WhisperSpeechAdapter,
    microphone: AlsaMicrophoneAdapter,
    speech: crate::hosted_speech::PiperSpeechAdapter,
    playback: crate::hosted_audio::HostedPlaybackSelection,
) -> Result<SpokenMicrophoneHouseProofReceipt, Box<dyn std::error::Error>> {
    let run = run_with_source(
        config,
        composition,
        local_model,
        whisper,
        HouseAudioSource::Microphone(Box::new(microphone)),
        Some(SpokenOutput { speech, playback }),
    )?;
    let microphone = run
        .microphone
        .ok_or("spoken House Play omitted capture receipt")?;
    let spoken = run
        .spoken
        .ok_or("spoken House Play omitted output receipt")?;
    Ok(SpokenMicrophoneHouseProofReceipt {
        microphone_house: microphone_receipt(run.house, microphone),
        speech_implementation_id: spoken.speech_implementation_id,
        source_pcm_frames: spoken.source_pcm_frames,
        conversion_implementation_id: conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION.into(),
        target_pcm_frames: spoken.target_pcm_frames,
        playback_resource_pool_id: spoken.playback_resource_pool_id,
        playback_blocks_committed: spoken.playback_blocks_committed,
        playback_frames_committed: spoken.playback_frames_committed,
    })
}

fn microphone_receipt(
    house: RecordedHouseProofReceipt,
    microphone: MicrophoneCaptureReceipt,
) -> MicrophoneHouseProofReceipt {
    MicrophoneHouseProofReceipt {
        house,
        microphone_implementation_id: conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION.into(),
        microphone_executable_sha256: microphone.executable_sha256,
        microphone_base_identity: microphone.base_identity,
        microphone_card_id: microphone.card_id,
        microphone_device: microphone.device,
        capture_milliseconds: microphone.capture_milliseconds,
        raw_pcm_sha256: microphone.raw_pcm_sha256,
        raw_pcm_bytes: microphone.raw_pcm_bytes,
        microphone_diagnostic_bytes: microphone.diagnostic_bytes,
    }
}

fn run_with_source(
    config: StdHostConfig,
    composition: StdHostComposition,
    local_model: Box<dyn HostedLocalModelAdapter>,
    whisper: WhisperSpeechAdapter,
    source: HouseAudioSource,
    spoken: Option<SpokenOutput>,
) -> Result<HouseRunReceipt, Box<dyn std::error::Error>> {
    let capture = Arc::new(Mutex::new(ModelCapture::default()));
    let local_model_implementation_id = conduit_ai::LOCAL_MODEL_IMPLEMENTATION.to_string();
    let mut additional = crate::installed_std::test_local_model_io::house_source_offers().to_vec();
    additional.push(crate::installed_std::test_local_model_io::house_text_sink_offer());
    if matches!(&source, HouseAudioSource::Microphone(_)) {
        additional.push(conduit_std_offers::text_literal_offer());
    }
    let mut host = StdHost::new_with_local_model_capabilities(
        config,
        composition,
        Box::new(CapturingModel {
            inner: local_model,
            capture: Arc::clone(&capture),
        }),
        additional,
    )?;
    let microphone_source = matches!(&source, HouseAudioSource::Microphone(_));
    match source {
        HouseAudioSource::Recorded(clip) => host.attach_whisper_clip_proof(whisper, clip)?,
        HouseAudioSource::Microphone(microphone) => {
            host.attach_microphone(*microphone)?;
            host.attach_whisper_clip_recognizer(whisper)?;
        }
    }
    if let Some(spoken) = spoken {
        host.attach_piper_speech_and_playback(spoken.speech, spoken.playback)?;
    }
    let spoken_output = host.speech_synthesis.is_some();

    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_microphone_clip_catalogs(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    conduit_ai::install_model_text_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_house_conversation_catalog(&mut startup, &mut profiles)?;
    crate::installed_std::test_local_model_io::install_house_source_catalog(
        &mut startup,
        &mut profiles,
    );
    crate::installed_std::test_local_model_io::install_house_text_sink_catalog(
        &mut startup,
        &mut profiles,
    );
    let (audio_kind, audio_wiring) = if microphone_source {
        (
            conduit_semantic_catalog::MICROPHONE_CLIP_SOURCE_KIND,
            "\"capture\" > audio.request\n audio.clip > recognize.clip",
        )
    } else {
        (
            crate::installed_std::test_local_model_io::HOUSE_AUDIO_CLIP_SOURCE_KIND,
            "audio.value > recognize.clip",
        )
    };
    let (output_declarations, output_wiring) = if spoken_output {
        (
            format!(
                "synthesize: speech/synthesize(maximum-output-bytes = {})\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\")\n output: audio/play",
                conduit_tongues::MAXIMUM_PCM_BYTES
            ),
            "house.response > synthesize.text\n synthesize.audio > convert.audio\n convert.converted > output.audio",
        )
    } else {
        (
            format!(
                "sink: {}",
                crate::installed_std::test_local_model_io::HOUSE_TEXT_SINK_KIND
            ),
            "house.response > sink.value",
        )
    };
    let source = format!(
        "{}\n{}\nform recorded-house-proof {{\n audio: {}\n recognize: speech/recognize-clip\n recognized: speech/recognition-to-text\n addresses: {}\n addressed: addressed-utterance\n context: {}\n house: house-conversation\n {}\n {}\n recognize.result > recognized.result\n recognized.text > addressed.recognized\n addresses.value > addressed.addresses\n addressed.detection > house.detection\n context.value > house.context\n {}\n}}\n",
        include_str!("../../../forms/addressed-utterance/main.conduit"),
        include_str!("../../../forms/house-conversation/main.conduit"),
        audio_kind,
        crate::installed_std::test_local_model_io::HOUSE_ADDRESSES_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_CONTEXT_SOURCE_KIND,
        output_declarations,
        audio_wiring,
        output_wiring,
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &startup).map_err(|error| {
            format!(
                "recorded House Form check: {} {}",
                error.code, error.message
            )
        })?;
    let expanded = conduit_form::expand_canonical_form(&checked, "recorded-house-proof", &profiles)
        .map_err(|error| format!("recorded House expansion: {} {}", error.code, error.message))?;
    let clip_connection = expanded
        .connections
        .iter()
        .find(|connection| {
            connection.source_gear_id.as_str().ends_with("/audio")
                && connection.sink_gear_id.as_str().ends_with("/recognize")
        })
        .ok_or("expanded recorded House Form omitted the PCM clip Cord")?;
    let mut connection_limits = BTreeMap::new();
    connection_limits.insert(
        (
            clip_connection.source_gear_id.clone(),
            clip_connection.source_port_id.clone(),
            clip_connection.sink_gear_id.clone(),
            clip_connection.sink_port_id.clone(),
        ),
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        },
    );
    if microphone_source {
        let trigger_connection = expanded
            .connections
            .iter()
            .find(|connection| {
                connection.sink_gear_id.as_str().ends_with("/audio")
                    && connection.sink_port_id.as_str() == "request"
            })
            .ok_or("expanded microphone House Form omitted the capture trigger Cord")?;
        connection_limits.insert(
            (
                trigger_connection.source_gear_id.clone(),
                trigger_connection.source_port_id.clone(),
                trigger_connection.sink_gear_id.clone(),
                trigger_connection.sink_port_id.clone(),
            ),
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 16,
            },
        );
    }
    if spoken_output {
        for connection in &expanded.connections {
            let byte_capacity = if connection.source_gear_id.as_str().ends_with("/synthesize")
                && connection.source_port_id.as_str() == "audio"
            {
                conduit_tongues::MAXIMUM_PCM_BYTES
            } else if connection.source_gear_id.as_str().ends_with("/convert")
                && connection.source_port_id.as_str() == "converted"
            {
                conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES
            } else {
                continue;
            };
            connection_limits.insert(
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            );
        }
    }
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &advertisements)?;
    let mut authority_grants = Vec::new();
    if microphone_source {
        authority_grants.push(host.microphone_authority_grant("grant/microphone-house-proof")?);
    }
    if spoken_output {
        authority_grants.push(host.playback_authority_grant("grant/spoken-house-playback")?);
    }
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &advertisements,
        &placements,
        &bases,
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES,
            authority_grants: &authority_grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
    )?;
    let plan = if spoken_output {
        let playback = host
            .playback
            .as_ref()
            .ok_or("spoken House host omitted selected playback")?;
        let realization = playback.realization_advertisement(host.advertisement().host_id.clone());
        let observation = playback.resource_observation(
            host.advertisement().host_id.clone(),
            conduit_core::SignId::from("sign/spoken-house-playback-ready"),
        );
        conduit_planner::seal_exact_plan_with_selected_realizations(
            plan,
            &advertisements,
            &[realization],
            &[observation],
        )?
    } else {
        plan
    };
    let plan_id = plan.plan_id.as_str().to_string();
    let fragment = plan
        .fragments
        .into_iter()
        .next()
        .ok_or("recorded House Plan has no fragment")?;
    let report = host.run_fragment_to(fragment, &mut Vec::new(), &mut ThreadTimer)?;
    if !matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ) {
        return Err("recorded House Play did not complete".into());
    }
    let recognition = report
        .speech_recognition
        .first()
        .ok_or("recorded House Play omitted Whisper receipt")?;
    let captured = capture
        .lock()
        .map_err(|_| "recorded House response capture lock is poisoned")?;
    let response = captured
        .response
        .as_ref()
        .ok_or("recorded House model emitted no response")?;
    let spoken = if spoken_output {
        let synthesis = report
            .speech_synthesis
            .first()
            .ok_or("spoken House Play omitted synthesis receipt")?;
        if synthesis.text_sha256 != sha256(response) {
            return Err("spoken House synthesis input differs from the model response".into());
        }
        let playback = report
            .kernel
            .as_ref()
            .and_then(|kernel| kernel.playback.first())
            .ok_or("spoken House Play omitted playback receipt")?;
        let target_pcm_frames = u64::from(synthesis.frames)
            .checked_mul(48_000)
            .and_then(|frames| frames.checked_add(22_049))
            .map(|frames| frames / 22_050)
            .ok_or("spoken House converted frame count overflowed")?;
        if playback.metrics.frames_committed != target_pcm_frames {
            return Err("spoken House playback frame extent differs from conversion".into());
        }
        Some(SpokenReceipt {
            speech_implementation_id: synthesis.implementation_id.as_str().to_string(),
            source_pcm_frames: synthesis.frames,
            target_pcm_frames,
            playback_resource_pool_id: playback.resource_pool_id.clone(),
            playback_blocks_committed: u64::from(playback.metrics.blocks_committed),
            playback_frames_committed: playback.metrics.frames_committed,
        })
    } else {
        None
    };
    let microphone = report.microphone.into_iter().next();
    Ok(HouseRunReceipt {
        house: RecordedHouseProofReceipt {
            plan_id,
            play_id: recognition.active_play_id.as_str().to_string(),
            whisper_implementation_id: recognition.implementation_id.as_str().to_string(),
            local_model_implementation_id,
            clip_sha256: hex(&recognition.audio_sha256),
            recognized_text_sha256: recognition.text_sha256.clone(),
            recognized_text_bytes: recognition.text_bytes,
            response_sha256: sha256(response),
            response_bytes: u32::try_from(response.len())?,
            local_model_invocations: captured.invocations,
        },
        microphone,
        spoken,
    })
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "recorded_house_proof_tests.rs"]
mod tests;
