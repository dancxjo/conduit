use std::path::Path;

use bumpalo::Bump;
use conduit_audio::transform_implementations::{
    FFMPEG_EXECUTABLE, HALF_GAIN_Q15, MediaImplementation, ObservedMediaArtifact,
    PROCESS_EXECUTE_AUTHORITY, SOX_EXECUTABLE, execute_process_gain,
    install_audio_gain_implementation, observe_media_executable,
};
use conduit_compile::{
    ExternalHostServiceAuthorityObservationInput, InstalledHostObservationInput, InstalledProfile,
    compile_source, observed_external_host_service_authority,
};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SchedulerPolicy, SemanticHash,
};
use conduit_media::{
    ChannelLayout, PcmChunk, gain_pcm, register_deterministic_audio_processing_providers,
    register_deterministic_media_providers,
};
use conduit_process::SupervisedProcessCancellation;
use conduit_runtime::{Registry, RunIo, SchedulerReservation};

const SOURCE: &str = include_str!("../../../examples/media-gain-provider.panel");
const RUN_ID: &str = "conduit/run/media-provider-conformance";

fn fixture() -> PcmChunk {
    PcmChunk::new(
        0,
        48_000,
        ChannelLayout::StereoLr,
        false,
        vec![0, 0, 200, -200, 1_000, -1_000, 20_000, -20_000],
    )
    .unwrap()
}

fn browser_observation() -> ObservedMediaArtifact {
    ObservedMediaArtifact::browser_wasm_linked(
        include_bytes!("../src/transform_implementations.rs"),
        10,
        20,
    )
    .unwrap()
}

fn host_observation(name: &str) -> InstalledHostObservationInput {
    let mut observation = InstalledHostObservationInput::conduct_host();
    observation.id = format!("conduit/observation/{name}");
    observation.host = format!("conduit/host/{name}");
    observation.time_basis = format!("clock/{name}");
    observation
}

fn process_authority(
    host: &InstalledHostObservationInput,
) -> conduit_compile::ObservedHostServiceAuthority {
    observed_external_host_service_authority(ExternalHostServiceAuthorityObservationInput {
        contract_id: "conduit.media/audio/gain".to_owned(),
        instance: "root/gain".to_owned(),
        run_id: RUN_ID.to_owned(),
        epoch: 148,
        host: host.host.clone(),
        time_basis: host.time_basis.clone(),
        observed_at_tick: host.observed_at_tick,
        valid_until_tick: host.valid_until_tick,
        name: "media-provider-process".to_owned(),
        requirement: PROCESS_EXECUTE_AUTHORITY.to_string(),
        action: "conduit.action/execute".to_owned(),
        resource_kind: "conduit.resource/executable".to_owned(),
        resource_id: "conduit.executable/observed-media-provider".to_owned(),
        grant_id: "conduit.grant/media-provider-process".to_owned(),
        constraints: Vec::new(),
        revocation_grace_ticks: 1,
        cleanup_ticks: 2,
    })
}

fn base_registry() -> Registry {
    let mut registry = Registry::hosted_primitives();
    register_deterministic_media_providers(&mut registry).unwrap();
    registry
}

fn exact_plan(
    registry: &Registry,
    host: &InstalledHostObservationInput,
    authorities: &[conduit_compile::ObservedHostServiceAuthority],
) -> (InstalledProfile, conduit_compile::ExactPlanDocument) {
    let installed =
        InstalledProfile::observe_registry_on_host(SOURCE, registry, host, authorities).unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    (installed, document)
}

fn run_exact(
    registry: &Registry,
    installed: &InstalledProfile,
    document: &conduit_compile::ExactPlanDocument,
) -> Vec<u8> {
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let panel = conduit_panel::parse(SOURCE).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let grants = installed.grant_observations(&plan).unwrap();
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    resolved
        .run_exact(
            &plan,
            &bindings,
            conduit_runtime::ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 148,
                run_id: Id(RUN_ID),
                grant_observations: &grants,
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: conduit_core::SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 128,
                    max_tick: 256,
                    max_consecutive_yields: 8,
                    max_events: 64,
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
            },
            &mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut display,
            },
        )
        .unwrap();
    assert!(output.is_empty());
    assert!(error.is_empty());
    display
}

