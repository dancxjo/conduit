use super::cost_tests::{costs_with_condition, hosts, instant, plan as cost_plan, revision};
use super::*;
use crate::shared_pool_runtime::*;

fn queued(
    plan: &Plan,
    host: &conduit_core::HostAdvertisement,
    index: u8,
    requester: &str,
) -> QueuedPoolOperation {
    QueuedPoolOperation {
        plan: plan.plan_id.clone(),
        pool: plan.fragments[0].shared_pools[0].pool_id.clone(),
        operation: format!("waiting/{index}").as_str().into(),
        requester: requester.into(),
        key: key(index),
        demand: [9; 32],
        admitted_bytes: 4,
        waiting_expires: instant(host, 20),
    }
}
fn waiting(plan: &Plan, now: conduit_core::MonotonicInstant) -> HostedSharedPool<2, 64, 4> {
    let mut pool = HostedSharedPool::new(
        plan,
        0,
        plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    pool.enable_waiting(
        PoolWaitingLimits {
            operations: 3,
            per_requester: 2,
            admitted_bytes: 12,
        },
        now,
    )
    .unwrap();
    pool
}
#[test]
fn finite_waiting_refuses_scope_duplicates_requester_and_byte_pressure_without_occupation() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = waiting(&plan, now.clone());
    let oversized = queued(&plan, &hosts[0], 99, &"x".repeat(257));
    assert_eq!(
        pool.queue_operation(oversized, &now),
        Err(PoolWaitingError::InvalidIdentity)
    );
    let a = queued(&plan, &hosts[0], 1, "a");
    pool.queue_operation(a.clone(), &now).unwrap();
    assert_eq!(
        pool.queue_operation(a.clone(), &now),
        Err(PoolWaitingError::Duplicate)
    );
    let mut changed = queued(&plan, &hosts[0], 2, "a");
    changed.plan = "foreign-plan".into();
    assert_eq!(
        pool.queue_operation(changed, &now),
        Err(PoolWaitingError::Correlation)
    );
    pool.queue_operation(queued(&plan, &hosts[0], 2, "a"), &now)
        .unwrap();
    assert_eq!(
        pool.queue_operation(queued(&plan, &hosts[0], 3, "a"), &now),
        Err(PoolWaitingError::RequesterFull)
    );
    let mut huge = queued(&plan, &hosts[0], 3, "b");
    huge.admitted_bytes = 5;
    assert_eq!(
        pool.queue_operation(huge, &now),
        Err(PoolWaitingError::ByteLimit)
    );
    pool.queue_operation(queued(&plan, &hosts[0], 3, "b"), &now)
        .unwrap();
    assert_eq!(
        pool.queue_operation(queued(&plan, &hosts[0], 4, "c"), &now),
        Err(PoolWaitingError::Full)
    );
    assert_eq!(pool.waiting_population(), 3);
    assert_eq!(pool.population(), 0);
    assert_eq!(pool.cancel_waiting_operation(&a.operation).unwrap(), a);
    pool.queue_operation(queued(&plan, &hosts[0], 4, "c"), &now)
        .unwrap();
    assert_eq!(
        pool.next_waiting_operation(&now).unwrap().unwrap().key,
        key(2)
    );
}
#[test]
fn expiry_and_clock_refusals_keep_exact_custody_and_cancel_never_releases_a_member() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = waiting(&plan, now.clone());
    let a = queued(&plan, &hosts[0], 1, "a");
    pool.queue_operation(a.clone(), &now).unwrap();
    assert_eq!(
        pool.next_waiting_operation(&instant(&hosts[1], 15)),
        Err(PoolWaitingError::ClockChanged)
    );
    assert_eq!(
        pool.retire_expired_waiting(&instant(&hosts[0], 14)),
        Err(PoolWaitingError::ClockRegressed)
    );
    assert_eq!(
        pool.next_waiting_operation(&instant(&hosts[0], 20)),
        Err(PoolWaitingError::Expired)
    );
    assert_eq!(pool.waiting_population(), 1);
    assert_eq!(pool.population(), 0);
    assert_eq!(
        pool.retire_expired_waiting(&instant(&hosts[0], 20))
            .unwrap(),
        Some(a.clone())
    );
    assert_eq!(
        pool.queue_operation(a, &instant(&hosts[0], 20)),
        Err(PoolWaitingError::Expired)
    );
    assert_eq!(pool.waiting_population(), 0);
    assert_eq!(
        pool.cancel_waiting_operation(&"absent".into()),
        Err(PoolWaitingError::Missing)
    );
}
#[test]
fn queued_cost_requests_use_atomic_admission_keep_capacity_wait_and_never_replay_pinned_members() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = waiting(&plan, now.clone());
    let current = vec![
        observation(0, PoolRealizationHealth::Ready, 1),
        observation(1, PoolRealizationHealth::Ready, 1),
    ];
    let clocks = hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let revisions = [revision()];
    let evaluations = hosts
        .iter()
        .zip(&clocks)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &revisions,
            executions: &[],
        })
        .collect::<Vec<_>>();
    let a = queued(&plan, &hosts[0], 1, "a");
    let b = queued(&plan, &hosts[0], 2, "a");
    let c = queued(&plan, &hosts[0], 3, "b");
    for op in [&a, &b, &c] {
        pool.queue_operation(op.clone(), &now).unwrap();
    }
    let wrong = costs_with_condition(&plan, &hosts, &b.operation, None);
    assert!(pool
        .admit_next_waiting_with_costs(&now, &current, "waiting/wrong".into(), &wrong, &evaluations)
        .is_err());
    assert_eq!(pool.population(), 0);
    assert_eq!(pool.waiting_population(), 3);
    let mut pinned = Vec::new();
    for (operation, expected) in [(&a, 1), (&b, 0)] {
        let costs = costs_with_condition(&plan, &hosts, &operation.operation, None);
        let selected = pool
            .admit_next_waiting_with_costs(
                &now,
                &current,
                "waiting/selected".into(),
                &costs,
                &evaluations,
            )
            .unwrap()
            .unwrap();
        assert_eq!(selected.retired.as_ref(), Some(operation));
        let SharedPoolAdmissionOutcome::Selected { member, .. } = selected.outcome.admission else {
            panic!("selection expected");
        };
        assert_eq!(member.placement.realization, expected);
        pool.trigger(member).unwrap();
        pinned.push(member);
        assert_eq!(
            pool.cancel_waiting_operation(&operation.operation),
            Err(PoolWaitingError::Missing)
        );
    }
    let costs = costs_with_condition(&plan, &hosts, &c.operation, None);
    let refused = pool
        .admit_next_waiting_with_costs(&now, &current, "waiting/full".into(), &costs, &evaluations)
        .unwrap()
        .unwrap();
    assert!(matches!(
        refused.outcome.admission,
        SharedPoolAdmissionOutcome::Refused(_)
    ));
    assert!(refused.retired.is_none());
    assert_eq!(pool.waiting_population(), 1);
    assert_eq!(pool.population(), 2);
    pool.release(pinned[0]).unwrap();
    let selected = pool
        .admit_next_waiting_with_costs(
            &now,
            &current,
            "waiting/reused".into(),
            &costs,
            &evaluations,
        )
        .unwrap()
        .unwrap();
    assert_eq!(selected.retired, Some(c));
    let SharedPoolAdmissionOutcome::Selected { member, .. } = selected.outcome.admission else {
        panic!("selection expected");
    };
    assert_eq!(member.placement.realization, 1);
    assert_ne!(member.epoch, pinned[0].epoch);
    assert_eq!(pinned[1].placement.realization, 0);
    assert_eq!(pool.plan_id(), &plan.plan_id);
    assert_eq!(pool.waiting_population(), 0);
    assert_eq!(pool.population(), 2);
}

