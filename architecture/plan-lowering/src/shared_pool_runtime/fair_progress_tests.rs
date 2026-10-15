//! Modeled cost error, finite contention, and owner-confirmed lost-member retirement.
use super::cost_tests::{hosts, instant, plan as cost_plan};
use super::waiting_policy_tests::{attempt, attempt_current, policy_pool};
use super::waiting_tests::queued;
use super::*;
use crate::shared_pool_runtime::*;

#[test]
fn unrelated_requester_progresses_while_the_predicted_fast_worker_stays_occupied() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = policy_pool(
        &plan,
        now.clone(),
        PoolWaitingPolicy::RequesterTurnWithAging {
            maximum_bypasses: 2,
        },
    );
    let first = queued(&plan, &hosts[0], 0, "heavy");
    pool.queue_operation(first.clone(), &now).unwrap();
    let admitted = attempt(&mut pool, &plan, &hosts);
    let SharedPoolAdmissionOutcome::Selected { member: fast, .. } = admitted.outcome.admission
    else {
        panic!("expected member");
    };
    assert_eq!(fast.placement.realization, 1);
    pool.trigger(fast).unwrap();
    // The model says this worker is faster, but hold its real admitted slot.
    // This is deterministic occupation evidence, not measured inference latency.
    for index in 1..32 {
        pool.queue_operation(queued(&plan, &hosts[0], index, "heavy"), &now)
            .unwrap();
    }
    let other = queued(&plan, &hosts[0], 32, "other");
    pool.queue_operation(other.clone(), &now).unwrap();
    assert_eq!(pool.waiting_population(), 32);
    let admitted = attempt(&mut pool, &plan, &hosts);
    assert_eq!(admitted.retired, Some(other));
    assert_eq!(
        admitted.decision.reason,
        PoolWaitingChoiceReason::RequesterTurn
    );
    let SharedPoolAdmissionOutcome::Selected { member: slow, .. } = admitted.outcome.admission
    else {
        panic!("expected fallback capacity");
    };
    assert_eq!(slow.placement.realization, 0);
    pool.trigger(slow).unwrap();
    assert_eq!(pool.population(), 2);
    for _ in 0..4 {
        let refused = attempt(&mut pool, &plan, &hosts);
        assert!(matches!(
            refused.outcome.admission,
            SharedPoolAdmissionOutcome::Refused(_)
        ));
        assert!(refused.retired.is_none());
        assert_eq!(pool.population(), 2);
        assert_eq!(pool.waiting_population(), 31);
    }
    pool.cancel_waiting_operation(&queued(&plan, &hosts[0], 1, "heavy").operation)
        .unwrap();
    assert_eq!(pool.population(), 2);
    pool.release(slow).unwrap();
    let admitted = attempt(&mut pool, &plan, &hosts);
    let SharedPoolAdmissionOutcome::Selected { member: next, .. } = admitted.outcome.admission
    else {
        panic!("expected progress");
    };
    assert_eq!(next.placement.realization, 0);
    assert_ne!(next.epoch, slow.epoch);
    pool.trigger(next).unwrap();
    assert!(
        pool.trigger(fast).is_err(),
        "in-flight effect cannot be triggered twice"
    );
    assert!(
        pool.release(slow).is_err(),
        "stale epoch cannot free the new member"
    );
    pool.release(next).unwrap();
    pool.release(fast).unwrap();
    assert_eq!(pool.population(), 0);
    assert_eq!(pool.plan_id(), &plan.plan_id);
}

#[test]
fn provider_loss_holds_capacity_until_owner_confirms_retirement() {
    let plan = cost_plan();
    let hosts = hosts(&plan);
    let now = instant(&hosts[0], 15);
    let mut pool = policy_pool(&plan, now.clone(), PoolWaitingPolicy::Fifo);
    let first = queued(&plan, &hosts[0], 1, "a");
    pool.queue_operation(first.clone(), &now).unwrap();
    let admitted = attempt(&mut pool, &plan, &hosts);
    let SharedPoolAdmissionOutcome::Selected { member: lost, .. } = admitted.outcome.admission
    else {
        panic!("expected member");
    };
    assert_eq!(lost.placement.realization, 1);
    pool.trigger(lost).unwrap();
    let observations = [
        observation(0, PoolRealizationHealth::Ready, 2),
        observation(1, PoolRealizationHealth::Unavailable, 2),
    ];
    let evidence = pool
        .provider_lost(first.operation, lost, &observations, "fair/loss".into())
        .unwrap();
    assert_eq!(evidence.disposition, PoolSelectionDisposition::ProviderLost);
    assert_eq!(pool.population(), 1);
    assert!(pool.trigger(lost).is_err());
    assert!(evidence
        .exhaustion_replan_events(
            &plan,
            "coordinator".into(),
            "coordinator/boot-1".into(),
            PlanningRequestAuthority::HostLocal,
            "fair/no-replay".into()
        )
        .is_err());
    pool.queue_operation(queued(&plan, &hosts[0], 2, "b"), &now)
        .unwrap();
    let admitted = attempt_current(&mut pool, &plan, &hosts, &observations);
    let SharedPoolAdmissionOutcome::Selected { member: other, .. } = admitted.outcome.admission
    else {
        panic!("expected surviving worker");
    };
    assert_eq!(other.placement.realization, 0);
    pool.trigger(other).unwrap();
    assert_eq!(pool.population(), 2);
    assert!(
        pool.complete_release(other).is_err(),
        "active work has not retired"
    );
    assert_eq!(pool.population(), 2);
    pool.complete_release(lost).unwrap();
    assert!(
        pool.complete_release(lost).is_err(),
        "retirement completes only once"
    );
    assert_eq!(pool.population(), 1);
    pool.release(other).unwrap();
    assert_eq!(pool.population(), 0);
    pool.queue_operation(queued(&plan, &hosts[0], 3, "c"), &now)
        .unwrap();
    let recovered = [
        observation(0, PoolRealizationHealth::Ready, 3),
        observation(1, PoolRealizationHealth::Ready, 3),
    ];
    let admitted = attempt_current(&mut pool, &plan, &hosts, &recovered);
    let SharedPoolAdmissionOutcome::Selected { member: next, .. } = admitted.outcome.admission
    else {
        panic!("expected newly admitted epoch");
    };
    assert_eq!(next.placement.realization, lost.placement.realization);
    assert_ne!(next.epoch, lost.epoch);
    assert!(
        pool.complete_release(lost).is_err(),
        "stale retirement cannot free new work"
    );
    assert_eq!(pool.population(), 1);
    pool.trigger(next).unwrap();
    pool.release(next).unwrap();
    assert_eq!(pool.population(), 0);
    assert_eq!(pool.plan_id(), &plan.plan_id);
}
