use conduit_core::{
    BlockingFairness, BoundedFlowQueue, CancellationOutcome, CancellationRegistration,
    CancellationScope, CordLifecycle, CordState, FlowCapacity, FlowEventKind, FlowOffer,
    FlowPolicy, FlowQueueState, FlowTypeFacts, FlowWatermarks, Id, LifecycleError,
    LifecycleMachine, LifecycleState, ManagedSubject, Pressure, ReplicaIdentity,
    ReplicaPoolContract, ReplicaState, StopPolicy, SubjectState, SupervisionPolicy, TerminalCause,
    TerminalCauseCode, TerminalClass, TraitProof, cancel_scope, cord_transition_allowed,
    derive_composite, managed_transition_allowed, replica_transition_allowed, resolve_terminal,
};

const MANAGED: [LifecycleState; 8] = [
    LifecycleState::Created,
    LifecycleState::Preparing,
    LifecycleState::Ready,
    LifecycleState::Running,
    LifecycleState::Draining,
    LifecycleState::Succeeded,
    LifecycleState::Cancelled,
    LifecycleState::Failed,
];
const CORD: [CordState; 8] = [
    CordState::Created,
    CordState::Prepared,
    CordState::Open,
    CordState::Draining,
    CordState::Completed,
    CordState::Cancelled,
    CordState::Failed,
    CordState::Disconnected,
];

fn expected_managed(from: LifecycleState, to: LifecycleState) -> bool {
    matches!(
        (from, to),
        (LifecycleState::Created, LifecycleState::Preparing)
            | (LifecycleState::Preparing, LifecycleState::Ready)
            | (LifecycleState::Ready, LifecycleState::Running)
            | (LifecycleState::Running, LifecycleState::Draining)
            | (LifecycleState::Draining, LifecycleState::Succeeded)
    ) || (!from.is_terminal() && matches!(to, LifecycleState::Cancelled | LifecycleState::Failed))
}

fn expected_cord(from: CordState, to: CordState) -> bool {
    matches!(
        (from, to),
        (CordState::Created, CordState::Prepared)
            | (CordState::Prepared, CordState::Open)
            | (CordState::Open, CordState::Draining)
            | (CordState::Draining, CordState::Completed)
    ) || (!from.is_terminal()
        && matches!(
            to,
            CordState::Cancelled | CordState::Failed | CordState::Disconnected
        ))
}

#[test]
fn every_legal_and_illegal_transition_is_exact() {
    for from in MANAGED {
        for to in MANAGED {
            assert_eq!(
                managed_transition_allowed(from, to),
                expected_managed(from, to),
                "{from:?} -> {to:?}"
            );
        }
    }
    for from in CORD {
        for to in CORD {
            assert_eq!(
                cord_transition_allowed(from, to),
                expected_cord(from, to),
                "{from:?} -> {to:?}"
            );
        }
    }

    let fixture = include_str!("../../../conformance/c2/lifecycle.tsv");
    assert!(fixture.contains("node\tcreated\tpreparing\taccepted"));
    assert!(fixture.contains("composite\tsucceeded\trunning\trejected\tCND-LIF-001"));
    assert!(fixture.contains("run\tcreated\trunning\trejected\tCND-LIF-001"));
    assert!(fixture.contains("cord\tcompleted\topen\trejected\tCND-LIF-001"));
}

