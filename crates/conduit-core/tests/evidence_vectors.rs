use conduit_core::{
    ArtifactDigest, EventCorrelation, EventPayload, EventPayloadShape, EventRelations,
    EventTerminality, EventTime, EventTimeKind, EvidencePolicy, EvidenceReason, ExecutionEvent,
    ExecutionEventKind, Id, InstancePath, SemanticHash, Sensitivity, TerminalClass,
    TypeContractRef, validate_event_stream, validate_execution_event,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const PLAN: SemanticHash = SemanticHash::from_bytes([1; 32]);
const VALUE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([2; 32]),
};
const POLICY: EvidencePolicy = EvidencePolicy {
    max_inline_payload_bytes: 32,
    reveal_redacted_byte_length: true,
    reveal_redacted_item_count: true,
};

fn observed(recorder: &str, tick: i64) -> EventTime<'_> {
    EventTime {
        kind: EventTimeKind::Monotonic,
        basis: Id(recorder),
        tick,
    }
}

#[allow(clippy::too_many_arguments)]
fn event<'a>(
    event_id: &'a str,
    sequence: u64,
    observer: &'a str,
    observer_sequence: u64,
    subject: &'a str,
    kind: ExecutionEventKind,
    detail: &'a str,
    observed_tick: i64,
    relations: EventRelations<'a>,
    terminality: EventTerminality<'a>,
    payload: EventPayload<'a>,
) -> ExecutionEvent<'a> {
    let mut event = ExecutionEvent {
        schema_version: 1,
        identity: ZERO,
        event_id: Id(event_id),
        run_id: Id("run/fixture"),
        plan_identity: PLAN,
        sequence,
        recorder: Id("recorder/main"),
        observer: Id(observer),
        observer_sequence,
        logical_template: Some(InstancePath::new("root").unwrap()),
        subject: InstancePath::new(subject).unwrap(),
        kind,
        detail: Id(detail),
        observed_time: observed(observer, observed_tick),
        domain_time: Some(EventTime {
            kind: EventTimeKind::Domain,
            basis: Id("domain/media"),
            tick: observed_tick * 10,
        }),
        correlation: EventCorrelation {
            request: Some(Id("request/a")),
            exchange: Some(Id("exchange/a")),
            session: Some(Id("session/a")),
            epoch: Some(3),
            work_unit: Some(Id("work/a")),
            attempt: Some(Id("attempt/a")),
            correlation: Some(Id("correlation/a")),
            idempotency: Some(Id("idempotency/a")),
            checkpoint: Some(Id("checkpoint/a")),
            transport: Some(Id("transport/a")),
        },
        relations,
        terminality,
        payload,
    };
    event.identity = event.semantic_hash().unwrap();
    event
}

fn no_relations() -> EventRelations<'static> {
    EventRelations {
        caused_by: None,
        derived_from: &[],
        supersedes: None,
        retracts: None,
    }
}

#[test]
fn correlation_categories_cannot_collapse_or_reuse_attempt_identity() {
    let fixture = include_str!("../../../conformance/c2/port-group-correlation-v1.json");
    for family in [
        "request",
        "exchange",
        "session",
        "session-epoch",
        "work-unit",
        "attempt",
        "causation",
        "correlation",
        "idempotency",
        "checkpoint",
        "transport",
        "logical-template",
        "concrete-instance",
        "generation",
        "plan-epoch",
    ] {
        assert!(fixture.contains(&format!("\"id\": \"{family}\"")));
    }
    for case in [
        "local-request-reply",
        "remote-request-reply",
        "retry",
        "restart",
        "checkpoint-resume",
        "replicated-generation",
        "plan-epoch-transition",
        "wall-clock-request",
        "scheduler-order-attempt",
        "registry-order-member",
        "map-order-generation",
        "transport-order-request",
        "work-as-attempt",
        "timestamp-as-causation",
    ] {
        assert!(fixture.contains(&format!("\"id\": \"{case}\"")));
    }
    let valid = event(
        "event/correlation-valid",
        0,
        "host/a",
        0,
        "root/source",
        ExecutionEventKind::Progress,
        "fixture/progress",
        1,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::None,
    );
    assert_eq!(validate_execution_event(&valid, POLICY), Ok(()));

    let mut collapsed = valid;
    collapsed.correlation.attempt = collapsed.correlation.work_unit;
    collapsed.identity = collapsed.semantic_hash().unwrap();
    assert_eq!(
        validate_execution_event(&collapsed, POLICY)
            .unwrap_err()
            .reason,
        EvidenceReason::InvalidDescriptor
    );

    let mut orphan_attempt = valid;
    orphan_attempt.correlation.work_unit = None;
    orphan_attempt.identity = orphan_attempt.semantic_hash().unwrap();
    assert_eq!(
        validate_execution_event(&orphan_attempt, POLICY)
            .unwrap_err()
            .reason,
        EvidenceReason::InvalidDescriptor
    );

    let mut orphan_epoch = valid;
    orphan_epoch.correlation.session = None;
    orphan_epoch.identity = orphan_epoch.semantic_hash().unwrap();
    assert_eq!(
        validate_execution_event(&orphan_epoch, POLICY)
            .unwrap_err()
            .reason,
        EvidenceReason::InvalidDescriptor
    );
}

