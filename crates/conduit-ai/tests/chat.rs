use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use bumpalo::Bump;
use conduit_ai::{
    ENDPOINT_CONSTRAINT, LOCAL_CHAT_ACTION, LOCAL_CHAT_AUTHORITY, LOCAL_CHAT_RESOURCE_KIND,
    MODEL_CONSTRAINT, OllamaInstallation, OllamaObserver, PROFILE_CONSTRAINT,
    install_observed_ollama_implementation, register_chat_contracts,
    register_deterministic_chat_provider,
};
use conduit_compile::{
    ExternalHostServiceAuthorityObservationInput, InstalledHostObservationInput, InstalledProfile,
    PinDocument, ReportCapabilityDocument, compile_source,
    observed_external_host_service_authority, observed_host_service_constraints,
};
use conduit_core::{Id, PlanValidationContext, ReadyQueueDiscipline, SchedulerPolicy};
use conduit_runtime::{Registry, RunIo, SchedulerReservation};
use tempfile::NamedTempFile;

const SOURCE: &str = include_str!("../../../examples/quick-local-chat.panel");
const RUN_ID: &str = "conduit/run/quick-local-chat";

fn run_exact(
    registry: &Registry,
    installed: &InstalledProfile,
    document: &conduit_compile::ExactPlanDocument,
    epoch: u64,
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
                plan_epoch: epoch,
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
                    max_events: 128,
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
fn checked_panel_runs_through_the_production_executor() {
    let mut registry = Registry::hosted_primitives();
    register_deterministic_chat_provider(&mut registry).unwrap();
    let mut first_host = InstalledHostObservationInput::conduct_host();
    first_host.id = "conduit/observation/chat-host-a".to_owned();
    first_host.host = "conduit/host/chat-a".to_owned();
    first_host.boot_id = "conduit/boot/chat-a".to_owned();
    let installed =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &first_host, &[]).unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let mut second_host = first_host.clone();
    second_host.id = "conduit/observation/chat-host-b".to_owned();
    second_host.host = "conduit/host/chat-b".to_owned();
    second_host.boot_id = "conduit/boot/chat-b".to_owned();
    let second_installed =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &second_host, &[]).unwrap();
    let second_document = compile_source(SOURCE, &second_installed.input).unwrap();
    assert_eq!(
        document.source_semantic_hash,
        second_document.source_semantic_hash
    );
    assert_ne!(document.identity, second_document.identity);
    assert_eq!(
        run_exact(&registry, &installed, &document, 123),
        b"Conduit keeps contracts, implementations, host facts, plans, and evidence distinct."
    );
    let chat = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "ai/chat")
        .unwrap();
    assert_eq!(chat.implementation.id, "conduit.ai/chat-reference-rust");
    assert!(!chat.execution_profile.id.is_empty());
    assert!(!chat.artifact.is_empty());
    assert!(!chat.host_observation.is_empty());

    let mut stale_host = first_host;
    stale_host.valid_until_tick = stale_host.current_tick;
    let stale =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &stale_host, &[]).unwrap();
    assert_eq!(
        compile_source(SOURCE, &stale.input).unwrap_err().code(),
        "CND-CMP-008"
    );
}

#[test]
fn a_host_can_know_the_contract_without_an_implementation() {
    let mut registry = Registry::hosted_primitives();
    register_chat_contracts(&mut registry);
    assert_eq!(
        registry.node_availability("ai/chat").state,
        conduit_runtime::AvailabilityState::ContractOnly
    );
    let error = match InstalledProfile::observe_registry(SOURCE, &registry) {
        Ok(_) => panic!("contract-only registry unexpectedly produced an installed profile"),
        Err(error) => error,
    };
    assert_eq!(error.code, "CND-IMP-001");
}