#[test]
fn lifecycle_evidence_is_complete_and_ordered() {
    for kind in [
        ManagedSubject::Node,
        ManagedSubject::Composite,
        ManagedSubject::Run,
    ] {
        let mut machine = LifecycleMachine::new(Id("fixture"), kind);
        let mut events = [
            machine.transition(LifecycleState::Preparing, None).unwrap(),
            machine.transition(LifecycleState::Ready, None).unwrap(),
            machine.transition(LifecycleState::Running, None).unwrap(),
            machine.transition(LifecycleState::Draining, None).unwrap(),
            machine.transition(LifecycleState::Succeeded, None).unwrap(),
        ];
        for (sequence, event) in events.iter_mut().enumerate() {
            assert_eq!(event.sequence, sequence as u64);
        }
        assert_eq!(machine.state(), LifecycleState::Succeeded);
        assert_eq!(
            machine.transition(LifecycleState::Running, None),
            Err(LifecycleError::IllegalTransition)
        );
    }

    let mut cord = CordLifecycle::new(Id("fixture/cord"));
    cord.transition(CordState::Prepared, None).unwrap();
    cord.transition(CordState::Open, None).unwrap();
    let event = cord.transition(CordState::Draining, None).unwrap();
    assert_eq!(event.from, SubjectState::Cord(CordState::Open));
    cord.transition(CordState::Completed, None).unwrap();
    assert_eq!(cord.state(), CordState::Completed);

    let mut invalid = LifecycleMachine::new(Id("invalid"), ManagedSubject::Node);
    assert_eq!(
        invalid.transition(
            LifecycleState::Cancelled,
            Some(TerminalCauseCode::NaturalCompletion)
        ),
        Err(LifecycleError::InvalidTerminalCause)
    );
}

#[test]
fn cancellation_is_hierarchical_bounded_and_idempotent() {
    let root = CancellationScope {
        id: Id("root"),
        parent: None,
        deadline_ticks: 10,
        stop: StopPolicy::Drain,
    };
    let child = CancellationScope {
        id: Id("child"),
        parent: Some(Id("root")),
        deadline_ticks: 4,
        stop: StopPolicy::Abort,
    };
    let sibling = CancellationScope {
        id: Id("sibling"),
        parent: Some(Id("root")),
        deadline_ticks: 6,
        stop: StopPolicy::Drain,
    };
    let mut registrations = [
        CancellationRegistration::new(Id("root-node"), root),
        CancellationRegistration::new(Id("child-node"), child),
        CancellationRegistration::new(Id("sibling-node"), sibling),
    ];
    let mut deliveries = [None; 3];

    assert_eq!(
        cancel_scope(
            &mut registrations,
            Id("child"),
            TerminalCauseCode::CancellationRequested,
            100,
            &mut deliveries,
        ),
        Ok(CancellationOutcome::Delivered(1))
    );
    let isolated = deliveries[0].unwrap();
    assert_eq!(isolated.sequence, 0);
    assert_eq!(isolated.resource, Id("child-node"));
    assert_eq!(isolated.deadline_tick, 104);
    assert_eq!(isolated.stop, StopPolicy::Abort);
    assert_eq!(
        cancel_scope(
            &mut registrations,
            Id("child"),
            TerminalCauseCode::CancellationRequested,
            101,
            &mut deliveries,
        ),
        Ok(CancellationOutcome::Repeated)
    );

    let mut parent_deliveries = [None; 2];
    assert_eq!(
        cancel_scope(
            &mut registrations,
            Id("root"),
            TerminalCauseCode::AuthorityRevoked,
            200,
            &mut parent_deliveries,
        ),
        Ok(CancellationOutcome::Delivered(2))
    );
    assert_eq!(parent_deliveries[0].unwrap().resource, Id("root-node"));
    assert_eq!(parent_deliveries[1].unwrap().resource, Id("sibling-node"));
    assert_eq!(
        parent_deliveries[1].unwrap().reason,
        TerminalCauseCode::ParentCancelled
    );
    assert_eq!(
        parent_deliveries[1].unwrap().caused_by_reason,
        Some(TerminalCauseCode::AuthorityRevoked)
    );
}

fn cause(
    code: TerminalCauseCode,
    subject: &'static str,
    stop: StopPolicy,
) -> TerminalCause<'static> {
    TerminalCause {
        code,
        subject: Id(subject),
        caused_by: None,
        stop,
    }
}

