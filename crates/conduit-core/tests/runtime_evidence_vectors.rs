use conduit_core::{
    BlockingFairness, EventClass, EventProviderCapabilities, EventStreamContract, FlowCapacity,
    FlowPolicy, FlowWatermarks, Id, PinnedDescriptor, Pressure, RUNTIME_EVIDENCE_POLICY_VERSION,
    ReplayDelivery, RetentionPolicy, RuntimeEvidenceBudget, RuntimeEvidenceMode,
    RuntimeEvidencePolicy, RuntimeEvidenceReason, SemanticHash, Sensitivity, SubscriberCoupling,
    TelemetryAdmission, TypeContractRef, validate_runtime_evidence_policy,
};

const FIXTURE: &str = include_str!("../../../conformance/c4/runtime-evidence-v1.json");

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn flow(pressure: Pressure<'static>) -> FlowPolicy<'static> {
    let capacity = FlowCapacity::new(4, 64, 256).unwrap();
    FlowPolicy::new(
        capacity,
        pressure,
        FlowWatermarks::new(1, 3, capacity).unwrap(),
    )
    .unwrap()
}

fn contract(
    pressure: Pressure<'static>,
) -> (EventStreamContract<'static>, EventProviderCapabilities) {
    (
        EventStreamContract {
            id: Id("stream/runtime"),
            event_class: EventClass::NormativeEvidence,
            payload_type: TypeContractRef {
                contract_id: Id("conduit/runtime-observation"),
                schema_version: 1,
                semantic_hash: hash(0x23),
            },
            retention: RetentionPolicy::Ring {
                maximum_events: 32,
                maximum_bytes: 32_000,
            },
            subscriber_coupling: SubscriberCoupling::Coupled(flow(pressure)),
            delivery: ReplayDelivery::AtLeastOnce,
            maximum_publishers: 1,
            maximum_subscribers: 2,
            maximum_pending_operations: 2,
            maximum_projection_bytes: 1_024,
            provider: PinnedDescriptor {
                id: Id("provider/evidence"),
                schema_version: 1,
                semantic_hash: hash(2),
            },
            recording_authority: None,
            sensitivity: Sensitivity::Public,
            terminal_evidence_required: true,
        },
        EventProviderCapabilities {
            ephemeral: true,
            retained: true,
            durable: false,
            checkpoint_cursor: false,
            integrity: true,
            redaction: true,
            maximum_events: 32,
            maximum_bytes: 32_000,
            maximum_subscribers: 2,
            maximum_pending_operations: 2,
        },
    )
}

fn policy() -> RuntimeEvidencePolicy<'static> {
    RuntimeEvidencePolicy {
        schema_version: RUNTIME_EVIDENCE_POLICY_VERSION,
        mode: RuntimeEvidenceMode::Record,
        stream: Some(Id("stream/runtime")),
        maximum_events: 16,
        maximum_bytes: 16_000,
        required_reserve_events: 1,
        required_reserve_bytes: 1_000,
        telemetry_period: 2,
        telemetry_offset: 0,
        gap_summary_bytes: 500,
    }
}

#[test]
fn fixture_inventory_names_every_runtime_evidence_boundary() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["suite"], "conduit.runtime-evidence/v1");
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    for required in [
        "explicit-disabled-policy",
        "plan-v8-policy-identity",
        "pressure-loss-golden-sequence",
        "scheduling-latency-monotonic",
        "derivation-links-accepted-input",
        "logical-and-expanded-paths",
        "telemetry-sampling-summary",
        "required-capacity-fails-closed",
        "shared-resonance-envelope",
        "redaction-does-not-copy-value-bytes",
        "run-terminal-exactly-once",
        "channel-bytes-not-evidence-input",
        "value-clock-semantics-deferred",
        "deadline-guarantee-deferred",
    ] {
        assert!(ids.contains(&required), "missing fixture {required}");
    }
}

#[test]
fn disabled_and_recording_policies_are_explicit_and_stream_checked() {
    let disabled = RuntimeEvidencePolicy {
        schema_version: RUNTIME_EVIDENCE_POLICY_VERSION,
        mode: RuntimeEvidenceMode::Disabled,
        stream: None,
        maximum_events: 0,
        maximum_bytes: 0,
        required_reserve_events: 0,
        required_reserve_bytes: 0,
        telemetry_period: 0,
        telemetry_offset: 0,
        gap_summary_bytes: 0,
    };
    assert_eq!(validate_runtime_evidence_policy(disabled, None), Ok(()));
    assert_eq!(
        validate_runtime_evidence_policy(
            RuntimeEvidencePolicy {
                stream: Some(Id("stream/runtime")),
                ..disabled
            },
            None
        ),
        Err(RuntimeEvidenceReason::StreamForbidden)
    );

    let stream = contract(Pressure::Block(BlockingFairness::Fifo));
    assert_eq!(
        validate_runtime_evidence_policy(policy(), Some(stream)),
        Ok(())
    );
    let lossy = contract(Pressure::DropDisposable);
    assert_eq!(
        validate_runtime_evidence_policy(policy(), Some(lossy)),
        Err(RuntimeEvidenceReason::StreamIncapable)
    );
}

#[test]
fn telemetry_sampling_is_summarized_and_terminal_capacity_is_reserved() {
    let mut budget = RuntimeEvidenceBudget::new(policy());
    assert_eq!(
        budget.admit_telemetry(100).unwrap(),
        TelemetryAdmission::Record
    );
    assert_eq!(
        budget.admit_telemetry(100).unwrap(),
        TelemetryAdmission::Sampled
    );
    assert_eq!(
        budget.admit_telemetry(100).unwrap(),
        TelemetryAdmission::RecordAfterSummary { skipped: 1 }
    );
    budget.record_required(100, false).unwrap();
    budget.record_required(900, true).unwrap();
    assert_eq!(budget.recorded_events(), 5);
    assert_eq!(budget.recorded_bytes(), 1_700);
    assert_eq!(budget.finish(), Ok(()));
}

#[test]
fn required_and_summary_capacity_fail_closed() {
    let tight = RuntimeEvidencePolicy {
        maximum_events: 3,
        maximum_bytes: 1_500,
        required_reserve_events: 1,
        required_reserve_bytes: 500,
        gap_summary_bytes: 400,
        telemetry_period: 100,
        ..policy()
    };
    let mut required = RuntimeEvidenceBudget::new(tight);
    required.record_required(500, false).unwrap();
    assert_eq!(
        required.record_required(501, false),
        Err(RuntimeEvidenceReason::RequiredCapacityExceeded)
    );

    let mut summary = RuntimeEvidenceBudget::new(tight);
    assert_eq!(
        summary.admit_telemetry(601).unwrap(),
        TelemetryAdmission::Record
    );
    assert_eq!(
        summary.admit_telemetry(601).unwrap(),
        TelemetryAdmission::Sampled
    );
    assert_eq!(
        summary.flush_sampling_summary(),
        Err(RuntimeEvidenceReason::SummaryCapacityExceeded)
    );
}
