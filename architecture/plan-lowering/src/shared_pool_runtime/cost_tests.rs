//! The sealed policy and prepared physical projection reach the ordinary pool.
use super::super::*;
use super::{fragment, key, observation, plan as reference_plan, pool};
use alloc::vec;
use conduit_core::*;
use conduit_kernel::NodeId;

pub(super) fn plan() -> Plan {
    let mut pool = pool();
    pool.selection_policy = SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity;
    seal_plan(
        PlotIdentity {
            source_document_id: "cost/source".into(),
            checked_plot_id: "cost/checked".into(),
            expanded_plot_id: "cost/expanded".into(),
        },
        vec![fragment(pool)],
    )
}
pub(super) fn hosts(plan: &Plan) -> Vec<HostAdvertisement> {
    plan.fragments[0].shared_pools[0].realization_envelope.iter().map(|r| {
        let offer = conduit_core::capability_offer_from_parts! {
            semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
            capability_id: r.capability_id.clone(), kind_id: "test/member".into(), kind_contract_revision: "test/member@1".into(),
            implementation: ImplementationOffer { execution_profile_id: "test/member@1".into(), implementation_id: r.implementation_id.clone(), artifact_id: r.artifact_id.clone() },
            inputs: vec![], outputs: vec![], host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
            limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 4096 },
        };
        HostAdvertisement { protocol_version: PROTOCOL_VERSION, host_id: r.host_id.clone(), boot_id: r.boot_id.clone(), offer_generation: r.offer_generation,
            profile: "cost/generic-fixture@1".into(), bases: vec![], resources: vec![], capabilities: vec![offer], planner_capabilities: vec![] }
    }).collect()
}
pub(super) fn instant(host: &HostAdvertisement, ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            host.host_id.clone(),
            host.boot_id.clone(),
            "cost/clock".into(),
            TemporalScale::Milliseconds,
            1,
            0,
        )
        .unwrap(),
    )
    .unwrap()
}
pub(super) fn revision() -> PoolCostCalibrationRevision {
    PoolCostCalibrationRevision::new("cost/test-calibration".into(), 1).unwrap()
}
fn costs<'a>(
    plan: &'a Plan,
    hosts: &'a [HostAdvertisement],
    operation: &PoolOperationId,
) -> Vec<PreparedPoolCost<'a>> {
    costs_with_condition(plan, hosts, operation, None)
}
pub(super) fn costs_with_condition<'a>(
    plan: &'a Plan,
    hosts: &'a [HostAdvertisement],
    operation: &PoolOperationId,
    condition: Option<CostExecutionCondition>,
) -> Vec<PreparedPoolCost<'a>> {
    hosts
        .iter()
        .enumerate()
        .map(|(index, host)| {
            let sign = SignId::from(if index == 0 { "cost/slow" } else { "cost/fast" });
            let duration = RationalQuantity::new(
                ExactRational::from_i128(if index == 0 { 100 } else { 10 }),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap();
            let tolerance = RationalQuantity::new(
                ExactRational::ZERO,
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap();
            PreparedPoolCost::prepare(
                PoolCostPreparation {
                    plan,
                    fragment_index: 0,
                    pool: &plan.fragments[0].shared_pools[0].pool_id,
                    operation,
                    demand: [9; 32],
                    realization: index as u16,
                    host,
                    offer: &host.capabilities[0],
                    resource: None,
                    calibration: &revision(),
                    expected_execution: condition,
                    sign: &sign,
                    supporting_signs: &[],
                    observed: &instant(host, 10),
                    expires: &instant(host, 20),
                    evaluated_at: &instant(host, 15),
                    duration,
                    tolerance,
                },
                &mut ExactNumericWork::new(1_000_000_000),
            )
            .ok()
            .unwrap()
        })
        .collect()
}
#[test]
fn sealed_completion_policy_selects_faster_member_and_released_capacity_without_replay() {
    let plan = plan();
    let hosts = hosts(&plan);
    let pool_id = plan.fragments[0].shared_pools[0].pool_id.clone();
    let operation = PoolOperationId::from("cost/request-1");
    let prepared = costs(&plan, &hosts, &operation);
    let observations = plan.fragments[0].shared_pools[0]
        .realization_envelope
        .iter()
        .enumerate()
        .map(|(index, _)| observation(index as u8, PoolRealizationHealth::Ready, 1))
        .collect::<Vec<_>>();
    let now = hosts
        .iter()
        .map(|host| instant(host, 15))
        .collect::<Vec<_>>();
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
    let mut pool = HostedSharedPool::<2, 32, 4>::new(&plan, 0, pool_id, NodeId(10), 1).unwrap();
    assert_eq!(
        pool.admit_new_operation(
            operation.clone(),
            key(1),
            &observations,
            "select/legacy".into()
        ),
        Err(SharedPoolRuntimeError::MissingPreparedCosts)
    );
    assert_eq!(pool.population(), 0);
    let first = pool
        .admit_new_operation_with_costs(
            operation,
            key(1),
            &observations,
            "select/1".into(),
            [9; 32],
            &prepared,
            &evaluations,
        )
        .unwrap();
    let member = match first.admission {
        SharedPoolAdmissionOutcome::Selected { member, evidence } => {
            assert!(evidence
                .observation_sign_ids
                .contains(&SignId::from("cost/fast")));
            member
        }
        _ => panic!("expected selected member"),
    };
    assert_eq!(member.placement.realization, 1);
    assert_eq!(first.sources[1].duration.upper_nanoseconds(), 10_000_000);
    pool.trigger(member).unwrap();
    let operation = PoolOperationId::from("cost/request-2");
    let prepared = costs(&plan, &hosts, &operation);
    let second = pool
        .admit_new_operation_with_costs(
            operation,
            key(2),
            &observations,
            "select/2".into(),
            [9; 32],
            &prepared,
            &evaluations,
        )
        .unwrap();
    assert!(
        matches!(second.admission, SharedPoolAdmissionOutcome::Selected { member, .. } if member.placement.realization == 0)
    );
    assert_eq!(
        second.decisions[1].admission,
        conduit_kernel::shared_pool::PoolCostAdmission::Full
    );
    pool.release(member).unwrap();
    let operation = PoolOperationId::from("cost/request-3");
    let prepared = costs(&plan, &hosts, &operation);
    let third = pool
        .admit_new_operation_with_costs(
            operation,
            key(3),
            &observations,
            "select/3".into(),
            [9; 32],
            &prepared,
            &evaluations,
        )
        .unwrap();
    assert!(
        matches!(third.admission, SharedPoolAdmissionOutcome::Selected { member: replacement, .. } if replacement.placement.realization == 1 && replacement.epoch != member.epoch)
    );
}
#[test]
fn stale_cross_operation_and_foreign_source_refuse_before_occupation() {
    let plan = plan();
    let hosts = hosts(&plan);
    let operation = PoolOperationId::from("cost/request-1");
    let prepared = costs(&plan, &hosts, &operation);
    let now = hosts
        .iter()
        .map(|host| instant(host, 20))
        .collect::<Vec<_>>();
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
    assert!(matches!(
        pool.admit_new_operation_with_costs(
            operation.clone(),
            key(1),
            &[],
            "select/stale".into(),
            [9; 32],
            &prepared,
            &evaluations
        ),
        Err(SharedPoolRuntimeError::Cost(
            PoolCostLoweringError::Prepared(PoolCostPreparationRefusal::Stale)
        ))
    ));
    assert_eq!(pool.population(), 0);
    assert!(matches!(
        pool.admit_new_operation_with_costs(
            "other/operation".into(),
            key(1),
            &[],
            "select/other".into(),
            [9; 32],
            &prepared,
            &evaluations
        ),
        Err(SharedPoolRuntimeError::Cost(
            PoolCostLoweringError::Correlation
        ))
    ));
    assert_eq!(pool.population(), 0);
    let mut changed = hosts.clone();
    changed[0].profile = "changed/profile".into();
    let now = changed
        .iter()
        .map(|host| instant(host, 15))
        .collect::<Vec<_>>();
    let evaluations = changed
        .iter()
        .zip(&now)
        .map(|(host, now)| PoolCostEvaluation {
            host,
            now,
            calibrations: &calibrations,
            executions: &[],
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        pool.admit_new_operation_with_costs(
            operation,
            key(1),
            &[],
            "select/changed".into(),
            [9; 32],
            &prepared,
            &evaluations
        ),
        Err(SharedPoolRuntimeError::Cost(
            PoolCostLoweringError::Prepared(PoolCostPreparationRefusal::SourceChanged)
        ))
    ));
    assert_eq!(pool.population(), 0);
}
#[test]
fn empty_costs_explicitly_use_capacity_reference_and_policy_is_in_plan_identity() {
    let plan = plan();
    let old = reference_plan();
    let mut capacity_fragments = plan.fragments.clone();
    capacity_fragments[0].shared_pools[0].selection_policy =
        old.fragments[0].shared_pools[0].selection_policy;
    let capacity_plan = seal_plan(
        PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        capacity_fragments,
    );
    assert!(verify_plan(&capacity_plan));
    assert_ne!(capacity_plan.plan_id, plan.plan_id);
    let mut mutated = old.clone();
    mutated.fragments[0].shared_pools[0].selection_policy =
        SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity;
    assert!(!verify_plan(&mutated));
    let observations = plan.fragments[0].shared_pools[0]
        .realization_envelope
        .iter()
        .enumerate()
        .map(|(index, _)| observation(index as u8, PoolRealizationHealth::Ready, 1))
        .collect::<Vec<_>>();
    let mut pool = HostedSharedPool::<2, 32, 4>::new(
        &plan,
        0,
        plan.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    let selected = pool
        .admit_new_operation_with_costs(
            "no/cost/request".into(),
            key(1),
            &observations,
            "select/fallback".into(),
            [9; 32],
            &[],
            &[],
        )
        .unwrap();
    assert!(
        matches!(selected.admission, SharedPoolAdmissionOutcome::Selected { member, .. } if member.placement.realization == 0)
    );
    assert!(selected.sources.is_empty());
    assert!(selected
        .decisions
        .iter()
        .all(|d| d.cost == conduit_kernel::shared_pool::PoolCostAvailability::Missing));
    let mut ordinary = HostedSharedPool::<2, 32, 4>::new(
        &old,
        0,
        old.fragments[0].shared_pools[0].pool_id.clone(),
        NodeId(10),
        1,
    )
    .unwrap();
    assert!(matches!(
        ordinary.admit_new_operation_with_costs(
            "no/cost/request".into(),
            key(1),
            &observations,
            "select/wrong-policy".into(),
            [9; 32],
            &[],
            &[]
        ),
        Err(SharedPoolRuntimeError::Cost(
            PoolCostLoweringError::PolicyMismatch
        ))
    ));
    assert_eq!(ordinary.population(), 0);
}
