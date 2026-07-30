use conduit_core::{
    AuthorityTime, BlockingFairness, ClockRounding, Direction, FeedbackBoundaryKind,
    FeedbackInitialization, FeedbackReplayGapPolicy, FeedbackTerminalPolicy, FlowCapacity,
    FlowPolicy, FlowWatermarks, Id, InstancePath, PinnedDescriptor, PlanClockConversion,
    PlanFeedbackBoundary, Pressure, ResolvedPlanCord, ResolvedPlanPort, SemanticHash, Sensitivity,
    TypeContractRef, ValueEnvelope, ValueEnvelopePolicy, ValueEnvelopeReason, ValueTimestamp,
    convert_clock, validate_feedback_boundary, validate_feedback_graph, validate_value_envelope,
};

const REPRESENTATION: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("fixture/bytes-v1"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([1; 32]),
};

const CANCELLATION: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("fixture/bounded-cancellation"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([2; 32]),
};

const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([8; 32]),
};

fn policy<'a>(clock_domains: &'a [Id<'a>]) -> ValueEnvelopePolicy<'a> {
    ValueEnvelopePolicy {
        cord: Id("cord/value"),
        representation: REPRESENTATION,
        maximum_payload_bytes: 1024,
        maximum_envelope_bytes: 256,
        maximum_fragments: 4,
        maximum_fragment_bytes: 256,
        maximum_timestamps: 2,
        clock_domains,
        identity_allowed: true,
        correlation_allowed: true,
        causation_allowed: true,
        provenance_allowed: true,
        sensitivity_ceiling: Sensitivity::Restricted,
    }
}

#[test]
fn authorized_bounded_metadata_is_accepted() {
    let clocks = [Id("clock/device"), Id("clock/host")];
    let timestamps = [ValueTimestamp {
        domain: clocks[0],
        tick: 42,
        uncertainty_ticks: 1,
    }];
    let envelope = ValueEnvelope {
        representation: REPRESENTATION,
        payload_bytes: 512,
        envelope_bytes: 96,
        fragment_count: 2,
        fragment_bytes: 512,
        identity: Some(Id("value/42")),
        correlation: Some(Id("request/7")),
        causation: Some(Id("value/41")),
        provenance: Some(Id("sensor/front")),
        timestamps: &timestamps,
        sensitivity: Sensitivity::Restricted,
    };

    validate_value_envelope(policy(&clocks), envelope).unwrap();
}

#[test]
fn unauthorized_clock_and_sensitivity_fail_closed() {
    let clocks = [Id("clock/device")];
    let timestamps = [ValueTimestamp {
        domain: Id("clock/ambient-wall"),
        tick: 42,
        uncertainty_ticks: 0,
    }];
    let envelope = ValueEnvelope {
        representation: REPRESENTATION,
        payload_bytes: 8,
        envelope_bytes: 16,
        fragment_count: 1,
        fragment_bytes: 8,
        identity: None,
        correlation: None,
        causation: None,
        provenance: None,
        timestamps: &timestamps,
        sensitivity: Sensitivity::Public,
    };
    assert_eq!(
        validate_value_envelope(policy(&clocks), envelope),
        Err(ValueEnvelopeReason::ClockNotAuthorized)
    );

    let secret = ValueEnvelope {
        timestamps: &[],
        sensitivity: Sensitivity::Secret,
        ..envelope
    };
    assert_eq!(
        validate_value_envelope(policy(&clocks), secret),
        Err(ValueEnvelopeReason::SensitivityWidening)
    );
}

#[test]
fn clock_conversion_preserves_uncertainty_and_validity() {
    let conversion = PlanClockConversion {
        id: Id("conversion/device-host"),
        source: Id("clock/device"),
        destination: Id("clock/host"),
        numerator: 2,
        denominator: 1,
        offset_ticks: -5,
        rounding: ClockRounding::Exact,
        maximum_uncertainty_ticks: 3,
        observed_at: AuthorityTime {
            basis: Id("clock/host"),
            tick: 10,
        },
        valid_until_tick: 20,
        authority: Id("host/front-sensor"),
    };
    assert_eq!(
        convert_clock(
            conversion,
            50,
            AuthorityTime {
                basis: Id("clock/host"),
                tick: 15,
            }
        )
        .unwrap(),
        conduit_core::ConvertedTime {
            domain_tick: 95,
            earliest_tick: 92,
            latest_tick: 98,
        }
    );
    assert_eq!(
        convert_clock(
            conversion,
            50,
            AuthorityTime {
                basis: Id("clock/host"),
                tick: 21,
            }
        ),
        Err(ValueEnvelopeReason::StaleClockConversion)
    );
}

#[test]
fn feedback_boundaries_require_real_delay_or_finite_state() {
    let delay = PlanFeedbackBoundary {
        id: Id("feedback/delay"),
        node: InstancePath::new("delay").unwrap(),
        cord: Id("cord/feedback"),
        kind: FeedbackBoundaryKind::Delay,
        initialization: FeedbackInitialization::Empty,
        initial_items: 0,
        initial_bytes: 0,
        maximum_retained_items: 1,
        maximum_retained_bytes: 1024,
        delay_ticks: 1,
        clock: Some(Id("clock/scheduler")),
        replay_gap: FeedbackReplayGapPolicy::Fail,
        cancellation: CANCELLATION,
        terminal: FeedbackTerminalPolicy::DropRetained,
    };
    validate_feedback_boundary(delay).unwrap();

    let instantaneous = PlanFeedbackBoundary {
        delay_ticks: 0,
        ..delay
    };
    assert_eq!(
        validate_feedback_boundary(instantaneous),
        Err(ValueEnvelopeReason::InvalidFeedbackBoundary)
    );

    let state = PlanFeedbackBoundary {
        id: Id("feedback/state"),
        node: InstancePath::new("state").unwrap(),
        kind: FeedbackBoundaryKind::State,
        initialization: FeedbackInitialization::InitialValue,
        initial_items: 1,
        initial_bytes: 8,
        delay_ticks: 0,
        clock: None,
        replay_gap: FeedbackReplayGapPolicy::Reset,
        terminal: FeedbackTerminalPolicy::DrainRetained,
        ..delay
    };
    validate_feedback_boundary(state).unwrap();
}

#[test]
fn feedback_edges_are_the_only_admitted_cycle_breaks() {
    let nodes = [
        InstancePath::new("source").unwrap(),
        InstancePath::new("state").unwrap(),
    ];
    let capacity = FlowCapacity::new(1, 64, 64).unwrap();
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, 1, capacity).unwrap(),
    )
    .unwrap();
    let cord = |id, from, to| ResolvedPlanCord {
        id: Id::new(id).unwrap(),
        from: ResolvedPlanPort {
            node: InstancePath::new(from).unwrap(),
            port: Id::new("out").unwrap(),
            direction: Direction::Output,
            port_contract_hash: SemanticHash::from_bytes([8; 32]),
            value_type: TYPE,
        },
        to: ResolvedPlanPort {
            node: InstancePath::new(to).unwrap(),
            port: Id::new("in").unwrap(),
            direction: Direction::Input,
            port_contract_hash: SemanticHash::from_bytes([8; 32]),
            value_type: TYPE,
        },
        flow,
        queue_memory_bytes: 64,
    };
    let cords = [
        cord("forward", "source", "state"),
        cord("feedback", "state", "source"),
    ];
    let boundary = PlanFeedbackBoundary {
        id: Id::new("feedback-boundary").unwrap(),
        node: nodes[1],
        cord: cords[1].id,
        kind: FeedbackBoundaryKind::State,
        initialization: FeedbackInitialization::InitialValue,
        initial_items: 1,
        initial_bytes: 8,
        maximum_retained_items: 1,
        maximum_retained_bytes: 64,
        delay_ticks: 0,
        clock: None,
        replay_gap: FeedbackReplayGapPolicy::Fail,
        cancellation: CANCELLATION,
        terminal: FeedbackTerminalPolicy::DropRetained,
    };
    let mut scratch = [false; 2];

    assert_eq!(
        validate_feedback_graph(&nodes, &cords, &[], &mut scratch),
        Err(ValueEnvelopeReason::InvalidFeedbackCycle)
    );
    assert_eq!(
        validate_feedback_graph(&nodes, &cords, &[boundary], &mut scratch),
        Ok(())
    );
}