#[test]
fn terminal_races_have_one_order_independent_answer_and_retain_causes() {
    let complete = cause(
        TerminalCauseCode::NaturalCompletion,
        "fixture/source",
        StopPolicy::Drain,
    );
    let failure = cause(
        TerminalCauseCode::NodeFailed,
        "fixture/worker",
        StopPolicy::Abort,
    );
    let authority = cause(
        TerminalCauseCode::AuthorityRevoked,
        "fixture/grant",
        StopPolicy::Abort,
    );
    let mut retained_a = [None; 3];
    let mut retained_b = [None; 3];
    let a = resolve_terminal(&[complete, failure, authority], &mut retained_a).unwrap();
    let b = resolve_terminal(&[authority, complete, failure], &mut retained_b).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.class, TerminalClass::Failed);
    assert_eq!(a.primary.code, TerminalCauseCode::NodeFailed);
    assert_eq!(a.queue, StopPolicy::Abort);
    assert_eq!(retained_a, retained_b);

    let mut source_retained = [None; 1];
    let source = resolve_terminal(&[complete], &mut source_retained).unwrap();
    assert_eq!(source.class, TerminalClass::Succeeded);
    assert_eq!(source.queue, StopPolicy::Drain);
    for (left, right, winner, class) in [
        (
            TerminalCauseCode::NaturalCompletion,
            TerminalCauseCode::CancellationRequested,
            TerminalCauseCode::CancellationRequested,
            TerminalClass::Cancelled,
        ),
        (
            TerminalCauseCode::DeadlineExpired,
            TerminalCauseCode::AuthorityRevoked,
            TerminalCauseCode::AuthorityRevoked,
            TerminalClass::Cancelled,
        ),
        (
            TerminalCauseCode::TransportDisconnected,
            TerminalCauseCode::DeadlineExpired,
            TerminalCauseCode::DeadlineExpired,
            TerminalClass::Cancelled,
        ),
        (
            TerminalCauseCode::NaturalCompletion,
            TerminalCauseCode::TransportDisconnected,
            TerminalCauseCode::TransportDisconnected,
            TerminalClass::Disconnected,
        ),
    ] {
        let pair = [
            cause(left, "fixture/left", StopPolicy::Abort),
            cause(right, "fixture/right", StopPolicy::Abort),
        ];
        let reverse = [pair[1], pair[0]];
        let mut retained_pair = [None; 2];
        let mut retained_reverse = [None; 2];
        let resolved = resolve_terminal(&pair, &mut retained_pair).unwrap();
        let reversed = resolve_terminal(&reverse, &mut retained_reverse).unwrap();
        assert_eq!(resolved, reversed);
        assert_eq!(resolved.primary.code, winner);
        assert_eq!(resolved.class, class);
        assert_eq!(retained_pair, retained_reverse);
    }
    assert!(
        include_str!("../../../conformance/c2/terminal-races.tsv")
            .contains("failure-during-drain\tnatural-completion,node-failed\tfailed\tabort")
    );
}

#[test]
fn nested_composites_derive_like_primitives() {
    let inner = derive_composite(
        &[LifecycleState::Succeeded, LifecycleState::Succeeded],
        &[CordState::Completed],
    );
    assert_eq!(inner, LifecycleState::Succeeded);
    let outer = derive_composite(&[inner, LifecycleState::Draining], &[CordState::Draining]);
    assert_eq!(outer, LifecycleState::Draining);
    assert_eq!(
        derive_composite(&[outer, LifecycleState::Failed], &[CordState::Cancelled]),
        LifecycleState::Failed
    );
}

#[test]
fn replicated_children_have_finite_admission_restart_and_cleanup() {
    let policy = ReplicaPoolContract {
        max_queued: 8,
        max_active: 2,
        supervision: SupervisionPolicy::Restart {
            max_attempts: 3,
            backoff_ticks: 5,
        },
    };
    assert_eq!(policy.validate(), Ok(()));
    let first = ReplicaIdentity::new(Id("fixture/template"), 0, 1).unwrap();
    let second = first.restart(policy).unwrap();
    let third = second.restart(policy).unwrap();
    assert_eq!(third.attempt, 3);
    assert_eq!(
        third.restart(policy),
        Err(LifecycleError::RestartBudgetExhausted)
    );
    assert!(replica_transition_allowed(
        ReplicaState::Template,
        ReplicaState::QueuedAdmission
    ));
    assert!(replica_transition_allowed(
        ReplicaState::Cleanup,
        ReplicaState::Attempt
    ));
    assert!(!replica_transition_allowed(
        ReplicaState::Succeeded,
        ReplicaState::Attempt
    ));
    assert_eq!(
        ReplicaPoolContract {
            max_queued: 0,
            max_active: 1,
            supervision: SupervisionPolicy::Isolate,
        }
        .validate(),
        Err(LifecycleError::UnboundedReplicaPool)
    );
}

