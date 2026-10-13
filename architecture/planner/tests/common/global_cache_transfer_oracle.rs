//! Cached transport-sensitive scoring must equal fresh ordinary planning.
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_plot::CheckedPlot;

pub fn check<'a>(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'a, '_>,
    transfer: GlobalTransferInputs<'a>,
    cold: &Result<GlobalPlacementResult, GlobalPlacementRefusal>,
) {
    let budget = GlobalPlacementBudget {
        maximum_states: 128,
    };
    let objective = GlobalPlacementObjective::CriticalPathFulfillmentTime;
    let mut cache = GlobalPlanCache::admit(GlobalPlanCachePolicy {
        maximum_entries: 32,
        maximum_encoded_plan_bytes: 1_048_576,
        maximum_fingerprint_bytes_per_call: 16_777_216,
    })
    .unwrap();
    let first = cache.plan(
        plot,
        inputs,
        budget,
        objective,
        Some(transfer),
        &mut ExactNumericWork::new(100_000_000),
    );
    let warm = cache.plan(
        plot,
        inputs,
        budget,
        objective,
        Some(transfer),
        &mut ExactNumericWork::new(100_000_000),
    );
    match cold {
        Err(_) => {
            assert!(first.is_err());
            assert!(warm.is_err());
        }
        Ok(reference) => {
            let first = first.unwrap();
            let warm = warm.unwrap();
            assert_eq!(first.plan, reference.plan);
            assert_eq!(warm.plan, reference.plan);
            assert_eq!(first.score, reference.score);
            assert_eq!(warm.score, reference.score);
            assert_eq!(warm.states, reference.states);
            assert_eq!(warm.disposition, reference.disposition);
            assert!(cache.metrics().reused_plans > 0);
            assert_eq!(cache.metrics().constructed_plans, 0);
            if !transfer.forecasts.is_empty() {
                let missing = GlobalTransferInputs {
                    forecasts: &transfer.forecasts[1..],
                    ..transfer
                };
                let oracle = plan_global_realizations_with_transfer_costs(
                    plot,
                    inputs,
                    budget,
                    missing,
                    &mut ExactNumericWork::new(100_000_000),
                )
                .unwrap();
                let changed = cache
                    .plan(
                        plot,
                        inputs,
                        budget,
                        objective,
                        Some(missing),
                        &mut ExactNumericWork::new(100_000_000),
                    )
                    .unwrap();
                assert_eq!(changed.plan, oracle.plan);
                assert_eq!(changed.score, oracle.score);
                assert!(changed.score.unknown_transfers > reference.score.unknown_transfers);
                assert!(cache.metrics().reused_plans > 0);
                assert_eq!(cache.metrics().constructed_plans, 0);
            }
            let later: Vec<_> = inputs
                .contexts
                .iter()
                .map(|ctx| {
                    MonotonicInstant::new(
                        ctx.now.ticks().checked_add(1).unwrap(),
                        ctx.now.clock().clone(),
                    )
                    .unwrap()
                })
                .collect();
            let contexts: Vec<_> = inputs
                .contexts
                .iter()
                .enumerate()
                .map(|(i, ctx)| CostEvaluationContext {
                    host: ctx.host,
                    calibrations: ctx.calibrations,
                    now: &later[i],
                    execution_observations: ctx.execution_observations,
                })
                .collect();
            let expired = CostPlanningInputs {
                contexts: &contexts,
                ..inputs
            };
            let oracle = plan_global_realizations_with_transfer_costs(
                plot,
                expired,
                budget,
                transfer,
                &mut ExactNumericWork::new(100_000_000),
            )
            .unwrap();
            let updated = cache
                .plan(
                    plot,
                    expired,
                    budget,
                    objective,
                    Some(transfer),
                    &mut ExactNumericWork::new(100_000_000),
                )
                .unwrap();
            assert_eq!(updated.plan, oracle.plan);
            assert_eq!(updated.score, oracle.score);
            assert!(updated.score.unknown_transfers > reference.score.unknown_transfers);
            assert_eq!(updated.score.critical_path_upper_seconds, None);
            assert!(cache.metrics().reused_plans > 0);
            assert_eq!(cache.metrics().constructed_plans, 0);
            let mut lines = inputs.options.line_offers.to_vec();
            for line in &mut lines {
                line.availability.availability = LineAvailability::Unavailable;
            }
            let options = PlanningOptions {
                line_offers: &lines,
                ..inputs.options
            };
            let disconnected = CostPlanningInputs { options, ..inputs };
            assert!(plan_global_realizations_with_transfer_costs(
                plot,
                disconnected,
                budget,
                transfer,
                &mut ExactNumericWork::new(100_000_000)
            )
            .is_err());
            assert!(cache
                .plan(
                    plot,
                    disconnected,
                    budget,
                    objective,
                    Some(transfer),
                    &mut ExactNumericWork::new(100_000_000)
                )
                .is_err());
            assert_eq!(cache.metrics().reused_plans, 0);
        }
    }
}