#[test]
fn same_semantic_source_and_topology_bind_distinct_generic_implementations() {
    let mut reference = base_registry();
    register_deterministic_audio_processing_providers(&mut reference).unwrap();
    let reference_host = host_observation("reference-media");
    let (reference_installed, reference_plan) = exact_plan(&reference, &reference_host, &[]);

    let mut browser = base_registry();
    let browser_artifact = browser_observation();
    let browser_version = browser_artifact.version.clone();
    install_audio_gain_implementation(&mut browser, browser_artifact).unwrap();
    let browser_host = host_observation("browser-audio-worklet");
    let (browser_installed, browser_plan) = exact_plan(&browser, &browser_host, &[]);

    assert_eq!(
        reference_plan.source_semantic_hash,
        browser_plan.source_semantic_hash
    );
    assert_eq!(reference_plan.cords, browser_plan.cords);
    assert_ne!(reference_plan.identity, browser_plan.identity);
    let reference_gain = reference_plan
        .nodes
        .iter()
        .find(|node| node.contract.id == "conduit.media/audio/gain")
        .unwrap();
    let browser_gain = browser_plan
        .nodes
        .iter()
        .find(|node| node.contract.id == "conduit.media/audio/gain")
        .unwrap();
    assert_eq!(reference_gain.contract, browser_gain.contract);
    assert_ne!(reference_gain.implementation, browser_gain.implementation);
    assert_ne!(reference_gain.artifact, browser_gain.artifact);
    assert_ne!(reference_gain.host, browser_gain.host);
    assert_eq!(
        run_exact(&reference, &reference_installed, &reference_plan),
        b"audio:s16le:48000:stereo-lr:16"
    );
    assert_eq!(
        browser_gain.implementation.id,
        MediaImplementation::BrowserWasmLinked.id()
    );
    let browser_candidate = browser_installed
        .input
        .candidates
        .iter()
        .find(|candidate| {
            candidate.implementation.id == MediaImplementation::BrowserWasmLinked.id()
        })
        .expect("browser implementation candidate");
    assert_eq!(
        browser_candidate.implementation.implementation_version,
        browser_version
    );
}

#[test]
fn process_implementations_match_reference_and_cancel_with_cleanup() {
    let expected = gain_pcm(&fixture(), HALF_GAIN_Q15, HALF_GAIN_Q15, 0, 0).unwrap();
    let required = std::env::var_os("CONDUIT_REQUIRE_MEDIA_PROCESS_PROVIDERS").is_some();
    for (implementation, executable) in [
        (MediaImplementation::FfmpegProcess, FFMPEG_EXECUTABLE),
        (MediaImplementation::SoxProcess, SOX_EXECUTABLE),
    ] {
        if !Path::new(executable).exists() {
            assert!(
                !required,
                "required provider executable is absent: {executable}"
            );
            continue;
        }
        let observed =
            observe_media_executable(implementation, Path::new(executable), 10, 20).unwrap();
        let output = execute_process_gain(
            &observed,
            12,
            &fixture(),
            SupervisedProcessCancellation::None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(output, expected, "{implementation:?} normalized output");
        assert_eq!(
            execute_process_gain(
                &observed,
                12,
                &fixture(),
                SupervisedProcessCancellation::AfterSpawn,
            )
            .unwrap(),
            None,
            "{implementation:?} cancellation is bounded and cleaned up"
        );

        let mut registry = base_registry();
        install_audio_gain_implementation(&mut registry, observed).unwrap();
        let host = host_observation(match implementation {
            MediaImplementation::FfmpegProcess => "linux-ffmpeg-process",
            MediaImplementation::SoxProcess => "linux-sox-process",
            MediaImplementation::BrowserWasmLinked => unreachable!(),
        });
        let ungranted =
            InstalledProfile::observe_registry_on_host(SOURCE, &registry, &host, &[]).unwrap();
        let process_candidate = ungranted
            .input
            .candidates
            .iter()
            .find(|candidate| candidate.implementation.id == implementation.id())
            .unwrap();
        assert_eq!(
            process_candidate.implementation.required_authorities,
            vec![PROCESS_EXECUTE_AUTHORITY.to_string()]
        );
        assert_eq!(
            compile_source(SOURCE, &ungranted.input).unwrap_err().code(),
            "CND-CMP-006",
            "{implementation:?} must not resolve without its process grant"
        );
        let authority = process_authority(&host);
        let (installed, plan) = exact_plan(&registry, &host, &[authority]);
        let gain = plan
            .nodes
            .iter()
            .find(|node| node.contract.id == "conduit.media/audio/gain")
            .unwrap();
        assert_eq!(gain.implementation.id, implementation.id());
        assert_eq!(gain.required_effects.len(), 1);
        assert_eq!(gain.required_effects[0], plan.authorities[0].effect_hash);
        assert!(plan.authorities.iter().any(|authority| {
            authority.grant.id == "conduit.grant/media-provider-process"
                && authority.binding.resource_id == "conduit.executable/observed-media-provider"
        }));
        assert_eq!(
            run_exact(&registry, &installed, &plan),
            b"audio:s16le:48000:stereo-lr:16"
        );
    }
}

#[test]
fn absent_stale_changed_and_unsupported_implementations_fail_before_media_work() {
    assert!(
        observe_media_executable(
            MediaImplementation::FfmpegProcess,
            Path::new("/definitely/absent/ffmpeg"),
            10,
            20,
        )
        .is_err()
    );
    let browser = browser_observation();
    assert!(!browser.fresh_at(21));

    let mut registry = base_registry();
    install_audio_gain_implementation(&mut registry, browser).unwrap();
    let unsupported = SOURCE.replace("start_gain_q15 = 16384", "start_gain_q15 = 32768");
    let unsupported = conduit_panel::parse(&unsupported).unwrap();
    assert_eq!(
        registry.resolve(&unsupported).unwrap_err().code,
        "CND-MPR-010"
    );

    let contract_hash =
        conduit_runtime::OwnedNodeSchema::from_contract(&conduit_media::AUDIO_GAIN_CONTRACT)
            .semantic_hash();
    assert_ne!(contract_hash, SemanticHash::from_bytes([0; 32]));
}
