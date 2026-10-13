use super::cost_tests::{costs_with_condition, hosts, instant, plan as cost_plan, revision};
use super::waiting_tests::queued;
use super::*;
use crate::shared_pool_runtime::*;

fn policy_pool(
    plan: &Plan,
    now: conduit_core::MonotonicInstant,
    policy: PoolWaitingPolicy,
) -> HostedSharedPool<2, 256, 4> {
    let mut pool = HostedSharedPool::new(
        plan,
        0,
        plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    pool.enable_waiting_with_policy(
        PoolWaitingLimits {
            operations: 32,
            per_requester: 32,
            admitted_bytes: 128,
        },
        policy,
        now,
    )
    .unwrap();
    pool
}
fn attempt(
    pool: &mut HostedSharedPool<2, 256, 4>,
    plan: &Plan,
    hosts: &[conduit_core::HostAdvertisement],
) -> QueuedCostAdmission {
    let now = instant(&hosts[0], 15);
    let operation = pool
        .next_waiting_operation(&now)
        .unwrap()
        .unwrap()
        .operation
        .clone();
    let costs = costs_with_condition(plan, hosts, &operation, None);
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
    pool.admit_next_waiting_with_costs(
        &now,
        &current,
        format!("queue/{operation:?}").as_str().into(),
        &costs,
        &evaluations,
    )
    .unwrap()
    .unwrap()
}
fn serve(
    pool: &mut HostedSharedPool<2, 256, 4>,
    plan: &Plan,
    hosts: &[conduit_core::HostAdvertisement],
) -> QueuedCostAdmission {
    let result = attempt(pool, plan, hosts);
    let SharedPoolAdmissionOutcome::Selected { member, .. } = &result.outcome.admission else {
        panic!("expected occupation");
    };
    pool.trigger(*member).unwrap();
    pool.release(*member).unwrap();
    result
}
#[test]
fn interactive_arrivals_cannot_starve_an_older_batch_request() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = policy_pool(
        &plan,
        now.clone(),
        PoolWaitingPolicy::InteractiveWithAging {
            maximum_bypasses: 2,
        },
    );
    let batch = queued(&plan, &hosts[0], 1, "batch");
    pool.queue_operation(batch.clone(), &now).unwrap();
    for index in [2, 3] {
        let mut interactive = queued(&plan, &hosts[0], index, "interactive");
        interactive.class = PoolWaitingClass::Interactive;
        pool.queue_operation(interactive.clone(), &now).unwrap();
        let result = serve(&mut pool, &plan, &hosts);
        assert_eq!(result.retired, Some(interactive));
        assert_eq!(result.decision.reason, PoolWaitingChoiceReason::Interactive);
    }
    let mut next = queued(&plan, &hosts[0], 4, "interactive");
    next.class = PoolWaitingClass::Interactive;
    pool.queue_operation(next, &now).unwrap();
    let result = serve(&mut pool, &plan, &hosts);
    assert_eq!(result.retired, Some(batch.clone()));
    assert_eq!(result.decision.reason, PoolWaitingChoiceReason::Aged);
    assert_eq!(
        result
            .decision
            .candidates
            .iter()
            .find(|c| c.operation == batch.operation)
            .unwrap()
            .bypasses,
        2
    );
    assert_eq!(pool.waiting_population(), 1);
    assert_eq!(pool.population(), 0);
}
#[test]
fn requester_turns_are_explicit_and_fifo_remains_the_reference() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    for (policy, expected) in [
        (PoolWaitingPolicy::Fifo, vec![1, 2, 3, 4]),
        (
            PoolWaitingPolicy::RequesterTurnWithAging {
                maximum_bypasses: 4,
            },
            vec![1, 3, 2, 4],
        ),
    ] {
        let mut pool = policy_pool(&plan, now.clone(), policy);
        for (index, requester) in [(1, "a"), (2, "a"), (3, "b"), (4, "c")] {
            pool.queue_operation(queued(&plan, &hosts[0], index, requester), &now)
                .unwrap();
        }
        let mut actual = Vec::new();
        let mut reasons = Vec::new();
        while pool.waiting_population() > 0 {
            let result = serve(&mut pool, &plan, &hosts);
            actual.push(result.retired.unwrap().key.0[0]);
            reasons.push(result.decision.reason);
        }
        assert_eq!(actual, expected);
        if policy != PoolWaitingPolicy::Fifo {
            assert_eq!(reasons[1], PoolWaitingChoiceReason::RequesterTurn);
        } else {
            assert!(reasons.iter().all(|r| *r == PoolWaitingChoiceReason::Fifo));
        }
        assert_eq!(pool.plan_id(), &plan.plan_id);
    }
}
#[test]
fn pressure_refusals_do_not_age_the_queue_or_release_pinned_members() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = policy_pool(
        &plan,
        now.clone(),
        PoolWaitingPolicy::InteractiveWithAging {
            maximum_bypasses: 2,
        },
    );
    let mut pinned = Vec::new();
    for index in [1, 2] {
        pool.queue_operation(queued(&plan, &hosts[0], index, "active"), &now)
            .unwrap();
        let result = attempt(&mut pool, &plan, &hosts);
        let SharedPoolAdmissionOutcome::Selected { member, .. } = result.outcome.admission else {
            panic!("occupation expected");
        };
        pool.trigger(member).unwrap();
        pinned.push(member);
    }
    let batch = queued(&plan, &hosts[0], 3, "batch");
    pool.queue_operation(batch.clone(), &now).unwrap();
    let mut interactive = queued(&plan, &hosts[0], 4, "interactive");
    interactive.class = PoolWaitingClass::Interactive;
    pool.queue_operation(interactive.clone(), &now).unwrap();
    for _ in 0..4 {
        let result = attempt(&mut pool, &plan, &hosts);
        assert!(matches!(
            result.outcome.admission,
            SharedPoolAdmissionOutcome::Refused(_)
        ));
        assert!(result.retired.is_none());
        assert_eq!(result.decision.operation, interactive.operation);
        assert!(result.decision.candidates.iter().all(|c| c.bypasses == 0));
    }
    assert_eq!(pool.population(), 2);
    assert_eq!(pool.waiting_population(), 2);
    pool.release(pinned[0]).unwrap();
    let result = serve(&mut pool, &plan, &hosts);
    assert_eq!(result.retired, Some(interactive));
    assert_eq!(pool.population(), 1);
    assert_eq!(
        pool.cancel_waiting_operation(&batch.operation).unwrap(),
        batch
    );
    assert_eq!(pool.population(), 1);
    assert_eq!(pinned[1].placement.realization, 0);
}
#[test]
fn worst_case_aging_is_bounded_even_with_replenished_interactive_work() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = policy_pool(
        &plan,
        now.clone(),
        PoolWaitingPolicy::InteractiveWithAging {
            maximum_bypasses: 2,
        },
    );
    for index in 0..32 {
        pool.queue_operation(queued(&plan, &hosts[0], index, "batch"), &now)
            .unwrap();
    }
    let victim = "waiting/31".into();
    let mut selected_victim = None;
    for service in 0..34 {
        let result = serve(&mut pool, &plan, &hosts);
        let operation = result.retired.unwrap();
        if operation.operation == victim {
            selected_victim = Some(service);
            break;
        }
        let mut interactive = queued(&plan, &hosts[0], (32 + service) as u8, "interactive");
        interactive.class = PoolWaitingClass::Interactive;
        pool.queue_operation(interactive, &now).unwrap();
    }
    assert!(selected_victim.is_some_and(|i| i <= 2 + 31));
    assert_eq!(pool.population(), 0);
}
#[test]
fn bypass_budget_must_be_finite_and_nonzero() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    for policy in [
        PoolWaitingPolicy::InteractiveWithAging {
            maximum_bypasses: 0,
        },
        PoolWaitingPolicy::RequesterTurnWithAging {
            maximum_bypasses: 33,
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
            pool.enable_waiting_with_policy(
                PoolWaitingLimits {
                    operations: 2,
                    per_requester: 1,
                    admitted_bytes: 8
                },
                policy,
                now.clone()
            ),
            Err(PoolWaitingError::InvalidLimits)
        );
        assert_eq!(pool.waiting_population(), 0);
    }
}

#[test]
fn enabled_waiting_cannot_be_bypassed_by_direct_admission() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = policy_pool(
        &plan,
        now.clone(),
        PoolWaitingPolicy::InteractiveWithAging {
            maximum_bypasses: 2,
        },
    );
    let operation = queued(&plan, &hosts[0], 1, "first");
    pool.queue_operation(operation.clone(), &now).unwrap();
    let costs = costs_with_condition(&plan, &hosts, &operation.operation, None);
    assert!(matches!(
        pool.admit_new_operation_with_costs(
            operation.operation.clone(),
            operation.key,
            &[],
            "bypass/cost".into(),
            operation.demand,
            &costs,
            &[]
        ),
        Err(SharedPoolRuntimeError::WaitingAdmissionRequired)
    ));
    assert_eq!(
        pool.admit_new_operation(
            operation.operation.clone(),
            operation.key,
            &[],
            "bypass/reference".into()
        ),
        Err(SharedPoolRuntimeError::WaitingAdmissionRequired)
    );
    assert_eq!(pool.population(), 0);
    assert_eq!(pool.waiting_population(), 1);
    assert_eq!(serve(&mut pool, &plan, &hosts).retired, Some(operation));
}