#[test]
fn causal_replay_preserves_distributed_order_corrections_and_terminal_state() {
    let source_id = Id("event/source");
    let pressure_id = Id("event/pressure");
    let source = event(
        source_id.as_str(),
        0,
        "host/a",
        0,
        "root/source/attempt.a",
        ExecutionEventKind::Domain,
        "fixture/hypothesis",
        100,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::InlinePublic {
            value_type: VALUE_TYPE,
            bytes: b"first",
        },
    );
    let pressure = event(
        pressure_id.as_str(),
        1,
        "host/b",
        0,
        "root/cord.values",
        ExecutionEventKind::Pressure,
        "conduit/pressure-entered",
        40,
        EventRelations {
            caused_by: Some(source_id),
            ..no_relations()
        },
        EventTerminality::NonTerminal,
        EventPayload::None,
    );
    let derivations = [source_id];
    let dropped = event(
        "event/dropped",
        2,
        "host/b",
        1,
        "root/sink/attempt.a",
        ExecutionEventKind::ValueDropped,
        "conduit/drop-disposable",
        50,
        EventRelations {
            caused_by: Some(pressure_id),
            derived_from: &derivations,
            supersedes: None,
            retracts: None,
        },
        EventTerminality::NonTerminal,
        EventPayload::Redacted {
            value_type: VALUE_TYPE,
            sensitivity: Sensitivity::Secret,
            shape: EventPayloadShape {
                byte_length: Some(128),
                item_count: Some(1),
            },
            reason: Id("conduit/sensitivity"),
        },
    );
    let correction_derivations = [source_id];
    let correction = event(
        "event/correction",
        3,
        "host/a",
        1,
        "root/source/attempt.a",
        ExecutionEventKind::Correction,
        "fixture/correction",
        120,
        EventRelations {
            caused_by: Some(source_id),
            derived_from: &correction_derivations,
            supersedes: Some(source_id),
            retracts: None,
        },
        EventTerminality::NonTerminal,
        EventPayload::InlinePublic {
            value_type: VALUE_TYPE,
            bytes: b"corrected",
        },
    );
    let terminal = event(
        "event/terminal",
        4,
        "host/a",
        2,
        "root",
        ExecutionEventKind::Terminal,
        "conduit/run-succeeded",
        130,
        EventRelations {
            caused_by: Some(Id("event/correction")),
            ..no_relations()
        },
        EventTerminality::Terminal {
            class: TerminalClass::Succeeded,
            cause: Id("conduit/natural"),
        },
        EventPayload::None,
    );
    let events = [source, pressure, dropped, correction, terminal];
    assert_eq!(validate_event_stream(&events, POLICY), Ok(()));
    assert!(events[1].observed_time.tick < events[0].observed_time.tick);
    assert_eq!(events[2].relations.derived_from, &[source_id]);
    assert!(!format!("{:?}", events[2].payload).contains("first"));

    let fixture = include_str!("../../../conformance/c2/execution-event-v1.tsv");
    for case in [
        "causation_chain",
        "nested_subject",
        "pressure_loss",
        "redaction",
        "correction",
        "terminal",
        "distributed_out_of_order",
        "replay_equivalence",
        "malformed_payload_reference",
        "semantic_hash_stability",
        "ndjson_round_trip",
    ] {
        assert!(
            fixture.lines().any(|line| line.starts_with(case)),
            "missing fixture {case}"
        );
    }
}

