use conduit_core::{
    BackoffSchedule, HostServiceContract, HostServiceRisk, Id, RetryContract,
    StandardContractError, StandardNodeContract, StandardNodeKind, StandardNodeLimits,
    validate_host_service_contract, validate_retry_contract, validate_standard_node_contract,
};

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
