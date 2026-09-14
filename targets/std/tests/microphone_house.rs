#![cfg(feature = "local-model-proof")]

use conduit_ai::{
    LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelIdentity,
    LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
};
use conduit_core::{BootId, HostId, OfferGeneration, PlannedGear};
use conduit_std_host::hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal};
use conduit_std_host::hosted_microphone::{AlsaMicrophoneDiscovery, MicrophoneLimits};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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
        output.extend_from_slice(
            &serde_json::to_vec(&conduit_ai::ModelDerivedResult {
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
            })
            .unwrap(),
        );
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

fn executable(root: &Path, name: &str, source: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, source).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

fn providers(
    transcript: &str,
    suffix: &str,
) -> (
    conduit_std_host::hosted_microphone::AlsaMicrophoneAdapter,
    conduit_std_host::hosted_speech_recognition::WhisperSpeechAdapter,
    PathBuf,
) {
    let root = std::env::temp_dir().join(format!(
        "conduit-microphone-house-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let arecord = executable(
        &root,
        "arecord",
        "#!/bin/sh\nif [ \"$1\" = -l ]; then printf 'card 1: DSP [SOF DSP], device 7: DMIC [DMIC]\\n'; exit 0; fi\ndd if=/dev/zero bs=320 count=1 2>/dev/null\n",
    );
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
    let whisper = executable(
        &root,
        "whisper-cli",
        &format!("#!/bin/sh\nout=\nwhile [ $# -gt 0 ]; do if [ \"$1\" = --output-file ]; then out=$2; shift 2; else shift; fi; done\nprintf '{transcript}\\n' > \"${{out}}.txt\"\n"),
    );
    let model = root.join("whisper-model.bin");
    fs::write(&model, b"whisper model fixture").unwrap();
    let whisper = WhisperDiscovery::inspect(&whisper, &model)
        .unwrap()
        .initialize(WhisperLimits {
            maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
            maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
            threads: 1,
            timeout: Duration::from_secs(2),
        })
        .unwrap();
    (microphone, whisper, root)
}

#[test]
fn addressed_microphone_capture_reaches_the_model_in_one_plan_play() {
    let (microphone, whisper, root) =
        providers("Rosehip House, temperature upstairs?", "addressed");
    let receipt = conduit_std_host::recorded_house_proof::run_microphone(
        StdHostConfig {
            host_id: HostId::from("microphone-house-host"),
            boot_id: BootId::from("microphone-house-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(FakeModel {
            offer: local_offer(),
            calls: Arc::new(AtomicUsize::new(0)),
        }),
        whisper,
        microphone,
    )
    .unwrap();
    assert_eq!(receipt.house.local_model_invocations, 1);
    assert_eq!(receipt.raw_pcm_bytes, 320);
    assert_eq!(receipt.house.response_bytes, 33);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unaddressed_microphone_capture_never_invokes_the_model() {
    let (microphone, whisper, root) = providers("Temperature upstairs?", "unaddressed");
    let calls = Arc::new(AtomicUsize::new(0));
    let result = conduit_std_host::recorded_house_proof::run_microphone(
        StdHostConfig {
            host_id: HostId::from("unaddressed-microphone-house-host"),
            boot_id: BootId::from("unaddressed-microphone-house-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(FakeModel {
            offer: local_offer(),
            calls: Arc::clone(&calls),
        }),
        whisper,
        microphone,
    );
    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    fs::remove_dir_all(root).unwrap();
}
