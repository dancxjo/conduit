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
    let adapter = crate::hosted_speech_recognition::WhisperDiscovery::inspect(&executable, &model)
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

#[test]
fn addressed_microphone_response_is_spoken_and_committed_in_the_same_play() {
    use crate::hosted_audio::{
        AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection,
    };
    use crate::hosted_microphone::{AlsaMicrophoneDiscovery, MicrophoneLimits};
    use crate::hosted_speech::{PiperDiscovery, PiperLimits};

    let root = std::env::temp_dir().join(format!(
        "conduit-spoken-microphone-house-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let arecord = root.join("arecord");
    fs::write(
        &arecord,
        "#!/bin/sh\nif [ \"$1\" = -l ]; then printf 'card 1: DSP [SOF DSP], device 7: DMIC [DMIC]\\n'; exit 0; fi\ndd if=/dev/zero bs=320 count=1 2>/dev/null\n",
    )
    .unwrap();
    fs::set_permissions(&arecord, fs::Permissions::from_mode(0o700)).unwrap();
    let discovery = AlsaMicrophoneDiscovery::inspect(&arecord).unwrap();
    let microphone = discovery
        .clone()
        .initialize(
            &discovery.observations[0],
            MicrophoneLimits {
                capture_milliseconds: 10,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    let (whisper, whisper_root) = whisper_fixture("Rosehip House, what is upstairs?", "spoken");

    let piper_executable = root.join("piper");
    let piper_model = root.join("voice.onnx");
    let piper_config = root.join("voice.onnx.json");
    let piper_invoked = root.join("piper-invoked");
    fs::write(
        &piper_executable,
        format!(
            "#!/bin/sh\ncat >/dev/null\ntouch '{}'\ndd if=/dev/zero bs=1 count=678 2>/dev/null\n",
            piper_invoked.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&piper_executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&piper_model, b"bounded piper model").unwrap();
    fs::write(&piper_config, br#"{"audio":{"sample_rate":22050}}"#).unwrap();
    let speech = PiperDiscovery::inspect(&piper_executable, &piper_model, &piper_config, None)
        .unwrap()
        .initialize(PiperLimits {
            maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
            maximum_frames: conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2),
            maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
            timeout: Duration::from_secs(2),
        })
        .unwrap();
    let playback = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 0,
            card_id: "FIXTURE".into(),
            card_name: "Deterministic fixture".into(),
            device: 0,
            device_name: "Finite PCM sink".into(),
            base_identity: "fixture-base".into(),
        },
        BootId::from("spoken-house-boot"),
        OfferGeneration(1),
        FakePlaybackBehavior::Success,
    );
    let receipt = run_spoken_microphone(
        StdHostConfig {
            host_id: HostId::from("spoken-house-host"),
            boot_id: BootId::from("spoken-house-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(FakeModel {
            offer: local_offer(),
            calls: Arc::new(AtomicUsize::new(0)),
        }),
        whisper,
        microphone,
        speech,
        playback,
    )
    .unwrap();
    assert_eq!(receipt.microphone_house.house.local_model_invocations, 1);
    assert_eq!(receipt.source_pcm_frames, 339);
    assert_eq!(receipt.target_pcm_frames, 738);
    assert_eq!(receipt.playback_frames_committed, 738);
    assert_eq!(receipt.playback_blocks_committed, 14);
    assert!(piper_invoked.is_file());

    fs::remove_file(&piper_invoked).unwrap();
    let microphone_discovery = AlsaMicrophoneDiscovery::inspect(&arecord).unwrap();
    let microphone = microphone_discovery
        .clone()
        .initialize(
            &microphone_discovery.observations[0],
            MicrophoneLimits {
                capture_milliseconds: 10,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    let (whisper, unaddressed_root) = whisper_fixture("What is upstairs?", "spoken-unaddressed");
    let speech = PiperDiscovery::inspect(&piper_executable, &piper_model, &piper_config, None)
        .unwrap()
        .initialize(PiperLimits {
            maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
            maximum_frames: conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2),
            maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
            timeout: Duration::from_secs(2),
        })
        .unwrap();
    let playback = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 0,
            card_id: "FIXTURE".into(),
            card_name: "Deterministic fixture".into(),
            device: 0,
            device_name: "Finite PCM sink".into(),
            base_identity: "fixture-base".into(),
        },
        BootId::from("spoken-house-unaddressed-boot"),
        OfferGeneration(1),
        FakePlaybackBehavior::Success,
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let result = run_spoken_microphone(
        StdHostConfig {
            host_id: HostId::from("spoken-house-unaddressed-host"),
            boot_id: BootId::from("spoken-house-unaddressed-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(FakeModel {
            offer: local_offer(),
            calls: Arc::clone(&calls),
        }),
        whisper,
        microphone,
        speech,
        playback,
    );
    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(!piper_invoked.exists());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(whisper_root).unwrap();
    fs::remove_dir_all(unaddressed_root).unwrap();
}

#[test]
fn unchanged_spoken_house_form_plans_across_three_exact_hosts_and_lines() {
    let model = local_offer();
    let host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("host/house-template"),
            boot_id: BootId::from("boot/house-template"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let mut template = host.advertisement().clone();
    template
        .resources
        .extend(crate::hosted_local_model::resource_offers(&model.limits));
    template.resources.extend([
        conduit_core::resource_offer(
            "std/microphone/fixture",
            conduit_std_offers::MICROPHONE_CAPTURE_RESOURCE_CLASS,
            1,
        ),
        conduit_core::resource_offer(
            "std/whisper-process",
            conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
            1,
        ),
        crate::hosted_speech::process_resource_offer(),
        conduit_core::resource_offer(
            "std/audio/alsa/fixture/card-FIXTURE/device-0",
            conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS,
            1,
        ),
    ]);
    template
        .capabilities
        .extend(model.capability_offers().unwrap());
    template.capabilities.extend([
        conduit_std_offers::house_prompt_std_offer(),
        conduit_std_offers::model_result_to_text_std_offer(),
        conduit_std_offers::address_detect_offer(),
        conduit_std_offers::recognition_to_text_std_offer(),
        conduit_std_offers::text_literal_offer(),
        conduit_std_offers::microphone_clip_offer(),
        conduit_std_offers::whisper_clip_speech_offer(),
        conduit_std_offers::piper_speech_offer(),
        conduit_std_offers::audio_convert_pcm_profile_offer(),
        conduit_std_offers::audio_play_alsa_hw_offer(),
    ]);
    template
        .capabilities
        .extend(crate::installed_std::test_local_model_io::house_source_offers());
    template.capabilities.retain(|offer| {
        offer.implementation.implementation_id.as_str()
            != conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
    });
    template.resources.sort();
    template.capabilities.sort_by(|left, right| {
        left.capability_id
            .as_str()
            .cmp(right.capability_id.as_str())
    });

    let exact = crate::distributed_house_plan::exact_distributed_spoken_house_plan(&template)
        .expect("unchanged House Form plans across exact Hosts");
    assert_eq!(exact.plan.fragments.len(), 3);
    assert_eq!(exact.lines.len(), 2);
    assert_eq!(exact.plan.checked_form_id.as_str(), exact.checked_form_id);
    for fragment in &exact.plan.fragments {
        assert!(exact.hosts.iter().any(|host| {
            host.host_id == fragment.host_id
                && host.boot_id == fragment.boot_id
                && host.offer_generation == fragment.offer_generation
        }));
    }
    let remote_connections = exact
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| fragment.connections.iter())
        .filter(|connection| connection.selected_line.is_some())
        .collect::<Vec<_>>();
    assert_eq!(remote_connections.len(), exact.lines.len() * 2);
    for connection in remote_connections {
        let selected = connection.selected_line.as_ref().unwrap();
        assert_eq!(
            selected.binding.base.as_str(),
            crate::distributed_house_plan::REMOTE_BASE
        );
        assert_eq!(
            selected.binding.limits.maximum_payload_bytes,
            connection.byte_capacity
        );
        assert_eq!(
            selected.binding.limits.maximum_in_flight_items,
            connection.item_capacity
        );
    }

    for line in &exact.lines {
        let source = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == line.binding.source.host_id)
            .unwrap();
        let sink = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == line.binding.sink.host_id)
            .unwrap();
        let connection = source
            .connections
            .iter()
            .find(|connection| {
                connection
                    .selected_line
                    .as_ref()
                    .is_some_and(|selected| selected.line_id == line.line_id)
            })
            .unwrap();
        conduit_wire::SessionBinding::from_planned_connection(
            exact.plan.plan_id.clone(),
            source.fragment_id.clone(),
            sink.fragment_id.clone(),
            connection,
        )
        .expect("every distributed House Line admits its exact session contract and limits");
    }

    let mut runtimes = exact
        .hosts
        .iter()
        .map(|host| {
            let fragment = exact
                .plan
                .fragments
                .iter()
                .find(|fragment| fragment.host_id == host.host_id)
                .unwrap();
            crate::InstalledRemoteFragment::prepare(host, fragment, 1)
                .expect("every distributed House fragment prepares in the one std kernel")
        })
        .collect::<Vec<_>>();
    for line in &exact.lines {
        let source_index = exact
            .hosts
            .iter()
            .position(|host| host.host_id == line.binding.source.host_id)
            .unwrap();
        let sink_index = exact
            .hosts
            .iter()
            .position(|host| host.host_id == line.binding.sink.host_id)
            .unwrap();
        assert!(source_index < sink_index);
        let (sources, sinks) = runtimes.split_at_mut(sink_index);
        let source = &mut sources[source_index];
        let sink = &mut sinks[0];
        let source_endpoint = source
            .sessions()
            .iter()
            .find(|session| session.binding().attachment.line_id == line.line_id)
            .unwrap()
            .endpoint;
        let sink_endpoint = sink
            .sessions()
            .iter()
            .find(|session| session.binding().attachment.line_id == line.line_id)
            .unwrap()
            .endpoint;
        crate::remote_cord_sessions::activate_in_process(
            source.sessions_mut().get_mut(source_endpoint).unwrap(),
            sink.sessions_mut().get_mut(sink_endpoint).unwrap(),
        )
        .expect("every exact distributed House Line reaches Ready");
    }

    let capture_fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == exact.hosts[0].host_id)
        .unwrap();
    let mut stale_capture = exact.hosts[0].clone();
    stale_capture.boot_id = BootId::from("boot/house-capture-stale");
    assert_eq!(
        crate::InstalledRemoteFragment::prepare(&stale_capture, capture_fragment, 1)
            .err()
            .unwrap(),
        "remote fragment preparation requires its exact Host and Boot"
    );
    let capture_endpoint = runtimes[0].sessions().iter().next().unwrap().endpoint;
    runtimes[0]
        .fail_remote_line(capture_endpoint, 71)
        .expect("exact capture Line loss is retained before cancellation");
    assert!(runtimes[0]
        .next_egress(capture_endpoint)
        .unwrap_err()
        .contains("Cancelled"));
    runtimes[2]
        .cancel()
        .expect("playback fragment cancellation stays fragment-local");

    let accepted_plan = exact.plan.clone();
    for lost in [
        crate::distributed_house_plan::DistributedHouseRole::Cognition,
        crate::distributed_house_plan::DistributedHouseRole::Output,
    ] {
        let refusal = crate::distributed_house_plan::replan_distributed_spoken_house_after_loss(
            &template, lost,
        )
        .unwrap_err();
        assert!(refusal.contains("UnknownHost"), "{refusal}");
        assert_eq!(exact.plan, accepted_plan);
    }
}
