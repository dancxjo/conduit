use conduit_core::{
    InstancePath, PlanResourceBudget, PoolAdmissionDisposition, PoolAdmissionFacts,
    PoolAdmissionPolicy, PoolCleanupPolicy, PoolContract, PoolController, PoolError,
    PoolFailureDisposition, PoolGeneration, PoolGenerationReservation, PoolReason,
    PoolReservationProfile, PoolSlotState, PoolSupervisionPolicy, PoolWorkIdentity, SemanticHash,
    select_fair_pool,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn reservation() -> PoolReservationProfile {
    PoolReservationProfile {
        resources: PlanResourceBudget {
            memory_bytes: 1_024,
            storage_bytes: 128,
            cpu_units: 2,
            timers: 3,
            transports: 1,
            checkpoints: 1,
            evidence_bytes: 512,
        },
        child_nodes: 2,
        child_cords: 1,
        state_bytes: 256,
        scheduler_slots: 4,
        host_operations: 1,
        cancellation_scopes: 3,
    }
}

fn contract<'a>(
    admission: PoolAdmissionPolicy,
    supervision: PoolSupervisionPolicy<'a>,
    cleanup: PoolCleanupPolicy,
) -> PoolContract<'a> {
    let maximum_queued = u16::from(admission == PoolAdmissionPolicy::QueueBounded) * 2;
    PoolContract {
        pool: InstancePath::new("root/pool.workers").unwrap(),
        template_hash: hash(1),
        implementation_set_hash: hash(6),
        maximum_live: 2,
        maximum_queued,
        admission,
        supervision,
        cleanup,
        deadline_ticks: 100,
        idle_timeout_ticks: 20,
        cleanup_ticks: 5,
        reservation: reservation(),
        total_reservation: reservation().checked_mul(5).unwrap(),
        maximum_evidence_events: 64,
    }
}

fn generation() -> PoolGeneration {
    PoolGeneration {
        plan: hash(2),
        epoch: 7,
        generation: 3,
        template_hash: hash(1),
    }
}

fn work(byte: u8) -> PoolWorkIdentity {
    PoolWorkIdentity {
        request: hash(byte),
        work_unit: hash(byte.wrapping_add(40)),
        correlation: hash(byte.wrapping_add(80)),
    }
}

fn facts() -> PoolAdmissionFacts {
    PoolAdmissionFacts {
        authority_granted: true,
        sensitivity_allowed: true,
        template_hash: hash(1),
        implementation_set_hash: hash(6),
        available: reservation(),
    }
}

#[test]
fn admission_is_bounded_and_queue_has_no_hidden_overflow() {
    let mut pool = PoolController::<4, 64>::new(
        contract(
            PoolAdmissionPolicy::QueueBounded,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Drain,
        ),
        generation(),
    )
    .unwrap();

    assert_eq!(
        pool.offer(work(1), facts(), 0).unwrap(),
        PoolAdmissionDisposition::Started { slot: 0 }
    );
    assert_eq!(
        pool.offer(work(2), facts(), 0).unwrap(),
        PoolAdmissionDisposition::Started { slot: 1 }
    );
    assert_eq!(
        pool.offer(work(3), facts(), 0).unwrap(),
        PoolAdmissionDisposition::Queued { slot: 2 }
    );
    assert_eq!(
        pool.offer(work(4), facts(), 0).unwrap(),
        PoolAdmissionDisposition::Queued { slot: 3 }
    );
    assert_eq!(
        pool.offer(work(5), facts(), 0).unwrap(),
        PoolAdmissionDisposition::Rejected(PoolReason::QueueFull)
    );
    assert_eq!(pool.population().live, 2);
    assert_eq!(pool.population().queued, 2);
}

