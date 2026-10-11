//! Cached projections cannot outlive independently current producer dependencies.
use super::super::*;
use super::cost_tests::{costs_with_condition, hosts, instant, plan, revision};
use super::{key, observation};
use alloc::vec::Vec;
use conduit_core::*;
use conduit_kernel::NodeId;

#[test]
fn changed_dependencies_use_recorded_capacity_fallback() {
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
        let result = pool
            .admit_new_operation_with_costs(
                operation.clone(),
                key(1),
                &observations,
                "select/dependency-fallback".into(),
                [9; 32],
                &prepared,
                &evaluations,
            )
            .unwrap();
        assert_eq!(
            result.account.choice,
            Some(PoolCostChoiceBasis::CapacityReference)
        );
        assert!(result
            .account
            .sources
            .iter()
            .all(|source| source.availability
                == PoolCostSourceAvailability::Unavailable(PoolCostUnavailable::Prepared(
                    expected
                ))));
        result.account.validate(&plan).unwrap();
        let SharedPoolAdmissionOutcome::Selected { member, .. } = result.admission else {
            panic!("fresh hard evidence must permit capacity fallback");
        };
        assert_eq!(pool.population(), 1);
        pool.trigger(member).unwrap();
        pool.release(member).unwrap();
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
    assert_eq!(selected.account.sources[1].calibration, calibrations[0]);
    assert_eq!(
        selected.account.sources[1]
            .current_execution
            .as_ref()
            .unwrap()
            .sign,
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
    let result = pool
        .admit_new_operation_with_costs(
            operation,
            key(1),
            &[],
            "select/expired-execution".into(),
            [9; 32],
            &prepared,
            &expired,
        )
        .unwrap();
    assert!(matches!(
        result.admission,
        SharedPoolAdmissionOutcome::Refused { .. }
    ));
    assert!(result
        .account
        .sources
        .iter()
        .all(|source| source.availability
            == PoolCostSourceAvailability::Unavailable(PoolCostUnavailable::Prepared(
                PoolCostPreparationRefusal::Stale
            ))));
    result.account.validate(&plan).unwrap();
    assert_eq!(pool.population(), 0);
}

#[test]
fn unavailable_cheap_source_cannot_displace_a_fresh_estimate() {
    let plan = plan();
    let hosts = hosts(&plan);
    let operation = PoolOperationId::from("cost/request-mixed");
    let prepared = costs_with_condition(&plan, &hosts, &operation, None);
    let observations = (0..2)
        .map(|i| observation(i, PoolRealizationHealth::Ready, 1))
        .collect::<Vec<_>>();
    let now = [instant(&hosts[0], 15), instant(&hosts[1], 20)];
    let calibrations = [revision()];
    let evaluations = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations,
            executions: &[],
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
    let result = pool
        .admit_new_operation_with_costs(
            operation,
            key(1),
            &observations,
            "select/mixed".into(),
            [9; 32],
            &prepared,
            &evaluations,
        )
        .unwrap();
    assert_eq!(result.account.evidence.selected_realization, Some(0));
    assert_eq!(
        result.account.choice,
        Some(PoolCostChoiceBasis::MinimumPredictedCompletion)
    );
    assert_eq!(
        result.account.candidates[1].cost,
        PoolCostDisposition::Unavailable(PoolCostUnavailable::Prepared(
            PoolCostPreparationRefusal::Stale
        ))
    );
    result.account.validate(&plan).unwrap();
    let mut forged = result.account.clone();
    forged.sources[0].boot = "foreign/boot".into();
    assert_eq!(
        forged.validate(&plan),
        Err(PoolCostAccountRefusal::Correlation)
    );
    let mut forged = result.account.clone();
    forged.candidates[0].cost = PoolCostDisposition::Known {
        upper_nanoseconds: 0,
        source: 0,
    };
    assert_eq!(
        forged.validate(&plan),
        Err(PoolCostAccountRefusal::InconsistentDecision)
    );
    let mut forged = result.account.clone();
    forged.sources[0].evaluated_calibration = None;
    assert_eq!(
        forged.validate(&plan),
        Err(PoolCostAccountRefusal::InvalidSource)
    );
    let mut forged = result.account.clone();
    forged.sources[0].expected_execution = Some(CostExecutionCondition::Warm {
        residency_revision: 0,
    });
    assert_eq!(
        forged.validate(&plan),
        Err(PoolCostAccountRefusal::InvalidSource)
    );
    let mut forged = result.account.clone();
    forged
        .evidence
        .observation_sign_ids
        .retain(|s| s != &forged.sources[0].sign);
    assert_eq!(
        forged.validate(&plan),
        Err(PoolCostAccountRefusal::Correlation)
    );
}

#[test]
fn missing_context_is_recorded_without_inventing_a_zero_cost() {
    let plan = plan();
    let hosts = hosts(&plan);
    let operation = PoolOperationId::from("cost/request-missing");
    let prepared = costs_with_condition(&plan, &hosts, &operation, None);
    let observations = (0..2)
        .map(|i| observation(i, PoolRealizationHealth::Ready, 1))
        .collect::<Vec<_>>();
    let now = hosts.iter().map(|h| instant(h, 15)).collect::<Vec<_>>();
    let calibration = [revision()];
    for missing_host in [true, false] {
        let evaluations = hosts
            .iter()
            .zip(&now)
            .take(if missing_host { 1 } else { 2 })
            .map(|(host, now)| PoolCostEvaluation {
                host,
                now,
                calibrations: if missing_host { &calibration } else { &[] },
                executions: &[],
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
        let result = pool
            .admit_new_operation_with_costs(
                operation.clone(),
                key(1),
                &observations,
                "select/missing".into(),
                [9; 32],
                &prepared,
                &evaluations,
            )
            .unwrap();
        assert_eq!(result.account.evidence.selected_realization, Some(0));
        assert_eq!(
            result.account.choice,
            Some(if missing_host {
                PoolCostChoiceBasis::MinimumPredictedCompletion
            } else {
                PoolCostChoiceBasis::CapacityReference
            })
        );
        let expected = if missing_host {
            PoolCostUnavailable::MissingHost
        } else {
            PoolCostUnavailable::MissingCalibration
        };
        assert_eq!(
            result.account.sources[1].availability,
            PoolCostSourceAvailability::Unavailable(expected)
        );
        assert_eq!(
            result.account.candidates[1].cost,
            PoolCostDisposition::Unavailable(expected)
        );
        assert_eq!(
            result.account.sources[1].duration.upper_nanoseconds(),
            10_000_000
        );
        result.account.validate(&plan).unwrap();
    }
}
