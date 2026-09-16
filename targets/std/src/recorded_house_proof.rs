//! Recorded speech through address-gated House generation in one ordinary Plan/Play.

use crate::hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal};
use crate::hosted_microphone::{AlsaMicrophoneAdapter, MicrophoneCaptureReceipt};
use crate::hosted_speech_recognition::{WhisperEvidenceText, WhisperSpeechAdapter};
use crate::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub use crate::recorded_house_receipt::{
    MicrophoneHouseProofReceipt, RecordedHouseProofReceipt, RecordedHouseWavJourneyReceipt,
    SpokenMicrophoneHouseProofReceipt,
};

enum SpokenOutput {
    Playback {
        speech: crate::hosted_speech::PiperSpeechAdapter,
        playback: crate::hosted_audio::HostedPlaybackSelection,
    },
    WavArtifact {
        speech: crate::hosted_speech::PiperSpeechAdapter,
        artifact: crate::hosted_wav_artifact::WavArtifactSelection,
    },
}

enum SpokenReceipt {
    Playback {
        speech_implementation_id: String,
        source_pcm_frames: u32,
        target_pcm_frames: u64,
        playback_resource_pool_id: String,
        playback_blocks_committed: u64,
        playback_frames_committed: u64,
    },
    WavArtifact {
        speech_implementation_id: String,
        source_pcm_frames: u32,
        target_pcm_frames: u64,
        pcm_bytes: u32,
        blocks: u16,
    },
}

struct HouseRunReceipt {
    house: RecordedHouseProofReceipt,
    response: Vec<u8>,
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
        Some(SpokenOutput::Playback { speech, playback }),
    )?;
    let microphone = run
        .microphone
        .ok_or("spoken House Play omitted capture receipt")?;
    let spoken = run
        .spoken
        .ok_or("spoken House Play omitted output receipt")?;
    let SpokenReceipt::Playback {
        speech_implementation_id,
        source_pcm_frames,
        target_pcm_frames,
        playback_resource_pool_id,
        playback_blocks_committed,
        playback_frames_committed,
    } = spoken
    else {
        return Err("spoken microphone House Play returned a WAV artifact receipt".into());
    };
    Ok(SpokenMicrophoneHouseProofReceipt {
        microphone_house: microphone_receipt(run.house, microphone),
        speech_implementation_id,
        source_pcm_frames,
        conversion_implementation_id: conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION.into(),
        target_pcm_frames,
        playback_resource_pool_id,
        playback_blocks_committed,
        playback_frames_committed,
    })
}

pub fn run_recorded_with_wav(
    config: StdHostConfig,
    composition: StdHostComposition,
    local_model: Box<dyn HostedLocalModelAdapter>,
    mut whisper: WhisperSpeechAdapter,
    clip: Vec<u8>,
    speech: crate::hosted_speech::PiperSpeechAdapter,
    artifact: crate::hosted_wav_artifact::WavArtifactSelection,
) -> Result<RecordedHouseWavJourneyReceipt, Box<dyn std::error::Error>> {
    conduit_audio::decode_pcm_clip(&clip)
        .map_err(|error| format!("decode admitted House journey clip: {error:?}"))?;
    let recognized = whisper.enable_evidence_text();
    let run = run_with_source(
        config,
        composition,
        local_model,
        whisper,
        HouseAudioSource::Recorded(clip),
        Some(SpokenOutput::WavArtifact { speech, artifact }),
    )?;
    let recognized_text = evidence_text(&recognized, "Whisper")?;
    let response_text =
        String::from_utf8(run.response.clone()).map_err(|_| "House model response is not UTF-8")?;
    let spoken = run
        .spoken
        .ok_or("recorded House journey omitted output receipt")?;
    let SpokenReceipt::WavArtifact {
        speech_implementation_id,
        source_pcm_frames,
        target_pcm_frames,
        pcm_bytes,
        blocks,
    } = spoken
    else {
        return Err("recorded House journey returned a playback receipt".into());
    };
    Ok(RecordedHouseWavJourneyReceipt {
        house: run.house,
        recognized_text,
        response_text,
        speech_implementation_id,
        source_pcm_frames,
        conversion_implementation_id: conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION.into(),
        target_pcm_frames,
        wav_pcm_bytes: pcm_bytes,
        wav_blocks_written: blocks,
    })
}