#[test]
fn reject_block_and_fail_are_distinct_without_implicit_queues() {
    let policies = [
        (
            PoolAdmissionPolicy::Reject,
            PoolAdmissionDisposition::Rejected(PoolReason::Capacity),
        ),
        (
            PoolAdmissionPolicy::Block,
            PoolAdmissionDisposition::Blocked,
        ),
        (
            PoolAdmissionPolicy::Fail,
            PoolAdmissionDisposition::Failed(PoolReason::AdmissionFailed),
        ),
    ];
    for (policy, expected) in policies {
        let mut pool = PoolController::<2, 64>::new(
            contract(
                policy,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            ),
            generation(),
        )
        .unwrap();
        pool.offer(work(1), facts(), 0).unwrap();
        pool.offer(work(2), facts(), 0).unwrap();
        assert_eq!(pool.offer(work(3), facts(), 0).unwrap(), expected);
        assert_eq!(pool.population().queued, 0);
        assert_eq!(pool.evidence().len(), 3);
        assert_eq!(pool.evidence()[2].from, PoolSlotState::Empty);
        assert_eq!(pool.evidence()[2].to, PoolSlotState::Empty);
        assert_eq!(
            pool.evidence()[2].reason,
            match policy {
                PoolAdmissionPolicy::Reject => PoolReason::Capacity,
                PoolAdmissionPolicy::Block => PoolReason::CallerBlocked,
                PoolAdmissionPolicy::Fail => PoolReason::AdmissionFailed,
                PoolAdmissionPolicy::QueueBounded => unreachable!(),
            }
        );
    }
}

#[test]
fn identity_depends_on_semantics_not_offer_order() {
    let mut left = PoolController::<4, 64>::new(
        contract(
            PoolAdmissionPolicy::QueueBounded,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Drain,
        ),
        generation(),
    )
    .unwrap();
    let mut right = PoolController::<4, 64>::new(
        contract(
            PoolAdmissionPolicy::QueueBounded,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Drain,
        ),
        generation(),
    )
    .unwrap();
    left.offer(work(10), facts(), 0).unwrap();
    left.offer(work(11), facts(), 0).unwrap();
    right.offer(work(11), facts(), 0).unwrap();
    right.offer(work(10), facts(), 0).unwrap();

    let left_ten = left
        .slots()
        .iter()
        .find(|slot| slot.request == hash(10))
        .unwrap()
        .identity;
    let right_ten = right
        .slots()
        .iter()
        .find(|slot| slot.request == hash(10))
        .unwrap()
        .identity;
    assert_eq!(left_ten, right_ten);
}

#[test]
fn identical_work_in_distinct_pools_has_distinct_identity() {
    let left_contract = contract(
        PoolAdmissionPolicy::Reject,
        PoolSupervisionPolicy::Isolate,
        PoolCleanupPolicy::Abort,
    );
    let right_contract = PoolContract {
        pool: InstancePath::new("root/pool.other").unwrap(),
        ..left_contract
    };
    let mut left = PoolController::<2, 64>::new(left_contract, generation()).unwrap();
    let mut right = PoolController::<2, 64>::new(right_contract, generation()).unwrap();
    let PoolAdmissionDisposition::Started { slot: left_slot } =
        left.offer(work(1), facts(), 0).unwrap()
    else {
        panic!("left starts");
    };
    let PoolAdmissionDisposition::Started { slot: right_slot } =
        right.offer(work(1), facts(), 0).unwrap()
    else {
        panic!("right starts");
    };
    assert_ne!(
        left.slots()[usize::from(left_slot)].identity.instance,
        right.slots()[usize::from(right_slot)].identity.instance
    );
}

#[test]
fn admission_denies_resource_authority_and_template_before_slot_mutation() {
    let mut pool = PoolController::<2, 64>::new(
        contract(
            PoolAdmissionPolicy::Reject,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Abort,
        ),
        generation(),
    )
    .unwrap();
    for (bad, reason) in [
        (
            PoolAdmissionFacts {
                authority_granted: false,
                ..facts()
            },
            PoolReason::AuthorityDenied,
        ),
        (
            PoolAdmissionFacts {
                sensitivity_allowed: false,
                ..facts()
            },
            PoolReason::SensitivityDenied,
        ),
        (
            PoolAdmissionFacts {
                template_hash: hash(99),
                ..facts()
            },
            PoolReason::ImplementationMismatch,
        ),
        (
            PoolAdmissionFacts {
                implementation_set_hash: hash(99),
                ..facts()
            },
            PoolReason::ImplementationMismatch,
        ),
        (
            PoolAdmissionFacts {
                available: PoolReservationProfile::default(),
                ..facts()
            },
            PoolReason::ReservationUnavailable,
        ),
    ] {
        assert_eq!(
            pool.offer(work(reason as u8 + 20), bad, 0).unwrap(),
            PoolAdmissionDisposition::Rejected(reason)
        );
        assert_eq!(pool.population().live, 0);
        assert_eq!(pool.evidence().last().unwrap().reason, reason);
    }
}

