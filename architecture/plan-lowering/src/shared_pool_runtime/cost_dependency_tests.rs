//! Cached projections cannot outlive independently current producer dependencies.
use super::super::*;
use super::cost_tests::{costs_with_condition, hosts, instant, plan, revision};
use super::{key, observation};
use alloc::vec::Vec;
use conduit_core::*;
use conduit_kernel::NodeId;

#[test]
fn changed_calibration_or_execution_refuses_before_atomic_occupation() {
    let plan = plan();
    let hosts = hosts(&plan);
    let operation = PoolOperationId::from("cost/request-current");
    let warm = CostExecutionCondition::Warm {
        residency_revision: 7,
    };
    let prepared = costs_with_condition(&plan, &hosts, &operation, Some(warm));
    let observations = hosts
        .iter()
        .enumerate()
        .map(|(i, _)| observation(i as u8, PoolRealizationHealth::Ready, 1))
        .collect::<Vec<_>>();
    let calibrations = [revision()];
    let changed = [PoolCostCalibrationRevision::new("cost/test-calibration".into(), 2).unwrap()];
    let now = hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let observed = hosts.iter().map(|h| instant(h, 12)).collect::<Vec<_>>();
    let expires = hosts.iter().map(|h| instant(h, 18)).collect::<Vec<_>>();
    let signs = [
        SignId::from("execution/current-0"),
        SignId::from("execution/current-1"),
    ];
    let make_executions = |condition| {
        hosts
            .iter()
            .enumerate()
            .map(|(i, host)| PoolCostExecutionEvaluation {
                host,
                offer: &host.capabilities[0],
                resource: None,
                calibration: &calibrations[0],
                sign: &signs[i],
                observed: &observed[i],
                expires: &expires[i],
                condition,
            })
            .collect::<Vec<_>>()
    };
    let executions = make_executions(warm);
    let cold = make_executions(CostExecutionCondition::Cold);
    let mut pool = HostedSharedPool::<2, 32, 4>::new(
        &plan,
        0,
        plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    for (revisions, state, expected) in [
        (
            &changed[..],
            &executions[..],
            PoolCostPreparationRefusal::CalibrationChanged,
        ),
        (
            &calibrations[..],
            &[][..],
            PoolCostPreparationRefusal::MissingExecutionObservation,
        ),
        (
            &calibrations[..],
            &cold[..],
            PoolCostPreparationRefusal::ExecutionConditionChanged,
        ),
    ] {
        let evaluations = hosts
            .iter()
            .zip(&now)
            .map(|(host, now)| PoolCostEvaluation {
                host,
                now,
                calibrations: revisions,
                executions: state,
            })
            .collect::<Vec<_>>();
        assert!(
            matches!(pool.admit_new_operation_with_costs(operation.clone(), key(1), &observations,
            "select/dependency-refusal".into(), [9; 32], &prepared, &evaluations),
            Err(SharedPoolRuntimeError::Cost(PoolCostLoweringError::Prepared(e))) if e == expected)
        );
        assert_eq!(pool.population(), 0);
    }
    // Reusing the same projection is legal when fresh independent sources match.
    let evaluations = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations,
            executions: &executions,
        })
        .collect::<Vec<_>>();
    let selected = pool
        .admit_new_operation_with_costs(
            operation,
            key(1),
            &observations,
            "select/current-dependencies".into(),
            [9; 32],
            &prepared,
            &evaluations,
        )
        .unwrap();
    assert!(
        matches!(&selected.admission, SharedPoolAdmissionOutcome::Selected { member, evidence }
        if member.placement.realization == 1 && evidence.observation_sign_ids.contains(&signs[1]))
    );
    assert_eq!(selected.sources[1].calibration, calibrations[0]);
    assert_eq!(
        selected.sources[1].current_execution.as_ref().unwrap().sign,
        signs[1]
    );
    assert_eq!(pool.population(), 1);
}

#[test]
fn duplicate_catalogue_or_expired_execution_refuses_a_fresh_duration() {
    let plan = plan();
    let hosts = hosts(&plan);
    let operation = PoolOperationId::from("cost/request-bounded");
    let prepared = costs_with_condition(
        &plan,
        &hosts,
        &operation,
        Some(CostExecutionCondition::ResidentObserved),
    );
    let calibrations = [revision(), revision()];
    let now = hosts.iter().map(|h| instant(h, 18)).collect::<Vec<_>>();
    let observed = hosts.iter().map(|h| instant(h, 12)).collect::<Vec<_>>();
    let expires = hosts.iter().map(|h| instant(h, 18)).collect::<Vec<_>>();
    let sign = SignId::from("execution/resident-current");
    let executions = hosts
        .iter()
        .enumerate()
        .map(|(i, host)| PoolCostExecutionEvaluation {
            host,
            offer: &host.capabilities[0],
            resource: None,
            calibration: &calibrations[0],
            sign: &sign,
            observed: &observed[i],
            expires: &expires[i],
            condition: CostExecutionCondition::ResidentObserved,
        })
        .collect::<Vec<_>>();
    let mut pool = HostedSharedPool::<2, 32, 4>::new(
        &plan,
        0,
        plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    let duplicate = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations,
            executions: &executions,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        pool.admit_new_operation_with_costs(
            operation.clone(),
            key(1),
            &[],
            "select/duplicate".into(),
            [9; 32],
            &prepared,
            &duplicate
        ),
        Err(SharedPoolRuntimeError::Cost(
            PoolCostLoweringError::DuplicateCalibration
        ))
    ));
    assert_eq!(pool.population(), 0);
    let expired = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations[..1],
            executions: &executions,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        pool.admit_new_operation_with_costs(
            operation,
            key(1),
            &[],
            "select/expired-execution".into(),
            [9; 32],
            &prepared,
            &expired
        ),
        Err(SharedPoolRuntimeError::Cost(
            PoolCostLoweringError::Prepared(PoolCostPreparationRefusal::Stale)
        ))
    ));
    assert_eq!(pool.population(), 0);
}