fn evidence_text(
    evidence: &WhisperEvidenceText,
    provider: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    String::from_utf8(
        evidence
            .bytes()
            .map_err(|error| format!("{provider} evidence: {error}"))?,
    )
    .map_err(|_| format!("{provider} evidence is not UTF-8").into())
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
        match spoken {
            SpokenOutput::Playback { speech, playback } => {
                host.attach_piper_speech_and_playback(speech, playback)?;
            }
            SpokenOutput::WavArtifact { speech, artifact } => {
                host.attach_piper_speech_and_wav_artifact(speech, artifact)?;
            }
        }
    }
    let spoken_output = host.speech_synthesis.is_some();

    let topology = crate::house_conversation_topology::build(microphone_source, spoken_output)?;
    let executed_form_name = topology.entry.to_string();
    let executed_source = topology.source.to_string();
    let expanded = topology.expanded;
    let executed_source_document_id = expanded.source_document_id.as_str().to_string();
    let executed_checked_form_id = expanded.checked_form_id.as_str().to_string();
    let connection_limits = topology.connection_limits;
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &advertisements)?;
    let mut authority_grants = Vec::new();
    if microphone_source {
        authority_grants.push(host.microphone_authority_grant("grant/microphone-house-proof")?);
    }
    if spoken_output {
        authority_grants.push(if host.playback.is_some() {
            host.playback_authority_grant("grant/spoken-house-playback")?
        } else {
            host.wav_artifact_authority_grant("grant/recorded-house-wav")?
        });
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
    let plan = if host.playback.is_some() {
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
        let target_pcm_frames = u64::from(synthesis.frames)
            .checked_mul(48_000)
            .and_then(|frames| frames.checked_add(22_049))
            .map(|frames| frames / 22_050)
            .ok_or("spoken House converted frame count overflowed")?;
        let speech_implementation_id = synthesis.implementation_id.as_str().to_string();
        if host.playback.is_some() {
            let playback = report
                .kernel
                .as_ref()
                .and_then(|kernel| kernel.playback.first())
                .ok_or("spoken House Play omitted playback receipt")?;
            if playback.metrics.frames_committed != target_pcm_frames {
                return Err("spoken House playback frame extent differs from conversion".into());
            }
            Some(SpokenReceipt::Playback {
                speech_implementation_id,
                source_pcm_frames: synthesis.frames,
                target_pcm_frames,
                playback_resource_pool_id: playback.resource_pool_id.clone(),
                playback_blocks_committed: u64::from(playback.metrics.blocks_committed),
                playback_frames_committed: playback.metrics.frames_committed,
            })
        } else {
            let wav = report
                .kernel
                .as_ref()
                .and_then(|kernel| kernel.wav_artifacts.first())
                .ok_or("spoken House Play omitted WAV artifact receipt")?;
            if !wav.completed || u64::from(wav.frames) != target_pcm_frames {
                return Err("spoken House WAV extent differs from conversion".into());
            }
            Some(SpokenReceipt::WavArtifact {
                speech_implementation_id,
                source_pcm_frames: synthesis.frames,
                target_pcm_frames,
                pcm_bytes: wav.pcm_bytes,
                blocks: wav.blocks,
            })
        }
    } else {
        None
    };
    let microphone = report.microphone.into_iter().next();
    Ok(HouseRunReceipt {
        house: RecordedHouseProofReceipt {
            executed_source_document_id,
            executed_checked_form_id,
            semantic_topology_origin: "checked-reviewed-form-with-proof-adapters".into(),
            executed_form_name,
            executed_source,
            reviewed_source_paths: vec![
                "forms/addressed-utterance/main.conduit".into(),
                "forms/house-conversation/main.conduit".into(),
                "targets/std/proof/recorded-house/main.conduit".into(),
            ],
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
        response: response.clone(),
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
