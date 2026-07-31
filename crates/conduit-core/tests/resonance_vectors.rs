use conduit_core::{
    AppendCommit, AppendOutcome, AppendRecovery, ArtifactDigest, BlockingFairness,
    BoundedEventRing, EventClass, EventPayloadRef, EventProviderCapabilities, EventStreamContract,
    FlowCapacity, FlowPolicy, FlowWatermarks, Id, InstancePath, PinnedDescriptor, Pressure,
    ProjectionContract, ProjectionSnapshot, ReadOutcome, ReplayDelivery, ReplayStart,
    ResonanceEnvelope, ResonanceError, ResonanceRelations, RetentionPolicy, SampleSchedule,
    SemanticHash, Sensitivity, SubscriberCoupling, SubscriptionContract, TypeContractRef,
    validate_projection, validate_projection_snapshot, validate_stream_contract,
    validate_subscription,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn flow(items: u16) -> FlowPolicy<'static> {
    let capacity = FlowCapacity::new(items, 64, u64::from(items) * 64).unwrap();
    FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, items, capacity).unwrap(),
    )
    .unwrap()
}

fn envelope(event: &'static str, sequence: u64) -> ResonanceEnvelope<'static> {
    ResonanceEnvelope {
        event: Id(event),
        stream: Id("stream/domain"),
        run: Id("run/a"),
        plan_epoch: hash(1),
        producer: InstancePath::new("root/source").unwrap(),
        subject: InstancePath::new("root/value").unwrap(),
        class: EventClass::Domain,
        sequence,
        observer: Id("host/a"),
        observer_sequence: sequence,
        domain_time: None,
        correlation: Some(Id("correlation/a")),
        idempotency: Some(Id("idempotency/a")),
        payload_type: TypeContractRef {
            contract_id: Id("fixture/event"),
            schema_version: 0,
            semantic_hash: hash(2),
        },
        payload: EventPayloadRef::ContentAddressed {
            digest: ArtifactDigest::from_bytes([3; 32]),
            bytes: 16,
        },
        relations: ResonanceRelations {
            caused_by: None,
            derived_from: &[],
            supersedes: None,
            corrects: None,
            retracts: None,
        },
        provenance: Id("provider/reference"),
        recording_authority: Some(Id("grant/record")),
        sensitivity: Sensitivity::Restricted,
        integrity: hash(4),
    }
}

#[test]
fn bounded_ring_reports_pressure_and_retention_gaps() {
    let mut coupled = BoundedEventRing::<2>::new(64);
    assert_eq!(
        coupled.append(envelope("event/e0", 0), 32, false),
        Ok(AppendOutcome::Committed { cursor: 0 })
    );
    coupled.append(envelope("event/e1", 1), 32, false).unwrap();
    assert_eq!(
        coupled.append(envelope("event/e2", 2), 32, false),
        Ok(AppendOutcome::WouldBlock)
    );

    let mut isolated = BoundedEventRing::<2>::new(64);
    isolated.append(envelope("event/e0", 0), 32, true).unwrap();
    isolated.append(envelope("event/e1", 1), 32, true).unwrap();
    assert_eq!(
        isolated.append(envelope("event/e2", 2), 32, true),
        Ok(AppendOutcome::GapCreated { first_available: 1 })
    );
    assert_eq!(isolated.read(0), ReadOutcome::Gap { first_available: 1 });
    assert!(matches!(isolated.read(1), ReadOutcome::Event(_)));
    isolated.seal();
    assert_eq!(isolated.read(3), ReadOutcome::Sealed);
}

#[test]
fn provider_claims_and_append_crash_boundary_are_honest() {
    let contract = EventStreamContract {
        id: Id("stream/domain"),
        event_class: EventClass::Domain,
        payload_type: envelope("event/e0", 0).payload_type,
        retention: RetentionPolicy::DurableAppend {
            maximum_events: 10,
            maximum_bytes: 640,
            flush_ticks: 4,
        },
        subscriber_coupling: SubscriberCoupling::Isolated(flow(2)),
        delivery: ReplayDelivery::AtLeastOnce,
        maximum_publishers: 1,
        maximum_subscribers: 2,
        maximum_pending_operations: 2,
        maximum_projection_bytes: 128,
        provider: PinnedDescriptor {
            id: Id("provider/durable"),
            schema_version: 0,
            semantic_hash: hash(5),
        },
        recording_authority: Some(Id("grant/record")),
        sensitivity: Sensitivity::Restricted,
        terminal_evidence_required: true,
    };
    let embedded = EventProviderCapabilities {
        ephemeral: true,
        retained: true,
        durable: false,
        checkpoint_cursor: false,
        integrity: true,
        redaction: true,
        maximum_events: 10,
        maximum_bytes: 640,
        maximum_subscribers: 2,
        maximum_pending_operations: 2,
    };
    assert_eq!(
        validate_stream_contract(contract, embedded),
        Err(ResonanceError::ProviderIncapable)
    );
    assert_eq!(
        validate_stream_contract(
            contract,
            EventProviderCapabilities {
                durable: true,
                ..embedded
            }
        ),
        Ok(())
    );

    assert_eq!(
        AppendCommit::prepare().recover(),
        AppendRecovery::DiscardPartial
    );
    let mut committed = AppendCommit::prepare();
    committed.commit();
    assert_eq!(committed.recover(), AppendRecovery::ReplayCommitted);
}

#[test]
fn local_distributed_and_embedded_profiles_share_only_honest_capabilities() {
    let retained = EventStreamContract {
        id: Id("stream/domain"),
        event_class: EventClass::Domain,
        payload_type: envelope("event/e0", 0).payload_type,
        retention: RetentionPolicy::Ring {
            maximum_events: 2,
            maximum_bytes: 64,
        },
        subscriber_coupling: SubscriberCoupling::Isolated(flow(1)),
        delivery: ReplayDelivery::AtLeastOnce,
        maximum_publishers: 1,
        maximum_subscribers: 1,
        maximum_pending_operations: 1,
        maximum_projection_bytes: 64,
        provider: PinnedDescriptor {
            id: Id("provider/retained"),
            schema_version: 0,
            semantic_hash: hash(12),
        },
        recording_authority: Some(Id("grant/record")),
        sensitivity: Sensitivity::Restricted,
        terminal_evidence_required: true,
    };
    for profile in [
        EventProviderCapabilities {
            ephemeral: true,
            retained: true,
            durable: false,
            checkpoint_cursor: false,
            integrity: true,
            redaction: true,
            maximum_events: 2,
            maximum_bytes: 64,
            maximum_subscribers: 1,
            maximum_pending_operations: 1,
        },
        EventProviderCapabilities {
            ephemeral: true,
            retained: true,
            durable: false,
            checkpoint_cursor: true,
            integrity: true,
            redaction: true,
            maximum_events: 64,
            maximum_bytes: 4096,
            maximum_subscribers: 8,
            maximum_pending_operations: 8,
        },
        EventProviderCapabilities {
            ephemeral: false,
            retained: true,
            durable: false,
            checkpoint_cursor: false,
            integrity: true,
            redaction: true,
            maximum_events: 2,
            maximum_bytes: 64,
            maximum_subscribers: 1,
            maximum_pending_operations: 1,
        },
    ] {
        assert_eq!(validate_stream_contract(retained, profile), Ok(()));
    }
}

#[test]
fn required_evidence_and_control_authority_fail_closed() {
    let capacity = FlowCapacity::new(2, 64, 128).unwrap();
    let lossy = FlowPolicy::new(
        capacity,
        Pressure::Sample(SampleSchedule::new(2, 0).unwrap()),
        FlowWatermarks::new(0, 2, capacity).unwrap(),
    )
    .unwrap();
    let provider = EventProviderCapabilities {
        ephemeral: true,
        retained: true,
        durable: false,
        checkpoint_cursor: false,
        integrity: true,
        redaction: true,
        maximum_events: 2,
        maximum_bytes: 128,
        maximum_subscribers: 1,
        maximum_pending_operations: 1,
    };
    let required = EventStreamContract {
        id: Id("stream/evidence"),
        event_class: EventClass::NormativeEvidence,
        payload_type: envelope("event/e0", 0).payload_type,
        retention: RetentionPolicy::Ring {
            maximum_events: 2,
            maximum_bytes: 128,
        },
        subscriber_coupling: SubscriberCoupling::Isolated(lossy),
        delivery: ReplayDelivery::AtLeastOnce,
        maximum_publishers: 1,
        maximum_subscribers: 1,
        maximum_pending_operations: 1,
        maximum_projection_bytes: 64,
        provider: PinnedDescriptor {
            id: Id("provider/evidence"),
            schema_version: 0,
            semantic_hash: hash(13),
        },
        recording_authority: None,
        sensitivity: Sensitivity::Public,
        terminal_evidence_required: true,
    };
    assert_eq!(
        validate_stream_contract(required, provider),
        Err(ResonanceError::ProviderIncapable)
    );
    assert_eq!(
        validate_stream_contract(
            EventStreamContract {
                event_class: EventClass::Control,
                subscriber_coupling: SubscriberCoupling::Isolated(flow(1)),
                terminal_evidence_required: false,
                ..required
            },
            provider
        ),
        Err(ResonanceError::ProviderIncapable)
    );
}

#[test]
fn correction_is_append_only_and_prior_event_is_unchanged() {
    let original = ResonanceEnvelope {
        domain_time: Some((Id("clock/device"), 42)),
        ..envelope("event/original", 0)
    };
    let correction = ResonanceEnvelope {
        event: Id("event/correction"),
        sequence: 1,
        relations: ResonanceRelations {
            corrects: Some(original.event),
            ..original.relations
        },
        ..original
    };
    assert_eq!(original.relations.corrects, None);
    assert_eq!(correction.relations.corrects, Some(original.event));
    assert_eq!(correction.correlation, original.correlation);
    assert_eq!(correction.provenance, original.provenance);
    assert_eq!(correction.domain_time, original.domain_time);
    assert_eq!(correction.sensitivity, original.sensitivity);
    assert_eq!(correction.payload, original.payload);
}

#[test]
fn replay_and_projection_state_are_explicitly_bounded() {
    assert_eq!(
        validate_subscription(SubscriptionContract {
            id: Id("subscription/a"),
            stream: Id("stream/domain"),
            start: ReplayStart::Checkpoint(Id("checkpoint/a")),
            queue: flow(2),
            acknowledgement: true,
            maximum_unacknowledged: 2,
            cancellation_ticks: 4,
        }),
        Ok(())
    );
    let projection = ProjectionContract {
        id: Id("projection/a"),
        stream: Id("stream/domain"),
        logic: PinnedDescriptor {
            id: Id("projection/logic"),
            schema_version: 0,
            semantic_hash: hash(10),
        },
        snapshot_contract: PinnedDescriptor {
            id: Id("projection/snapshot"),
            schema_version: 0,
            semantic_hash: hash(11),
        },
        maximum_state_bytes: 128,
        maximum_rebuild_events: 16,
        gap_is_terminal: true,
    };
    assert_eq!(validate_projection(projection), Ok(()));
    assert_eq!(
        validate_projection_snapshot(
            projection,
            ProjectionSnapshot {
                projection: projection.id,
                stream: projection.stream,
                logic_hash: projection.logic.semantic_hash,
                cursor: 7,
                digest: ArtifactDigest::from_bytes([12; 32]),
                bytes: 64,
            }
        ),
        Ok(())
    );
}

#[test]
fn normative_resonance_fixture_inventory_is_owned_here() {
    let fixture = include_str!("../../../conformance/c4/resonance.json");
    for id in [
        "execution-event-compatible",
        "cord-value-without-publication",
        "explicit-domain-publication",
        "append-only-correction",
        "coupled-subscriber-pressure",
        "isolated-subscriber-pressure",
        "retained-ring-overflow-gap",
        "append-crash-before-commit",
        "append-crash-after-commit",
        "replay-exact-cursor",
        "replay-checkpoint-cursor",
        "redacted-stable-envelope",
        "projection-deterministic-rebuild",
        "durability-provider-rejected",
        "embedded-retained-not-durable",
        "terminal-evidence-survives-sampling",
    ] {
        assert!(fixture.contains(&format!("\"id\":\"{id}\"")), "{id}");
    }
}