#[test]
fn observed_local_provider_uses_the_same_generic_exact_binding_path() {
    let (endpoint, server) = mock_ollama();
    let mut binary = NamedTempFile::new().unwrap();
    binary
        .write_all(b"exact observed ollama executable")
        .unwrap();
    binary.flush().unwrap();
    let observation = OllamaObserver {
        endpoint,
        binary_path: binary.path().to_owned(),
        model_name: "llama3.2:latest".to_owned(),
        timeout: Duration::from_secs(2),
    }
    .observe(10, 20)
    .unwrap();
    let installation = OllamaInstallation::from_observation(
        observation,
        conduit_ai::ChatBounds::REFERENCE,
        4,
        1_000,
        64,
    )
    .unwrap();
    let implementation_id = installation.implementation_id.clone();
    let profile = installation.profile.clone();
    let endpoint_text = endpoint.to_string();
    let model_digest = profile.model_artifact_digest.clone();
    let profile_identity = profile.identity.clone();
    let mut registry = Registry::hosted_primitives();
    install_observed_ollama_implementation(&mut registry, installation).unwrap();

    let mut host = InstalledHostObservationInput::conduct_host();
    host.id = "conduit/observation/local-chat-host".to_owned();
    host.host = "conduit/host/local-chat".to_owned();
    host.boot_id = "conduit/boot/local-chat".to_owned();
    host.time_basis = "clock/local-chat".to_owned();

    let absent = InstalledProfile::observe_registry_on_host(SOURCE, &registry, &host, &[]).unwrap();
    assert_eq!(
        compile_source(SOURCE, &absent.input).unwrap_err().code(),
        "CND-CMP-006"
    );

    let required = profile.capability_requirement(implementation_id.clone());
    host.capabilities.push(ReportCapabilityDocument {
        interface: PinDocument {
            id: required.interface.id.to_string(),
            schema_version: required.interface.schema_version,
            semantic_hash: required.interface.semantic_hash.to_string(),
        },
        mode: required.mode,
        subject: required.subject.unwrap(),
        details: required.details.unwrap().to_string(),
        capacity: required.minimum_capacity.into(),
    });
    let denied = InstalledProfile::observe_registry_on_host(SOURCE, &registry, &host, &[]).unwrap();
    assert_eq!(
        compile_source(SOURCE, &denied.input).unwrap_err().code(),
        "CND-CMP-006"
    );
    let constraints = observed_host_service_constraints(&[
        (ENDPOINT_CONSTRAINT, endpoint_text.as_bytes()),
        (MODEL_CONSTRAINT, model_digest.as_bytes()),
        (PROFILE_CONSTRAINT, profile_identity.as_bytes()),
    ]);
    let authority =
        observed_external_host_service_authority(ExternalHostServiceAuthorityObservationInput {
            contract_id: "ai/chat".to_owned(),
            instance: "root/chat".to_owned(),
            run_id: RUN_ID.to_owned(),
            epoch: 123,
            host: host.host.clone(),
            time_basis: host.time_basis.clone(),
            observed_at_tick: host.observed_at_tick,
            valid_until_tick: host.valid_until_tick,
            name: "quick-local-chat".to_owned(),
            requirement: LOCAL_CHAT_AUTHORITY.to_string(),
            action: LOCAL_CHAT_ACTION.to_owned(),
            resource_kind: LOCAL_CHAT_RESOURCE_KIND.to_owned(),
            resource_id: "conduit.resource/local-model-service/fixture".to_owned(),
            grant_id: "conduit.grant/quick-local-chat/fixture".to_owned(),
            constraints,
            revocation_grace_ticks: 1,
            cleanup_ticks: 2,
        });
    let installed =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &host, &[authority]).unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let chat = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "ai/chat")
        .unwrap();
    assert_eq!(chat.implementation.id, implementation_id);
    assert_eq!(chat.host_observation, host.id);
    assert_eq!(chat.required_effects.len(), 1);
    assert!(
        document
            .artifacts
            .iter()
            .any(|artifact| artifact.id == profile.model_artifact_id)
    );
    assert!(document.authorities.iter().any(|authority| {
        authority.binding.resource_id == "conduit.resource/local-model-service/fixture"
            && authority.grant.id == "conduit.grant/quick-local-chat/fixture"
    }));
    assert_eq!(
        run_exact(&registry, &installed, &document, 123),
        b"bounded local reply"
    );
    server.join().unwrap();
}

fn mock_ollama() -> (SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for response in [
            br#"{"version":"0.24.0"}"#.as_slice(),
            br#"{"models":[{"name":"llama3.2:latest","model":"llama3.2:latest","size":2019393189,"digest":"a80c4f17acd55265feec403c7aef86be0c25983ab279d83f3bcd3abbcb5b8b72","details":{"format":"gguf","family":"llama","parameter_size":"3.2B","quantization_level":"Q4_K_M"}}]}"#.as_slice(),
            b"{\"model\":\"llama3.2:latest\",\"message\":{\"content\":\"bounded local reply\"},\"done\":false}\n{\"model\":\"llama3.2:latest\",\"message\":{\"content\":\"\"},\"done\":true}\n".as_slice(),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            )
            .unwrap();
            stream.write_all(response).unwrap();
        }
    });
    (endpoint, server)
}

fn read_request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        bytes.extend_from_slice(&buffer[..count]);
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let header_end = header_end + 4;
        let header = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let content_length = header
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
            })
            .unwrap_or(0);
        if bytes.len() >= header_end + content_length {
            return;
        }
    }
}
