use conduit_core::{
    BackoffSchedule, HostServiceAuthorization, HostServiceAvailability, HostServiceCapability,
    HostServiceContract, HostServiceRisk, Id, RetryContract, StandardContractError,
    StandardNodeContract, StandardNodeKind, StandardNodeLimits, resolve_host_service_contract,
    validate_host_service_contract, validate_retry_contract, validate_standard_node_contract,
};

const FIXTURE: &str = include_str!("../../../conformance/c4/standard-node-library.json");

fn host_service() -> HostServiceContract<'static> {
    HostServiceContract {
        interface: Id("host/blob-store"),
        interface_version: 1,
        operation: Id("blob/read"),
        provider_binding: Id("provider/blob-store"),
        resource_binding: Id("blob/exact"),
        grant: Id("grant/read-blob"),
        cancellation_scope: Id("scope/read"),
        maximum_request_bytes: 128,
        maximum_response_bytes: 4096,
        maximum_pending: 2,
        evidence_events: 8,
        risk: HostServiceRisk::ReferenceSafe,
        enabled_in_reference_registry: true,
    }
}

fn host_capability() -> HostServiceCapability<'static> {
    HostServiceCapability {
        interface: Id("host/blob-store"),
        interface_version: 1,
        operation: Id("blob/read"),
        provider_binding: Id("provider/blob-store"),
        observed_at_tick: 10,
        valid_until_tick: 20,
        maximum_request_bytes: 128,
        maximum_response_bytes: 4096,
        maximum_pending: 2,
        evidence_events: 8,
        availability: HostServiceAvailability::Supported,
    }
}

#[test]
fn given_a_stateful_timer_when_all_limits_are_finite_then_it_is_portable() {
    let contract = StandardNodeContract {
        id: Id("standard/window"),
        kind: StandardNodeKind::Window,
        limits: StandardNodeLimits {
            retained_values: 8,
            retained_bytes: 4096,
            pending_operations: 0,
            timers: 1,
            work_per_step: 8,
            evidence_events: 16,
        },
        terminal_policy: Id("terminal/drain"),
        cancellation_policy: Id("cancel/abort"),
    };
    assert_eq!(validate_standard_node_contract(contract), Ok(()));
}

#[test]
fn given_a_timer_without_timer_capacity_when_validated_then_it_is_rejected() {
    let contract = StandardNodeContract {
        id: Id("standard/debounce"),
        kind: StandardNodeKind::Debounce,
        limits: StandardNodeLimits {
            retained_values: 1,
            retained_bytes: 64,
            pending_operations: 0,
            timers: 0,
            work_per_step: 1,
            evidence_events: 1,
        },
        terminal_policy: Id("terminal/flush"),
        cancellation_policy: Id("cancel/abort"),
    };
    assert_eq!(
        validate_standard_node_contract(contract),
        Err(StandardContractError::IncompatibleLimits)
    );
}

#[test]
fn given_retry_when_attempt_evidence_is_insufficient_then_it_is_rejected() {
    let retry = RetryContract {
        maximum_attempts: 3,
        deadline_ticks: 100,
        backoff: BackoffSchedule::Fixed { ticks: 2 },
        provider_binding: Id("provider/selected"),
        resource_binding: Id("resource/exact"),
        grant: Id("grant/external"),
        cancellation_scope: Id("scope/request"),
        evidence_events: 2,
    };
    assert_eq!(
        validate_retry_contract(retry),
        Err(StandardContractError::Unbounded)
    );
}

#[test]
fn given_a_dangerous_service_when_enabled_by_default_then_it_is_rejected() {
    let service = HostServiceContract {
        interface: Id("host/process-spawn"),
        interface_version: 1,
        operation: Id("process/spawn"),
        provider_binding: Id("provider/process"),
        resource_binding: Id("executable/tool"),
        grant: Id("grant/spawn-tool"),
        cancellation_scope: Id("scope/process"),
        maximum_request_bytes: 1024,
        maximum_response_bytes: 4096,
        maximum_pending: 1,
        evidence_events: 8,
        risk: HostServiceRisk::Dangerous,
        enabled_in_reference_registry: true,
    };
    assert_eq!(
        validate_host_service_contract(service),
        Err(StandardContractError::UnsafeReferenceDefault)
    );
}