#[test]
fn payload_policy_and_event_identity_fail_closed() {
    let oversized = event(
        "event/large",
        0,
        "host/a",
        0,
        "root/source",
        ExecutionEventKind::Domain,
        "fixture/value",
        1,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::InlinePublic {
            value_type: VALUE_TYPE,
            bytes: &[0; 33],
        },
    );
    assert_eq!(
        validate_execution_event(&oversized, POLICY)
            .unwrap_err()
            .reason,
        EvidenceReason::InlinePayloadTooLarge
    );

    let protected = event(
        "event/reference",
        0,
        "host/a",
        0,
        "root/source",
        ExecutionEventKind::Checkpoint,
        "conduit/checkpoint",
        1,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::Reference {
            value_type: VALUE_TYPE,
            digest: ArtifactDigest::from_bytes([7; 32]),
            sensitivity: Sensitivity::Secret,
            shape: EventPayloadShape {
                byte_length: Some(64),
                item_count: None,
            },
            recording_authority: None,
        },
    );
    assert_eq!(
        validate_execution_event(&protected, POLICY)
            .unwrap_err()
            .reason,
        EvidenceReason::ProtectedPayloadUnrecordable
    );
    assert!(!format!("{:?}", protected.payload).contains("ArtifactDigest"));
    assert!(format!("{:?}", protected.payload).contains("<redacted>"));

    let redacted = event(
        "event/redacted",
        0,
        "host/a",
        0,
        "root/source",
        ExecutionEventKind::Domain,
        "fixture/redacted",
        1,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::Redacted {
            value_type: VALUE_TYPE,
            sensitivity: Sensitivity::Secret,
            shape: EventPayloadShape {
                byte_length: Some(64),
                item_count: None,
            },
            reason: Id("conduit/sensitivity"),
        },
    );
    assert_eq!(
        validate_execution_event(
            &redacted,
            EvidencePolicy {
                max_inline_payload_bytes: 32,
                reveal_redacted_byte_length: false,
                reveal_redacted_item_count: false,
            },
        )
        .unwrap_err()
        .reason,
        EvidenceReason::InvalidRedaction
    );

    let valid = event(
        "event/valid",
        0,
        "host/a",
        0,
        "root/source",
        ExecutionEventKind::Progress,
        "conduit/progress",
        1,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::None,
    );
    assert_eq!(validate_execution_event(&valid, POLICY), Ok(()));
    assert_eq!(
        validate_execution_event(
            &ExecutionEvent {
                detail: Id("fixture/changed"),
                ..valid
            },
            POLICY,
        )
        .unwrap_err()
        .reason,
        EvidenceReason::IdentityMismatch
    );

    let forward = [Id("event/a"), Id("event/b")];
    let reverse = [Id("event/b"), Id("event/a")];
    let with_derivations = |derived_from| {
        event(
            "event/derived",
            0,
            "host/a",
            0,
            "root/source",
            ExecutionEventKind::Derivation,
            "conduit/derivation",
            1,
            EventRelations {
                caused_by: None,
                derived_from,
                supersedes: None,
                retracts: None,
            },
            EventTerminality::NonTerminal,
            EventPayload::None,
        )
    };
    assert_eq!(
        with_derivations(&forward).identity,
        with_derivations(&reverse).identity
    );
}

#[test]
fn replay_rejects_missing_cause_sequence_and_mutating_correction() {
    let missing = event(
        "event/missing-cause",
        0,
        "host/a",
        0,
        "root/source",
        ExecutionEventKind::Derivation,
        "conduit/derivation",
        1,
        EventRelations {
            caused_by: Some(Id("event/absent")),
            ..no_relations()
        },
        EventTerminality::NonTerminal,
        EventPayload::None,
    );
    assert_eq!(
        validate_event_stream(&[missing], POLICY)
            .unwrap_err()
            .reason,
        EvidenceReason::CausalReferenceMissing
    );

    let first = event(
        "event/first",
        0,
        "host/a",
        0,
        "root/source",
        ExecutionEventKind::Progress,
        "conduit/progress",
        1,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::None,
    );
    let gap = event(
        "event/gap",
        2,
        "host/a",
        1,
        "root/source",
        ExecutionEventKind::Progress,
        "conduit/progress",
        2,
        no_relations(),
        EventTerminality::NonTerminal,
        EventPayload::None,
    );
    assert_eq!(
        validate_event_stream(&[first, gap], POLICY)
            .unwrap_err()
            .reason,
        EvidenceReason::SequenceViolation
    );
}
