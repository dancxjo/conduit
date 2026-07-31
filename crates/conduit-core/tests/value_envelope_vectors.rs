use conduit_core::{
    AuthorityTime, BlockingFairness, ClockRounding, Direction, FeedbackBoundaryKind,
    FeedbackInitialization, FeedbackReplayGapPolicy, FeedbackRuntimePhase, FeedbackRuntimeState,
    FeedbackTerminalPolicy, FlowCapacity, FlowPolicy, FlowWatermarks, Id, InstancePath,
    PinnedDescriptor, PlanClockConversion, PlanFeedbackBoundary, Pressure, ResolvedPlanCord,
    ResolvedPlanPort, SemanticHash, Sensitivity, TypeContractRef, ValueEnvelope,
    ValueEnvelopePolicy, ValueEnvelopeReason, ValueTimestamp, convert_clock,
    validate_feedback_boundary, validate_feedback_graph, validate_value_envelope,
};
use std::collections::BTreeSet;

const FIXTURE: &str = include_str!("../../../conformance/c5/value-envelope-clock-feedback.json");

const REPRESENTATION: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("fixture/bytes-v1"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([1; 32]),
};

const CANCELLATION: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("fixture/bounded-cancellation"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([2; 32]),
};

const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 0,
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
fn every_envelope_bound_and_authorization_failure_is_exact() {
    let clocks = [Id("clock/device")];
    let valid = ValueEnvelope {
        representation: REPRESENTATION,
        payload_bytes: 8,
        envelope_bytes: 16,
        fragment_count: 1,
        fragment_bytes: 8,
        identity: None,
        correlation: None,
        causation: None,
        provenance: None,
        timestamps: &[],
        sensitivity: Sensitivity::Public,
    };
    assert_eq!(
        validate_value_envelope(
            ValueEnvelopePolicy {
                maximum_envelope_bytes: 0,
                ..policy(&clocks)
            },
            valid,
        ),
        Err(ValueEnvelopeReason::InvalidBound)
    );
    assert_eq!(
        validate_value_envelope(
            policy(&clocks),
            ValueEnvelope {
                fragment_count: 5,
                ..valid
            },
        ),
        Err(ValueEnvelopeReason::InvalidBound)
    );
    assert_eq!(
        validate_value_envelope(
            policy(&clocks),
            ValueEnvelope {
                representation: PinnedDescriptor {
                    semantic_hash: SemanticHash::from_bytes([9; 32]),
                    ..REPRESENTATION
                },
                ..valid
            },
        ),
        Err(ValueEnvelopeReason::RepresentationMismatch)
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
            },
            3,
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
            },
            3,
        ),
        Err(ValueEnvelopeReason::StaleClockConversion)
    );
    assert_eq!(
        convert_clock(
            conversion,
            50,
            AuthorityTime {
                basis: Id("clock/host"),
                tick: 15,
            },
            2,
        ),
        Err(ValueEnvelopeReason::InvalidClockConversion)
    );
    assert_eq!(
        convert_clock(
            conversion,
            50,
            AuthorityTime {
                basis: Id("clock/unrelated"),
                tick: 15,
            },
            3,
        ),
        Err(ValueEnvelopeReason::StaleClockConversion)
    );
    assert_eq!(
        convert_clock(
            PlanClockConversion {
                numerator: u64::MAX,
                ..conversion
            },
            i64::MAX,
            AuthorityTime {
                basis: Id("clock/host"),
                tick: 15,
            },
            3,
        ),
        Err(ValueEnvelopeReason::ClockArithmeticOverflow)
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
    assert_eq!(
        validate_feedback_boundary(PlanFeedbackBoundary {
            maximum_retained_items: 0,
            ..state
        }),
        Err(ValueEnvelopeReason::InvalidFeedbackBoundary)
    );
    assert_eq!(
        validate_feedback_boundary(PlanFeedbackBoundary {
            initialization: FeedbackInitialization::InitialValue,
            initial_items: 0,
            initial_bytes: 0,
            ..state
        }),
        Err(ValueEnvelopeReason::InvalidFeedbackPolicy)
    );
}