#[test]
fn admitted_storage_has_an_exact_32_operation_boundary_and_exhausted_envelope_returns_custody() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    for limits in [
        PoolWaitingLimits {
            operations: 0,
            per_requester: 1,
            admitted_bytes: 1,
        },
        PoolWaitingLimits {
            operations: 33,
            per_requester: 1,
            admitted_bytes: 1,
        },
        PoolWaitingLimits {
            operations: 2,
            per_requester: 3,
            admitted_bytes: 1,
        },
        PoolWaitingLimits {
            operations: 2,
            per_requester: 1,
            admitted_bytes: 65_537,
        },
    ] {
        let mut pool = HostedSharedPool::<2, 64, 4>::new(
            &plan,
            0,
            plan.fragments[0].shared_pools[0].pool_id.clone(),
            NodeId(10),
            1,
        )
        .unwrap();
        assert_eq!(
            pool.enable_waiting(limits, now.clone()),
            Err(PoolWaitingError::InvalidLimits)
        );
        assert_eq!(pool.waiting_population(), 0);
    }
    let mut pool = HostedSharedPool::<2, 64, 4>::new(
        &plan,
        0,
        plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    pool.enable_waiting(
        PoolWaitingLimits {
            operations: 32,
            per_requester: 1,
            admitted_bytes: 32,
        },
        now.clone(),
    )
    .unwrap();
    for i in 0..32 {
        let mut operation = queued(&plan, &hosts[0], i, &format!("requester/{i}"));
        operation.admitted_bytes = 1;
        pool.queue_operation(operation, &now).unwrap();
    }
    assert_eq!(pool.waiting_population(), 32);
    assert_eq!(
        pool.queue_operation(queued(&plan, &hosts[0], 32, "last"), &now),
        Err(PoolWaitingError::Full)
    );
    let operation = pool.next_waiting_operation(&now).unwrap().unwrap().clone();
    let costs = costs_with_condition(&plan, &hosts, &operation.operation, None);
    let revisions = [revision()];
    let clocks = hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let evaluations = hosts
        .iter()
        .zip(&clocks)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &revisions,
            executions: &[],
        })
        .collect::<Vec<_>>();
    let current = vec![
        observation(0, PoolRealizationHealth::Unavailable, 1),
        observation(1, PoolRealizationHealth::Unavailable, 1),
    ];
    let exhausted = pool
        .admit_next_waiting_with_costs(
            &now,
            &current,
            "waiting/exhausted".into(),
            &costs,
            &evaluations,
        )
        .unwrap()
        .unwrap();
    assert_eq!(exhausted.retired, Some(operation));
    let SharedPoolAdmissionOutcome::Refused(evidence) = exhausted.outcome.admission else {
        panic!("exhausted envelope must refuse");
    };
    assert_eq!(
        evidence.disposition,
        PoolSelectionDisposition::EnvelopeExhausted
    );
    assert_eq!(pool.waiting_population(), 31);
    assert_eq!(pool.population(), 0);
    assert_eq!(pool.plan_id(), &plan.plan_id);
}
