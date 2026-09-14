//! Recorded speech through address-gated House generation in one ordinary Plan/Play.

use crate::hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal};
use crate::hosted_microphone::{AlsaMicrophoneAdapter, MicrophoneCaptureReceipt};
use crate::hosted_speech_recognition::WhisperSpeechAdapter;
use crate::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub use crate::recorded_house_receipt::{MicrophoneHouseProofReceipt, RecordedHouseProofReceipt};

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
    )?
    .0)
}

pub fn run_microphone(
    config: StdHostConfig,
    composition: StdHostComposition,
    local_model: Box<dyn HostedLocalModelAdapter>,
    whisper: WhisperSpeechAdapter,
    microphone: AlsaMicrophoneAdapter,
) -> Result<MicrophoneHouseProofReceipt, Box<dyn std::error::Error>> {
    let (house, microphone) = run_with_source(
        config,
        composition,
        local_model,
        whisper,
        HouseAudioSource::Microphone(Box::new(microphone)),
    )?;
    let microphone = microphone.ok_or("microphone House Play omitted capture receipt")?;
    Ok(MicrophoneHouseProofReceipt {
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
    })
}

fn run_with_source(
    config: StdHostConfig,
    composition: StdHostComposition,
    local_model: Box<dyn HostedLocalModelAdapter>,
    whisper: WhisperSpeechAdapter,
    source: HouseAudioSource,
) -> Result<(RecordedHouseProofReceipt, Option<MicrophoneCaptureReceipt>), Box<dyn std::error::Error>>
{
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

    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_microphone_clip_catalogs(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profiles)?;
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
    let source = format!(
        "{}\n{}\nform recorded-house-proof {{\n audio: {}\n recognize: speech/recognize-clip\n recognized: speech/recognition-to-text\n addresses: {}\n addressed: addressed-utterance\n context: {}\n house: house-conversation\n sink: {}\n {}\n recognize.result > recognized.result\n recognized.text > addressed.recognized\n addresses.value > addressed.addresses\n addressed.detection > house.detection\n context.value > house.context\n house.response > sink.value\n}}\n",
        include_str!("../../../forms/addressed-utterance/main.conduit"),
        include_str!("../../../forms/house-conversation/main.conduit"),
        audio_kind,
        crate::installed_std::test_local_model_io::HOUSE_ADDRESSES_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_CONTEXT_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_TEXT_SINK_KIND,
        audio_wiring,
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
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &advertisements)?;
    let authority_grants = if microphone_source {
        vec![host.microphone_authority_grant("grant/microphone-house-proof")?]
    } else {
        Vec::new()
    };
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
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
    let microphone = report.microphone.into_iter().next();
    Ok((
        RecordedHouseProofReceipt {
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
    ))
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_ai::{
        LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelIdentity,
        LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
    };
    use conduit_core::{BootId, HostId, OfferGeneration, PlannedGear};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct FakeModel {
        offer: LocalModelOffer,
        calls: Arc<AtomicUsize>,
    }

    impl HostedLocalModelAdapter for FakeModel {
        fn offer(&self) -> &LocalModelOffer {
            &self.offer
        }

        fn execute(
            &mut self,
            placement: &PlannedGear,
            input: &[u8],
            output: &mut Vec<u8>,
        ) -> LocalModelAdapterTerminal {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let contract = conduit_ai::llm_contract(placement.kind_id.as_str()).unwrap();
            let payload = b"The upstairs temperature is 21 C.".to_vec();
            let result = conduit_ai::ModelDerivedResult {
                provenance: conduit_ai::ModelResultProvenance::ModelDerived,
                payload_kind: contract.result_payload_kind.as_str().into(),
                payload: payload.clone(),
                implementation_identity: "fixture/model".into(),
                request_identity: "request/fixture".into(),
                run_identity: "run/fixture".into(),
                confidence: None,
                disposition: conduit_ai::ModelResultDisposition::Produced,
                determinism: self.offer.determinism,
                accounting: conduit_ai::ModelWorkAccounting {
                    input_bytes: input.len() as u64,
                    context_items: 1,
                    output_bytes: payload.len() as u64,
                    work_units: 1,
                    history_items: 0,
                },
            };
            output.clear();
            output.extend_from_slice(&serde_json::to_vec(&result).unwrap());
            LocalModelAdapterTerminal::Produced
        }
    }

    fn local_offer() -> LocalModelOffer {
        LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: "fixture".into(),
                runtime_version: "1".into(),
                runtime_build_identity: "fixture/build-1".into(),
                model_name: "fixture-model".into(),
                model_content_identity: "sha256-fixture".into(),
                architecture: "fixture".into(),
                parameter_profile: "tiny".into(),
                quantization: "exact".into(),
            },
            limits: LocalModelLimits {
                work: LlmWorkBounds {
                    maximum_input_bytes: 4_096,
                    maximum_context_items: 1,
                    maximum_output_bytes: 4_096,
                    maximum_work_units: 4_096,
                    maximum_history_items: 0,
                },
                model_bytes: 1,
                admitted_memory_mib: 1,
                compute: conduit_ai::LocalModelComputeNeed {
                    minimum_lanes: 1,
                    preferred_lanes: 1,
                    maximum_lanes: 1,
                    minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
                },
                maximum_in_flight: 1,
                maximum_queue_items: 4,
                maximum_queue_bytes: 16_384,
                cancellation_supported: true,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: vec![LocalModelKindProfile::Generate],
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        }
    }

    fn whisper_fixture(
        transcript: &str,
        suffix: &str,
    ) -> (
        crate::hosted_speech_recognition::WhisperSpeechAdapter,
        std::path::PathBuf,
    ) {
        let root = std::env::temp_dir().join(format!(
            "conduit-recorded-house-proof-{}-{suffix}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let executable = root.join("whisper-cli");
        let model = root.join("whisper-model.bin");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nout=\nwhile [ $# -gt 0 ]; do if [ \"$1\" = --output-file ]; then out=$2; shift 2; else shift; fi; done\nprintf '{}\\n' > \"${{out}}.txt\"\n",
                transcript
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&model, b"whisper model fixture").unwrap();
        let adapter =
            crate::hosted_speech_recognition::WhisperDiscovery::inspect(&executable, &model)
                .unwrap()
                .initialize(crate::hosted_speech_recognition::WhisperLimits {
                    maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
                    maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
                    threads: 1,
                    timeout: Duration::from_secs(2),
                })
                .unwrap();
        (adapter, root)
    }

    #[test]
    fn addressed_recorded_clip_reaches_the_model_in_one_plan_play() {
        let (whisper, root) = whisper_fixture(
            "Rosehip House, what is the temperature upstairs?",
            "addressed",
        );
        let receipt = run(
            StdHostConfig {
                host_id: HostId::from("recorded-house-host"),
                boot_id: BootId::from("recorded-house-boot"),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::minimal(),
            Box::new(FakeModel {
                offer: local_offer(),
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            whisper,
            crate::installed_std::test_local_model_io::recorded_house_audio_clip().unwrap(),
        )
        .unwrap();
        assert_eq!(receipt.local_model_invocations, 1);
        assert!(receipt.recognized_text_sha256.is_some());
        assert_eq!(receipt.response_bytes, 33);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unaddressed_recorded_clip_never_invokes_the_model() {
        let (whisper, root) = whisper_fixture("What is the temperature upstairs?", "unaddressed");
        let calls = Arc::new(AtomicUsize::new(0));
        let result = run(
            StdHostConfig {
                host_id: HostId::from("unaddressed-recorded-house-host"),
                boot_id: BootId::from("unaddressed-recorded-house-boot"),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::minimal(),
            Box::new(FakeModel {
                offer: local_offer(),
                calls: Arc::clone(&calls),
            }),
            whisper,
            crate::installed_std::test_local_model_io::recorded_house_audio_clip().unwrap(),
        );
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
