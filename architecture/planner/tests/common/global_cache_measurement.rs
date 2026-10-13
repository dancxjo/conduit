use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_plot::CheckedPlot;

/// Internal measurement over synthetic admitted facts, not physical work benefits.
pub fn measure(plot: &CheckedPlot, inputs: CostPlanningInputs<'_, '_>) {
    let budget = GlobalPlacementBudget { maximum_states: 64 };
    for objective in [
        GlobalPlacementObjective::AggregateFulfillmentTime,
        GlobalPlacementObjective::CriticalPathFulfillmentTime,
    ] {
        let mut cache = GlobalPlanCache::admit(GlobalPlanCachePolicy {
            maximum_entries: 8,
            maximum_encoded_plan_bytes: 1_048_576,
            maximum_fingerprint_bytes_per_call: 16_777_216,
        })
        .unwrap();
        let reference = plan_global_realizations_with_objective(
            plot,
            inputs,
            budget,
            objective,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
        cache
            .plan(
                plot,
                inputs,
                budget,
                objective,
                None,
                &mut ExactNumericWork::new(100_000_000),
            )
            .unwrap();
        let mut rows = Vec::new();
        for index in 0..32 {
            let mut cold_work = ExactNumericWork::new(100_000_000);
            let mut warm_work = ExactNumericWork::new(100_000_000);
            let mut cold_run = || {
                let start = std::time::Instant::now();
                let plan = plan_global_realizations_with_objective(
                    plot,
                    inputs,
                    budget,
                    objective,
                    &mut cold_work,
                )
                .unwrap();
                (plan, u64::try_from(start.elapsed().as_nanos()).unwrap())
            };
            let mut warm_run = || {
                let start = std::time::Instant::now();
                let plan = cache
                    .plan(plot, inputs, budget, objective, None, &mut warm_work)
                    .unwrap();
                (plan, u64::try_from(start.elapsed().as_nanos()).unwrap())
            };
            let (cold, warm) = if index % 2 == 0 {
                (cold_run(), warm_run())
            } else {
                let warm = warm_run();
                (cold_run(), warm)
            };
            assert_eq!(cold.0.plan, reference.plan);
            assert_eq!(warm.0.plan, reference.plan);
            assert_eq!(cold.0.score, warm.0.score);
            let metrics = cache.metrics();
            assert_eq!(metrics.constructed_plans, 0);
            assert!(metrics.reused_plans > 0);
            rows.push(serde_json::json!({"sample":index,"cold_ns":cold.1,"warm_ns":warm.1,
                "cold_numeric_work":cold_work.spent(),"warm_numeric_work":warm_work.spent(),
                "states":warm.0.states,"reused_plans":metrics.reused_plans,"fingerprint_bytes":metrics.fingerprint_bytes,
                "retained_entries":metrics.retained_entries,"retained_encoded_plan_bytes":metrics.retained_encoded_plan_bytes}));
        }
        println!(
            "CACHE_MEASUREMENT {}",
            serde_json::json!({"objective":format!("{objective:?}"),
            "scope":"two-Gear synthetic fixture, unchanged admitted facts, debug build; full planning call includes eligibility/fingerprinting/capacity/exact scoring; no physical workload, release speedup or confidence claim",
            "rows":rows})
        );
    }
}