#[test]
fn restart_backoff_is_finite_and_attempt_identity_changes() {
    let mut pool = PoolController::<2, 64>::new(
        contract(
            PoolAdmissionPolicy::Reject,
            PoolSupervisionPolicy::RestartBounded {
                maximum_attempts: 2,
                backoff_ticks: 5,
            },
            PoolCleanupPolicy::Abort,
        ),
        generation(),
    )
    .unwrap();
    let PoolAdmissionDisposition::Started { slot } = pool.offer(work(1), facts(), 10).unwrap()
    else {
        panic!("expected start");
    };
    pool.mark_running(slot, 10).unwrap();
    let first = pool.slots()[usize::from(slot)].identity;
    assert_eq!(
        pool.fail(slot, hash(90), 11).unwrap(),
        PoolFailureDisposition::RestartAt {
            tick: 16,
            attempt: 2
        }
    );
    assert_eq!(
        pool.slots()[usize::from(slot)].state,
        PoolSlotState::RestartBackoff
    );
    pool.tick(15).unwrap();
    assert_eq!(
        pool.slots()[usize::from(slot)].state,
        PoolSlotState::RestartBackoff
    );
    pool.tick(16).unwrap();
    let second = pool.slots()[usize::from(slot)].identity;
    assert_eq!(second.attempt, 2);
    assert_ne!(first.correlation, second.correlation);
    pool.mark_running(slot, 16).unwrap();
    assert_eq!(
        pool.fail(slot, hash(91), 17).unwrap(),
        PoolFailureDisposition::RestartExhausted
    );
}

#[test]
fn deadlines_idle_cleanup_and_queue_cancellation_are_evidenced() {
    let mut pool = PoolController::<4, 64>::new(
        contract(
            PoolAdmissionPolicy::QueueBounded,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Drain,
        ),
        generation(),
    )
    .unwrap();
    let PoolAdmissionDisposition::Started { slot } = pool.offer(work(1), facts(), 0).unwrap()
    else {
        panic!("expected start");
    };
    pool.mark_running(slot, 0).unwrap();
    pool.tick(20).unwrap();
    assert_eq!(
        pool.slots()[usize::from(slot)].state,
        PoolSlotState::Cleanup
    );
    pool.tick(25).unwrap();
    assert_eq!(pool.slots()[usize::from(slot)].state, PoolSlotState::Failed);
    assert_eq!(pool.complete(slot, 26), Err(PoolError::IllegalTransition));
    assert_eq!(
        pool.checkpoint(slot, hash(1), 26),
        Err(PoolError::IllegalTransition)
    );

    pool.reclaim_terminal(slot).unwrap();
    pool.offer(work(2), facts(), 30).unwrap();
    pool.offer(work(3), facts(), 30).unwrap();
    let PoolAdmissionDisposition::Queued { slot: queued } =
        pool.offer(work(4), facts(), 30).unwrap()
    else {
        panic!("expected queue");
    };
    pool.cancel(queued, hash(77), 31).unwrap();
    assert_eq!(
        pool.slots()[usize::from(queued)].state,
        PoolSlotState::Cancelled
    );
    assert!(
        pool.evidence().iter().any(|event| {
            event.reason == PoolReason::Cancelled && event.cause == Some(hash(77))
        })
    );
}

#[test]
fn checkpoint_resume_requires_exact_template_identity() {
    let mut pool = PoolController::<2, 64>::new(
        contract(
            PoolAdmissionPolicy::Reject,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Drain,
        ),
        generation(),
    )
    .unwrap();
    let PoolAdmissionDisposition::Started { slot } = pool.offer(work(1), facts(), 0).unwrap()
    else {
        panic!("expected start");
    };
    pool.mark_running(slot, 0).unwrap();
    assert!(!pool.checkpoint(slot, hash(9), 1).unwrap());
    assert_eq!(
        pool.slots()[usize::from(slot)].state,
        PoolSlotState::Running
    );
    assert!(pool.checkpoint(slot, hash(1), 2).unwrap());
    assert_eq!(
        pool.slots()[usize::from(slot)].state,
        PoolSlotState::Checkpointing
    );
    pool.resume(slot, 3).unwrap();
}