#[test]
fn feedback_runtime_orders_initialization_replay_cancellation_and_terminal_state() {
    let state = PlanFeedbackBoundary {
        id: Id("feedback/state"),
        node: InstancePath::new("state").unwrap(),
        cord: Id("cord/feedback"),
        kind: FeedbackBoundaryKind::State,
        initialization: FeedbackInitialization::InitialValue,
        initial_items: 1,
        initial_bytes: 8,
        maximum_retained_items: 2,
        maximum_retained_bytes: 64,
        delay_ticks: 0,
        clock: None,
        replay_gap: FeedbackReplayGapPolicy::Wait,
        cancellation: CANCELLATION,
        terminal: FeedbackTerminalPolicy::DropRetained,
    };

    let mut cancelled_during_initialization = FeedbackRuntimeState::NEW;
    cancelled_during_initialization.cancel();
    assert_eq!(
        cancelled_during_initialization.initialize(state),
        Err(ValueEnvelopeReason::InvalidFeedbackPolicy)
    );
    assert_eq!(
        cancelled_during_initialization,
        FeedbackRuntimeState {
            phase: FeedbackRuntimePhase::Cancelled,
            retained_items: 0,
            retained_bytes: 0,
        }
    );

    let mut runtime = FeedbackRuntimeState::NEW;
    runtime.initialize(state).unwrap();
    assert!(runtime.may_emit());
    runtime.replay_gap(state).unwrap();
    assert_eq!(runtime.phase, FeedbackRuntimePhase::WaitingForReplay);
    assert!(!runtime.may_emit());
    runtime.resume_replay().unwrap();
    runtime.cancel();
    runtime.finish(state).unwrap();
    assert_eq!(runtime.phase, FeedbackRuntimePhase::Cancelled);
    assert_eq!(runtime.retained_items, 0);
    assert_eq!(runtime.retained_bytes, 0);
    assert!(!runtime.may_emit());

    let drain = PlanFeedbackBoundary {
        terminal: FeedbackTerminalPolicy::DrainRetained,
        ..state
    };
    let mut terminal = FeedbackRuntimeState::NEW;
    terminal.initialize(drain).unwrap();
    assert_eq!(
        terminal.finish(drain),
        Err(ValueEnvelopeReason::InvalidFeedbackPolicy)
    );
    terminal.retain(drain, 0, 0).unwrap();
    terminal.finish(drain).unwrap();
    assert_eq!(terminal.phase, FeedbackRuntimePhase::Terminal);
    assert!(!terminal.may_emit());
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

#[test]
fn conformance_fixture_names_every_required_positive_and_negative_case() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 25);
    let ids = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for required in [
        "payload-only",
        "authorized-correlation-provenance",
        "identity-affecting-envelope-bounds",
        "local-envelope-preserved",
        "distributed-envelope-preserved",
        "replayed-envelope-preserved",
        "corrected-envelope-preserved",
        "fresh-clock-conversion",
        "finite-delay-feedback",
        "finite-state-feedback",
        "unbounded-envelope",
        "oversized-fragmented-envelope",
        "unknown-representation",
        "forbidden-sensitivity-widening",
        "unlisted-clock",
        "incomparable-clocks",
        "stale-clock-conversion",
        "clock-arithmetic-overflow",
        "uncertainty-above-consumer-ceiling",
        "zero-feedback-retention",
        "cycle-without-boundary",
        "missing-feedback-initialization",
        "replay-correlation-impersonation",
        "cancellation-during-feedback-initialization",
        "terminal-before-retained-value",
    ] {
        assert!(ids.contains(required), "fixture covers {required}");
    }
    for (id, reason) in [
        ("unbounded-envelope", ValueEnvelopeReason::InvalidBound),
        (
            "oversized-fragmented-envelope",
            ValueEnvelopeReason::InvalidBound,
        ),
        (
            "unknown-representation",
            ValueEnvelopeReason::RepresentationMismatch,
        ),
        (
            "forbidden-sensitivity-widening",
            ValueEnvelopeReason::SensitivityWidening,
        ),
        ("unlisted-clock", ValueEnvelopeReason::ClockNotAuthorized),
        (
            "incomparable-clocks",
            ValueEnvelopeReason::InvalidClockConversion,
        ),
        (
            "stale-clock-conversion",
            ValueEnvelopeReason::StaleClockConversion,
        ),
        (
            "clock-arithmetic-overflow",
            ValueEnvelopeReason::ClockArithmeticOverflow,
        ),
        (
            "uncertainty-above-consumer-ceiling",
            ValueEnvelopeReason::InvalidClockConversion,
        ),
        (
            "zero-feedback-retention",
            ValueEnvelopeReason::InvalidFeedbackBoundary,
        ),
        (
            "cycle-without-boundary",
            ValueEnvelopeReason::InvalidFeedbackCycle,
        ),
        (
            "missing-feedback-initialization",
            ValueEnvelopeReason::InvalidFeedbackPolicy,
        ),
    ] {
        let case = cases
            .iter()
            .find(|case| case["id"] == id)
            .expect("required fixture");
        assert_eq!(case["expected"]["code"], reason.code(), "{id}");
    }
}
