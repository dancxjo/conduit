use conduit_core::{
    EventCorrelation, EventPayload, EventPayloadShape, EventRelations, EventTerminality, EventTime,
    EventTimeKind, EvidencePolicy, ExecutionEvent, ExecutionEventKind, Id, InstancePath,
    SemanticHash, Sensitivity, TerminalClass, TypeContractRef, validate_event_stream,
    validate_execution_event,
};
use conduit_runtime::{OwnedEventPayload, OwnedPayloadShape, OwnedTypeRef};
use conduit_runtime::{decode_event_ndjson, encode_event_ndjson, encode_owned_event_ndjson};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const PLAN: SemanticHash = SemanticHash::from_bytes([3; 32]);
const VALUE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([4; 32]),
};
const POLICY: EvidencePolicy = EvidencePolicy {
    max_inline_payload_bytes: 32,
    reveal_redacted_byte_length: true,
    reveal_redacted_item_count: true,
};

#[allow(clippy::too_many_arguments)]
fn event<'a>(
    id: &'a str,
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
        event_id: Id(id),
        run_id: Id("run/ndjson"),
        plan_identity: PLAN,
        sequence,
        recorder: Id("recorder/main"),
        observer: Id(observer),
        observer_sequence,
        logical_template: Some(InstancePath::new("root").unwrap()),
        subject: InstancePath::new(subject).unwrap(),
        kind,
        detail: Id(detail),
        observed_time: EventTime {
            kind: EventTimeKind::Monotonic,
            basis: Id(observer),
            tick: observed_tick,
        },
        domain_time: None,
        correlation: EventCorrelation {
            request: Some(Id("request/a")),
            exchange: Some(Id("exchange/a")),
            session: Some(Id("session/a")),
            epoch: Some(1),
            work_unit: Some(Id("work/a")),
            attempt: Some(Id("attempt/a")),
            correlation: Some(Id("correlation/a")),
            idempotency: Some(Id("idempotency/a")),
            checkpoint: None,
            transport: Some(Id("transport/a")),
        },
        relations,
        terminality,
        payload,
    };
    event.identity = event.semantic_hash().unwrap();
    event
}

fn none() -> EventRelations<'static> {
    EventRelations {
        caused_by: None,
        derived_from: &[],
        supersedes: None,
        retracts: None,
    }
}

#[test]
fn frozen_ndjson_round_trips_through_owned_and_core_forms() {
    let source_id = Id("event/source");
    let source = event(
        source_id.as_str(),
        0,
        "host/a",
        0,
        "root/source/attempt.a",
        ExecutionEventKind::Domain,
        "fixture/hypothesis",
        100,
        none(),
        EventTerminality::NonTerminal,
        EventPayload::InlinePublic {
            value_type: VALUE_TYPE,
            bytes: b"hello",
        },
    );
    let derived = [source_id];
    let dropped_id = Id("event/dropped");
    let dropped = event(
        dropped_id.as_str(),
        1,
        "host/b",
        0,
        "root/sink/attempt.a",
        ExecutionEventKind::ValueDropped,
        "conduit/drop-disposable",
        40,
        EventRelations {
            caused_by: Some(source_id),
            derived_from: &derived,
            supersedes: None,
            retracts: None,
        },
        EventTerminality::NonTerminal,
        EventPayload::Redacted {
            value_type: VALUE_TYPE,
            sensitivity: Sensitivity::Secret,
            shape: EventPayloadShape {
                byte_length: Some(5),
                item_count: Some(1),
            },
            reason: Id("conduit/sensitivity"),
        },
    );
    let terminal = event(
        "event/terminal",
        2,
        "host/a",
        1,
        "root",
        ExecutionEventKind::Terminal,
        "conduit/run-succeeded",
        110,
        EventRelations {
            caused_by: Some(dropped_id),
            ..none()
        },
        EventTerminality::Terminal {
            class: TerminalClass::Succeeded,
            cause: Id("conduit/natural"),
        },
        EventPayload::None,
    );
    let events = [source, dropped, terminal];
    assert_eq!(validate_event_stream(&events, POLICY), Ok(()));

    let encoded = encode_event_ndjson(&events).unwrap();
    assert_eq!(
        encoded,
        include_str!("../../../conformance/c2/execution-event-v1.ndjson")
    );
    let decoded = decode_event_ndjson(&encoded).unwrap();
    assert_eq!(encode_owned_event_ndjson(&decoded).unwrap(), encoded);
    for owned in &decoded {
        let mut derivations = [Id("scratch"); 16];
        let borrowed = owned.as_event(&mut derivations).unwrap();
        assert_eq!(validate_execution_event(&borrowed, POLICY), Ok(()));
        assert_eq!(borrowed.identity, borrowed.semantic_hash().unwrap());
    }

    let mut malformed = decoded[0].clone();
    malformed.payload = OwnedEventPayload::Reference {
        value_type: OwnedTypeRef {
            id: "fixture/value".to_owned(),
            schema_version: 1,
            semantic_hash:
                "sha256:0404040404040404040404040404040404040404040404040404040404040404".to_owned(),
        },
        digest: "not-a-digest".to_owned(),
        sensitivity: "public".to_owned(),
        shape: OwnedPayloadShape {
            byte_length: Some(5),
            item_count: Some(1),
        },
        recording_authority: None,
    };
    assert!(malformed.as_event(&mut []).is_err());

    let with_unknown = encoded.replacen(
        "\"schema_version\":1,",
        "\"schema_version\":1,\"unknown\":true,",
        1,
    );
    assert!(decode_event_ndjson(&with_unknown).is_err());

    let mut uppercase_hash = decoded[0].clone();
    uppercase_hash.identity = uppercase_hash.identity.to_uppercase();
    assert!(uppercase_hash.as_event(&mut []).is_err());
}