fn node_contract_outcome(id: &str) -> &'static str {
    let (kind, limits) = match id {
        "finite-window" => (
            StandardNodeKind::Window,
            StandardNodeLimits {
                retained_values: 8,
                retained_bytes: 4096,
                pending_operations: 0,
                timers: 1,
                work_per_step: 8,
                evidence_events: 16,
            },
        ),
        "debounce-without-timer" => (
            StandardNodeKind::Debounce,
            StandardNodeLimits {
                retained_values: 1,
                retained_bytes: 64,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "ticker-without-timer" => (
            StandardNodeKind::Ticker,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-ticker" => (
            StandardNodeKind::Ticker,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 1,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "batch-without-state" => (
            StandardNodeKind::Batch,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-batch" => (
            StandardNodeKind::Batch,
            StandardNodeLimits {
                retained_values: 8,
                retained_bytes: 512,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-merge" => (
            StandardNodeKind::Merge,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-select" => (
            StandardNodeKind::Select,
            StandardNodeLimits {
                retained_values: 2,
                retained_bytes: 64,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "record-without-state" => (
            StandardNodeKind::Record,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-record" => (
            StandardNodeKind::Record,
            StandardNodeLimits {
                retained_values: 100,
                retained_bytes: 65536,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 100,
            },
        ),
        "replay-without-state" => (
            StandardNodeKind::Replay,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-replay" => (
            StandardNodeKind::Replay,
            StandardNodeLimits {
                retained_values: 100,
                retained_bytes: 65536,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 100,
            },
        ),
        "probe-without-state" => (
            StandardNodeKind::Probe,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-probe" => (
            StandardNodeKind::Probe,
            StandardNodeLimits {
                retained_values: 16,
                retained_bytes: 1024,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 16,
            },
        ),
        "clock-without-timer" => (
            StandardNodeKind::InjectedClock,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-clock" => (
            StandardNodeKind::InjectedClock,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 1,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "file-read-without-state" => (
            StandardNodeKind::FileRead,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-file-read" => (
            StandardNodeKind::FileRead,
            StandardNodeLimits {
                retained_values: 16,
                retained_bytes: 4096,
                pending_operations: 1,
                timers: 0,
                work_per_step: 1,
                evidence_events: 16,
            },
        ),
        "process-without-state" => (
            StandardNodeKind::ProcessSpawn,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-process" => (
            StandardNodeKind::ProcessSpawn,
            StandardNodeLimits {
                retained_values: 32,
                retained_bytes: 8192,
                pending_operations: 1,
                timers: 0,
                work_per_step: 1,
                evidence_events: 32,
            },
        ),
        "cell-without-state" => (
            StandardNodeKind::RegisterCell,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-cell" => (
            StandardNodeKind::RegisterCell,
            StandardNodeLimits {
                retained_values: 1,
                retained_bytes: 256,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "circuit-breaker-without-timer" => (
            StandardNodeKind::CircuitBreaker,
            StandardNodeLimits {
                retained_values: 10,
                retained_bytes: 1024,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 10,
            },
        ),
        "finite-circuit-breaker" => (
            StandardNodeKind::CircuitBreaker,
            StandardNodeLimits {
                retained_values: 10,
                retained_bytes: 1024,
                pending_operations: 0,
                timers: 1,
                work_per_step: 1,
                evidence_events: 10,
            },
        ),
        "wifi-sta-without-timer" => (
            StandardNodeKind::WifiStation,
            StandardNodeLimits {
                retained_values: 8,
                retained_bytes: 512,
                pending_operations: 1,
                timers: 0,
                work_per_step: 1,
                evidence_events: 8,
            },
        ),
        "finite-wifi-sta" => (
            StandardNodeKind::WifiStation,
            StandardNodeLimits {
                retained_values: 8,
                retained_bytes: 512,
                pending_operations: 1,
                timers: 1,
                work_per_step: 1,
                evidence_events: 8,
            },
        ),
        "tcp-socket-without-state" => (
            StandardNodeKind::TcpSocket,
            StandardNodeLimits {
                retained_values: 0,
                retained_bytes: 0,
                pending_operations: 0,
                timers: 0,
                work_per_step: 1,
                evidence_events: 1,
            },
        ),
        "finite-tcp-socket" => (
            StandardNodeKind::TcpSocket,
            StandardNodeLimits {
                retained_values: 64,
                retained_bytes: 16384,
                pending_operations: 1,
                timers: 1,
                work_per_step: 1,
                evidence_events: 64,
            },
        ),
        _ => panic!("unknown node fixture {id}"),
    };

    let contract = StandardNodeContract {
        id: Id("standard/fixture"),
        kind,
        limits,
        terminal_policy: Id("terminal/drain"),
        cancellation_policy: Id("cancel/abort"),
    };
    match validate_standard_node_contract(contract) {
        Ok(()) => "accepted",
        Err(StandardContractError::IncompatibleLimits) => "incompatible-limits",
        Err(StandardContractError::Unbounded) => "unbounded",
        Err(err) => panic!("unexpected node validation error: {err:?}"),
    }
}

#[test]
fn every_standard_node_contract_fixture_executes_independently() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["schema"], "conduit.standard-node-library-fixture");
    let cases = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["contract"] == "node")
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 28);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(node_contract_outcome(id), case["expected"], "{id}");
    }
}

fn host_service_resolution_outcome(id: &str) -> &'static str {
    let mut capability = host_capability();
    let mut current_tick = 15;
    let mut authorization = HostServiceAuthorization::Authorized {
        grant: Id("grant/read-blob"),
    };
    match id {
        "capable-host-service" => {}
        "insufficient-host-service" => capability.maximum_response_bytes = 4095,
        "denied-host-service" => authorization = HostServiceAuthorization::Denied,
        "stale-host-service" => current_tick = 20,
        "unsupported-host-service" => {
            capability.availability = HostServiceAvailability::Unsupported;
        }
        _ => panic!("unknown host-service resolution fixture {id}"),
    }
    match resolve_host_service_contract(host_service(), capability, current_tick, authorization) {
        Ok(()) => "accepted",
        Err(StandardContractError::InsufficientCapability) => "insufficient-capability",
        Err(StandardContractError::AuthorityDenied) => "authority-denied",
        Err(StandardContractError::StaleCapability) => "stale-capability",
        Err(StandardContractError::UnsupportedHostService) => "unsupported-host-service",
        Err(error) => panic!("unexpected host-service resolution error: {error:?}"),
    }
}

#[test]
fn every_host_service_resolution_fixture_executes_independently() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["schema"], "conduit.standard-node-library-fixture");
    let cases = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["contract"] == "host-service-resolution")
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 5);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            host_service_resolution_outcome(id),
            case["expected"],
            "{id}"
        );
    }
}
