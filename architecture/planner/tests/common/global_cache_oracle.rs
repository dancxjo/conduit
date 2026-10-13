//! Fresh cold-oracle checks for retained hard Plans, never retained cost scores.
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_plot::CheckedPlot;

pub fn check<'a>(
    plot: &CheckedPlot,
    inputs: CostPlanningInputs<'a, '_>,
    sources: &[CostEvidenceSource<'a>],
    models: &'a [[FulfillmentCostForecast; 2]; 3],
) {
    let budget = GlobalPlacementBudget { maximum_states: 64 };
    let objective = GlobalPlacementObjective::AggregateFulfillmentTime;
    let mut cache = GlobalPlanCache::admit(GlobalPlanCachePolicy {
        maximum_entries: 8,
        maximum_encoded_plan_bytes: 1_048_576,
        maximum_fingerprint_bytes_per_call: 16_777_216,
    })
    .unwrap();
    let cold = plan_global_realizations_with_objective(
        plot,
        inputs,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let first = cache
        .plan(
            plot,
            inputs,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(first.plan, cold.plan);
    assert_eq!(first.score, cold.score);
    assert!(cache.metrics().constructed_plans > 0);
    let warm = cache
        .plan(
            plot,
            inputs,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(warm.plan, cold.plan);
    assert_eq!(warm.score, cold.score);
    assert_eq!(warm.states, cold.states);
    assert_eq!(warm.disposition, cold.disposition);
    assert!(cache.metrics().reused_plans > 0);
    assert_eq!(cache.metrics().constructed_plans, 0);
    let mut extra_hosts = inputs.hosts.to_vec();
    let mut unused = inputs.hosts[0].clone();
    unused.host_id = HostId::from("host/cache/unrelated");
    unused.boot_id = BootId::from("boot/cache/unrelated");
    unused.resources.clear();
    unused.capabilities.clear();
    unused.planner_capabilities.clear();
    extra_hosts.push(unused);
    let unused_host = extra_hosts.last().unwrap();
    let unused_clock = MonotonicClockIdentity::new(
        unused_host.host_id.clone(),
        unused_host.boot_id.clone(),
        "fixture/cache/unrelated-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let unused_now = MonotonicInstant::new(101, unused_clock).unwrap();
    let mut extra_contexts: Vec<_> = inputs
        .contexts
        .iter()
        .map(|ctx| CostEvaluationContext {
            host: ctx.host,
            calibrations: ctx.calibrations,
            now: ctx.now,
            execution_observations: ctx.execution_observations,
        })
        .collect();
    extra_contexts.push(CostEvaluationContext {
        host: unused_host,
        calibrations: inputs.contexts[0].calibrations,
        now: &unused_now,
        execution_observations: &[],
    });
    let unrelated = CostPlanningInputs {
        hosts: &extra_hosts,
        contexts: &extra_contexts,
        ..inputs
    };
    let oracle = plan_global_realizations_with_objective(
        plot,
        unrelated,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let reused = cache
        .plan(
            plot,
            unrelated,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(reused.plan, oracle.plan);
    assert_eq!(reused.score, oracle.score);
    assert!(cache.metrics().reused_plans > 0);
    assert_eq!(cache.metrics().constructed_plans, 0);
    let exhausted: Vec<_> = inputs
        .observations
        .iter()
        .map(|sample| {
            let mut observation = sample.observation().clone();
            observation.unreserved_units = 0;
            observation
        })
        .collect();
    let timed: Vec<_> = exhausted
        .iter()
        .zip(inputs.observations)
        .map(|(value, old)| TimedResourceObservation::admit(value, old.window().clone()).unwrap())
        .collect();
    let pressure = CostPlanningInputs {
        observations: &timed,
        ..inputs
    };
    assert!(plan_global_realizations_with_objective(
        plot,
        pressure,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000)
    )
    .is_err());
    assert!(cache
        .plan(
            plot,
            pressure,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000)
        )
        .is_err());
    assert_eq!(cache.metrics().reused_plans, 0);
    let tiny = GlobalPlacementBudget { maximum_states: 1 };
    assert!(plan_global_realizations_with_objective(
        plot,
        inputs,
        tiny,
        objective,
        &mut ExactNumericWork::new(100_000_000)
    )
    .is_err());
    assert!(cache
        .plan(
            plot,
            inputs,
            tiny,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000)
        )
        .is_err());
    let mut hosts = inputs.hosts.to_vec();
    hosts[0].capabilities[0].implementation.artifact_id =
        ArtifactId::from("fixture/cache/new-artifact");
    let contexts: Vec<_> = inputs
        .contexts
        .iter()
        .enumerate()
        .map(|(i, ctx)| CostEvaluationContext {
            host: &hosts[i],
            calibrations: ctx.calibrations,
            now: ctx.now,
            execution_observations: ctx.execution_observations,
        })
        .collect();
    let operations: Vec<_> = inputs
        .operations
        .iter()
        .map(|op| CostPlanningOperation {
            demand: op.demand,
            forecasts: op.forecasts,
            requirements: op.requirements,
            tie_policy: op.tie_policy,
            intent: op.intent,
            unavailable: UnavailableCostPolicy::PreferKnown,
        })
        .collect();
    let artifact_changed = CostPlanningInputs {
        hosts: &hosts,
        contexts: &contexts,
        operations: &operations,
        ..inputs
    };
    let oracle = plan_global_realizations_with_objective(
        plot,
        artifact_changed,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let changed_plan = cache
        .plan(
            plot,
            artifact_changed,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(changed_plan.plan, oracle.plan);
    assert_eq!(changed_plan.score, oracle.score);
    assert!(cache.metrics().invalidated_entries > 0);
    assert!(cache.metrics().constructed_plans > 0);
    assert!(changed_plan
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .any(|p| p.artifact_id.as_str() == "fixture/cache/new-artifact"));
    // Restore the original hard basis after the artifact mutation, then change
    // only costs. Reuse is expected within a hard basis, not across artifacts.
    let restored = cache
        .plan(
            plot,
            inputs,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(restored.plan, cold.plan);
    assert!(cache.metrics().constructed_plans > 0);
    let bound: Vec<Vec<_>> = inputs
        .operations
        .iter()
        .enumerate()
        .map(|(index, operation)| {
            sources
                .iter()
                .zip(&models[index])
                .map(|(source, model)| {
                    CostedForecast::bind(source, operation.demand, model).unwrap()
                })
                .collect()
        })
        .collect();
    let operations: Vec<_> = inputs
        .operations
        .iter()
        .enumerate()
        .map(|(index, op)| CostPlanningOperation {
            demand: op.demand,
            forecasts: &bound[index],
            requirements: op.requirements,
            tie_policy: op.tie_policy,
            intent: op.intent,
            unavailable: op.unavailable,
        })
        .collect();
    let changed = CostPlanningInputs {
        operations: &operations,
        ..inputs
    };
    let oracle = plan_global_realizations_with_objective(
        plot,
        changed,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let replanned = cache
        .plan(
            plot,
            changed,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(replanned.plan, oracle.plan);
    assert_eq!(replanned.score, oracle.score);
    assert_ne!(replanned.plan, cold.plan);
    assert!(cache.metrics().reused_plans > 0);
    let options = PlanningOptions {
        connection_byte_capacity: 2048,
        ..inputs.options
    };
    let changed_hard = CostPlanningInputs { options, ..changed };
    let oracle = plan_global_realizations_with_objective(
        plot,
        changed_hard,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let replanned = cache
        .plan(
            plot,
            changed_hard,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(replanned.plan, oracle.plan);
    assert_eq!(replanned.score, oracle.score);
    assert!(cache.metrics().invalidated_entries > 0);
    assert!(cache.metrics().constructed_plans > 0);
    let mut bypass = GlobalPlanCache::admit(GlobalPlanCachePolicy {
        maximum_entries: 1,
        maximum_encoded_plan_bytes: 1,
        maximum_fingerprint_bytes_per_call: 1,
    })
    .unwrap();
    let limited = bypass
        .plan(
            plot,
            inputs,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(limited.plan, cold.plan);
    assert_eq!(limited.score, cold.score);
    assert_eq!(bypass.metrics().retained_entries, 0);
    assert!(bypass.metrics().bypassed_assignments > 0);
    let expired: Vec<_> = sources
        .iter()
        .map(|source| source.window().expires().clone())
        .collect();
    let contexts: Vec<_> = inputs
        .contexts
        .iter()
        .enumerate()
        .map(|(i, ctx)| CostEvaluationContext {
            host: ctx.host,
            calibrations: ctx.calibrations,
            now: &expired[i],
            execution_observations: ctx.execution_observations,
        })
        .collect();
    let stale = CostPlanningInputs {
        contexts: &contexts,
        ..inputs
    };
    assert!(plan_global_realizations_with_objective(
        plot,
        stale,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000)
    )
    .is_err());
    assert!(cache
        .plan(
            plot,
            stale,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000)
        )
        .is_err());
}
