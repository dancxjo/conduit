//! Actual production receipt joins; no model service or timing benchmark.
mod pool_dispatch {
    pub mod fixture;
}
use conduit_core::*;
use conduit_observatory::{
    render_pool_dispatch, CostExplanationRefusal, MAXIMUM_COST_EXPLANATION_BYTES,
};
use pool_dispatch::fixture::*;

#[test]
fn current_short_and_long_demand_render_exact_stages_and_decisive_signs() {
    for size in [1, 20] {
        scenario(Case::Healthy, size, |plan, pool, dispatch, forecasts| {
            let selected = dispatch.account.evidence.selected_realization.unwrap() as usize;
            let selected_host =
                &plan.fragments[0].shared_pools[0].realization_envelope[selected].host_id;
            assert_eq!(
                selected_host,
                &forecasts[if size == 1 { 0 } else { 1 }].source.host_id
            );
            let text = render_pool_dispatch(plan, dispatch, forecasts, &mut work()).unwrap();
            for expected in [
                "retained_input_bytes=",
                "queue_prediction=",
                "1/4",
                "setup_prediction=",
                "transfer_prediction=",
                "inspection/forecast-",
                "owner_derived_amount=",
                "measured_latency=not-retained",
                "warmup_prediction=not-separately-retained",
                "occupied_members=0",
            ] {
                assert!(text.contains(expected), "missing {expected}: {text}");
            }
            assert_eq!(pool.population(), 1);
            assert!(text.len() <= MAXIMUM_COST_EXPLANATION_BYTES);
        });
    }
}
#[test]
fn full_hard_missing_and_stale_costs_keep_distinct_dispositions() {
    for (case, expected) in [
        (Case::Full, "hard_admission=Full"),
        (Case::HardRefusal, "hard_admission=NotCurrent"),
        (Case::Missing, "cost=Missing"),
        (Case::Stale, "Unavailable(Prepared(Stale))"),
    ] {
        scenario(case, 1, |plan, _, dispatch, forecasts| {
            let text = render_pool_dispatch(plan, dispatch, forecasts, &mut work()).unwrap();
            assert!(text.contains(expected), "missing {expected}: {text}");
        });
    }
}
#[test]
fn queued_occupation_is_an_owner_snapshot_and_inspection_cannot_dispatch() {
    scenario(Case::Queued, 1, |plan, pool, dispatch, forecasts| {
        assert_eq!(dispatch.population_before_selection.waiting_operations, 1);
        assert_eq!(pool.waiting_population(), 0);
        let population = pool.population();
        let plan_bytes = serde_json::to_vec(plan).unwrap();
        let receipt = format!("{dispatch:?}");
        for _ in 0..32 {
            let text = render_pool_dispatch(plan, dispatch, forecasts, &mut work()).unwrap();
            assert!(text.contains("waiting_operations=1"));
            assert!(text.len() <= MAXIMUM_COST_EXPLANATION_BYTES);
        }
        assert_eq!(pool.population(), population);
        assert_eq!(pool.waiting_population(), 0);
        assert_eq!(serde_json::to_vec(plan).unwrap(), plan_bytes);
        assert_eq!(format!("{dispatch:?}"), receipt);
    });
}
#[test]
fn foreign_operation_source_forged_stages_and_inspection_limits_refuse() {
    scenario(Case::Healthy, 1, |plan, pool, dispatch, forecasts| {
        let operation = forecasts[0].binding.operation.clone();
        forecasts[0].binding.operation = "foreign/operation".into();
        assert_eq!(
            render_pool_dispatch(plan, dispatch, forecasts, &mut work()),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        forecasts[0].binding.operation = operation;
        let sign = forecasts[0].source.sign_id.clone();
        forecasts[0].source.sign_id = "foreign/sign".into();
        assert_eq!(
            render_pool_dispatch(plan, dispatch, forecasts, &mut work()),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        forecasts[0].source.sign_id = sign;
        let stages = forecasts[0].stages;
        forecasts[0].stages.queue = None;
        assert_eq!(
            render_pool_dispatch(plan, dispatch, forecasts, &mut work()),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        forecasts[0].stages = stages;
        assert_eq!(
            render_pool_dispatch(plan, dispatch, forecasts, &mut ExactNumericWork::new(0)),
            Err(CostExplanationRefusal::Extent)
        );
        dispatch.population_before_selection.waiting_operations = usize::MAX;
        assert_eq!(
            render_pool_dispatch(plan, dispatch, forecasts, &mut work()),
            Err(CostExplanationRefusal::Extent)
        );
        assert_eq!(pool.population(), 1);
    });
}