#[test]
fn generation_overlap_includes_old_candidate_and_rollback() {
    let exact = PoolGenerationReservation {
        old_maximum_live: 2,
        candidate_maximum_live: 2,
        rollback_maximum_live: 1,
        reserved_slots: 5,
        per_instance: reservation(),
        reserved_resources: reservation().checked_mul(5).unwrap(),
    };
    assert_eq!(exact.validate(), Ok(()));
    assert_eq!(
        PoolGenerationReservation {
            reserved_slots: 4,
            ..exact
        }
        .validate(),
        Err(PoolError::GenerationOverlapExceeded)
    );
    assert_eq!(
        PoolGenerationReservation {
            reserved_slots: 6,
            reserved_resources: reservation().checked_mul(6).unwrap(),
            ..exact
        }
        .validate(),
        Err(PoolError::GenerationOverlapExceeded)
    );
}

#[test]
fn fairness_selection_wraps_without_registry_order() {
    let ready = [false, true, true, false];
    assert_eq!(select_fair_pool(&ready, 0), Some(1));
    assert_eq!(select_fair_pool(&ready, 2), Some(2));
    assert_eq!(select_fair_pool(&ready, 3), Some(1));
}

#[test]
fn storage_and_evidence_bounds_fail_closed() {
    assert!(matches!(
        PoolController::<1, 64>::new(
            contract(
                PoolAdmissionPolicy::QueueBounded,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Drain,
            ),
            generation()
        ),
        Err(PoolError::StorageTooSmall)
    ));
    assert!(matches!(
        PoolController::<4, 8>::new(
            contract(
                PoolAdmissionPolicy::QueueBounded,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Drain,
            ),
            generation()
        ),
        Err(PoolError::StorageTooSmall)
    ));
}

#[test]
fn terminal_slots_require_explicit_reclaim_and_extra_storage_is_invisible() {
    let mut pool = PoolController::<8, 64>::new(
        contract(
            PoolAdmissionPolicy::Reject,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Drain,
        ),
        generation(),
    )
    .unwrap();
    assert_eq!(pool.slots().len(), 2);

    for byte in [1, 2] {
        let PoolAdmissionDisposition::Started { slot } =
            pool.offer(work(byte), facts(), 0).unwrap()
        else {
            panic!("expected start");
        };
        pool.complete(slot, 1).unwrap();
    }
    pool.tick(6).unwrap();
    assert_eq!(pool.population().terminal, 2);
    assert_eq!(
        pool.offer(work(3), facts(), 7),
        Err(PoolError::ReservationDrift)
    );

    pool.reclaim_terminal(0).unwrap();
    assert_eq!(
        pool.offer(work(3), facts(), 8).unwrap(),
        PoolAdmissionDisposition::Started { slot: 0 }
    );
}

#[test]
fn foreign_usage_cannot_expand_the_declared_profile() {
    let mut pool = PoolController::<2, 64>::new(
        contract(
            PoolAdmissionPolicy::Reject,
            PoolSupervisionPolicy::Isolate,
            PoolCleanupPolicy::Abort,
        ),
        generation(),
    )
    .unwrap();
    let PoolAdmissionDisposition::Started { slot } = pool.offer(work(1), facts(), 0).unwrap()
    else {
        panic!("expected start");
    };
    pool.mark_running(slot, 0).unwrap();
    let mut excess = reservation();
    excess.host_operations += 1;
    assert!(!pool.observe_usage(slot, excess, 1).unwrap());
    assert_eq!(
        pool.slots()[usize::from(slot)].state,
        PoolSlotState::Cleanup
    );
    assert!(
        pool.evidence()
            .iter()
            .any(|event| event.reason == PoolReason::ForeignProfileExceeded)
    );
}