fn queue<'a>(slots: &'a mut [Option<(u8, u32)>; 2]) -> BoundedFlowQueue<'a, 'static, u8> {
    let capacity = FlowCapacity::new(2, 8, 16).unwrap();
    BoundedFlowQueue::new(
        slots,
        FlowPolicy::new(
            capacity,
            Pressure::Block(BlockingFairness::Fifo),
            FlowWatermarks::new(0, 2, capacity).unwrap(),
        )
        .unwrap(),
        FlowTypeFacts {
            disposable: TraitProof::Disproven,
            coalescers: Some(&[]),
        },
    )
    .unwrap()
}

#[test]
fn natural_completion_drains_and_abort_returns_every_queued_value() {
    let mut drain_slots = [None, None];
    let mut draining = queue(&mut drain_slots);
    draining.offer(
        1,
        FlowOffer {
            size_bytes: 1,
            coalesce_target: None,
        },
    );
    draining.offer(
        2,
        FlowOffer {
            size_bytes: 1,
            coalesce_target: None,
        },
    );
    assert!(draining.complete_source().iter().any(|event| matches!(
        event.kind,
        FlowEventKind::DrainStarted {
            terminal: FlowQueueState::Completed
        }
    )));
    assert_eq!(draining.state(), FlowQueueState::Draining);
    assert_eq!(draining.pop().value, Some(1));
    let final_pop = draining.pop();
    assert_eq!(final_pop.value, Some(2));
    assert!(
        final_pop
            .events
            .iter()
            .any(|event| event.kind == FlowEventKind::Completed)
    );
    assert_eq!(draining.state(), FlowQueueState::Completed);

    let mut abort_slots = [None, None];
    let mut aborting = queue(&mut abort_slots);
    for value in [3, 4] {
        aborting.offer(
            value,
            FlowOffer {
                size_bytes: 2,
                coalesce_target: None,
            },
        );
    }
    let mut discarded = [None, None];
    let events = aborting.cancel_abort(&mut discarded).unwrap();
    assert_eq!(discarded, [Some(3), Some(4)]);
    assert!(events.iter().any(|event| matches!(
        event.kind,
        FlowEventKind::ValuesDiscardedOnAbort { items: 2, bytes: 4 }
    )));
    assert_eq!(aborting.state(), FlowQueueState::Cancelled);
    assert_eq!(aborting.occupancy_items(), 0);

    let mut rejected_slots = [None, None];
    let mut rejected = queue(&mut rejected_slots);
    for value in [5, 6] {
        rejected.offer(
            value,
            FlowOffer {
                size_bytes: 1,
                coalesce_target: None,
            },
        );
    }
    let mut too_small = [None];
    assert!(rejected.cancel_abort(&mut too_small).is_err());
    assert_eq!(rejected.state(), FlowQueueState::Active);
    assert_eq!(rejected.occupancy_items(), 2);
    rejected.complete_source();
    assert_eq!(rejected.state(), FlowQueueState::Draining);
    let mut racing_discarded = [None, None];
    let race_events = rejected
        .terminate(
            TerminalClass::Failed,
            StopPolicy::Abort,
            &mut racing_discarded,
        )
        .unwrap();
    assert_eq!(racing_discarded, [Some(5), Some(6)]);
    assert_eq!(rejected.state(), FlowQueueState::Failed);
    assert_eq!(
        race_events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        vec![
            FlowEventKind::ValuesDiscardedOnAbort { items: 2, bytes: 2 },
            FlowEventKind::Failed,
        ]
    );

    let mut waiting_slots = [None, None];
    let mut waiting = queue(&mut waiting_slots);
    assert_eq!(waiting.pop().value, None);
    assert!(waiting.cancel_drain().iter().any(|event| matches!(
        event.kind,
        FlowEventKind::Cancelled {
            wake_consumer: true,
            ..
        }
    )));
}
