//! Adversarial joint assignments through ordinary hard checks and Plan sealing.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use std::collections::BTreeMap;

fn forecast(ms: i128) -> FulfillmentCostForecast {
    let mut work = ExactNumericWork::new(100_000_000);
    let mut point = |ms| {
        let value = CostDuration::admit(
            RationalQuantity::new(
                ExactRational::from_i128(ms),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap(),
            &mut work,
        )
        .unwrap();
        CostDurationRange::admit(value, value, &mut work).unwrap()
    };
    let stages = CostForecastStages {
        setup: Some(point(0)),
        queue: Some(point(0)),
        service: Some(point(ms)),
        transfer: Some(point(0)),
    };
    FulfillmentCostForecast::sequential(stages, &mut work).unwrap()
}
fn exercise(
    constrained_b: bool,
    states: u32,
    inspect: impl FnOnce(
        Result<GlobalPlacementResult, GlobalPlacementRefusal>,
        Result<CostPlannedRealizations<'_>, CostPlanningError<'_>>,
        &[HostAdvertisement],
    ),
) {
    exercise_objective(
        constrained_b,
        states,
        GlobalPlacementObjective::AggregateFulfillmentTime,
        false,
        inspect,
    )
}
fn exercise_objective(
    constrained_b: bool,
    states: u32,
    objective: GlobalPlacementObjective,
    critical_tradeoff: bool,
    inspect: impl FnOnce(
        Result<GlobalPlacementResult, GlobalPlacementRefusal>,
        Result<CostPlannedRealizations<'_>, CostPlanningError<'_>>,
        &[HostAdvertisement],
    ),
) {
    let plot = conduit_plot::parse_with_startup(
        "plot costs {\n first: flow/pulse(count = 2, period-ms = 0, initial = false)\n second: flow/pulse(count = 3, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(), &conduit_signal::signal_profile_catalog()).unwrap();
    let mut hosts = common::competing_hosts();
    for host in &mut hosts {
        host.capabilities[0].limits.max_active_instances = 1;
        host.capabilities[0].resource_requirements[0].units = 1;
    }
    hosts[1].capabilities[0].limits.max_queue_items = 1;
    let windows: Vec<_> = hosts
        .iter()
        .map(|host| {
            let clock = MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "global/clock".into(),
                TemporalScale::Milliseconds,
                1,
                0,
            )
            .unwrap();
            CostObservationWindow::admit(
                MonotonicInstant::new(100, clock.clone()).unwrap(),
                MonotonicInstant::new(1000, clock).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let now: Vec<_> = windows
        .iter()
        .map(|window| MonotonicInstant::new(101, window.observed().clock().clone()).unwrap())
        .collect();
    let identity = ArtifactId::from("fixture/global-cost@1");
    let calibration = [CostCalibrationRevision::new(&identity, 1).unwrap()];
    let contexts: Vec<_> = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &calibration,
            now,
            execution_observations: &[],
        })
        .collect();
    let signs: [SignId; 2] = ["fixture/global/a", "fixture/global/b"].map(Into::into);
    let sources: Vec<_> = hosts
        .iter()
        .enumerate()
        .map(|(i, host)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                &signs[i],
                calibration[0],
                windows[i].clone(),
            )
            .unwrap()
        })
        .collect();
    // The two checked startup counts are distinct work classes, so the second
    // operation legitimately has a different demand-specific fulfillment model.
    let forecasts = if critical_tradeoff {
        [[forecast(1), forecast(6)], [forecast(6), forecast(9)]]
    } else {
        [[forecast(1), forecast(2)], [forecast(2), forecast(100)]]
    };
    let bound: Vec<Vec<_>> = plot
        .gears
        .iter()
        .enumerate()
        .map(|(i, gear)| {
            let demand = CostDemand::admit(gear, &[]).unwrap();
            sources
                .iter()
                .zip(&forecasts[i])
                .map(|(source, forecast)| CostedForecast::bind(source, demand, forecast).unwrap())
                .collect()
        })
        .collect();
    let requirements = [
        HardRealizationRequirements::default(),
        HardRealizationRequirements {
            minimum_queue_items: if constrained_b { 2 } else { 0 },
            ..Default::default()
        },
    ];
    let policy = RealizationPolicy::default();
    let operations: Vec<_> = plot
        .gears
        .iter()
        .enumerate()
        .map(|(i, gear)| CostPlanningOperation {
            demand: CostDemand::admit(gear, &[]).unwrap(),
            forecasts: &bound[i],
            requirements: &requirements[i],
            tie_policy: &policy,
            intent: PerformanceIntent::ThroughputBatch,
            unavailable: UnavailableCostPolicy::RequireKnown,
        })
        .collect();
    let mut observations = common::resource_observations(&hosts);
    for observation in &mut observations {
        if observation.class_id == hosts[0].capabilities[0].resource_requirements[0].class_id {
            observation.unreserved_units = 1;
        }
    }
    let timed: Vec<_> = observations
        .iter()
        .map(|observation| {
            let host = hosts
                .iter()
                .position(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, windows[host].clone()).unwrap()
        })
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let inputs = CostPlanningInputs {
        hosts: &hosts,
        contexts: &contexts,
        operations: &operations,
        advertisements: &[],
        observations: &timed,
        bases: &[],
        options: PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    };
    let global = plan_global_realizations_with_objective(
        &plot,
        inputs,
        GlobalPlacementBudget {
            maximum_states: states,
        },
        objective,
        &mut ExactNumericWork::new(100_000_000),
    );
    let greedy = plan_selected_realizations_with_costs(
        &plot,
        inputs,
        &mut ExactNumericWork::new(100_000_000),
    );
    inspect(global, greedy, &hosts);
}
#[test]
fn joint_mode_finds_the_admitted_arrangement_that_greedy_starves() {
    exercise(true, 64, |global, greedy, hosts| {
        assert!(greedy.is_err());
        let global = global.unwrap();
        assert!(verify_plan(&global.plan));
        assert_eq!(global.disposition, GlobalPlacementDisposition::Exhaustive);
        let placement = global
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.gear_id.as_str().ends_with("/first"))
            .unwrap();
        assert_eq!(placement.host_id, hosts[1].host_id);
        assert_eq!(global.score.unknown_operations, 0);
        assert_eq!(
            global.score.known_upper_seconds,
            ExactRational::ratio(4, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap()
        );
    });
}
#[test]
fn joint_mode_improves_aggregate_demand_specific_time_under_shared_capacity() {
    exercise(false, 64, |global, greedy, _| {
        let greedy = greedy.unwrap();
        let global = global.unwrap();
        assert!(verify_plan(&global.plan));
        assert_ne!(global.plan.plan_id, greedy.plan.plan_id);
        let mut work = ExactNumericWork::new(100_000_000);
        let mut baseline = ExactRational::ZERO;
        for decision in &greedy.decisions {
            let selected = decision.costs.iter().find(|cost| cost.selected).unwrap();
            let CostAvailability::Known(forecast) = selected.availability else {
                panic!("known fixture cost missing");
            };
            baseline = baseline
                .checked_add(forecast.upper_seconds(), &mut work)
                .unwrap();
        }
        assert_eq!(
            baseline,
            ExactRational::ratio(101, 1000, &mut work).unwrap()
        );
        assert!(global
            .score
            .known_upper_seconds
            .checked_compare(baseline, &mut work)
            .unwrap()
            .is_lt());
        assert_eq!(
            global.score.known_upper_seconds,
            ExactRational::ratio(4, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap()
        );
        assert!(global.refused_assignments > 0);
    });
}
#[test]
fn state_bound_returns_only_an_independently_feasible_plan_or_explicit_refusal() {
    exercise(false, 4, |global, _, _| {
        let global = global.unwrap();
        assert!(verify_plan(&global.plan));
        assert_eq!(global.states, 4);
        assert_eq!(global.disposition, GlobalPlacementDisposition::StateLimit);
    });
    exercise(false, 2, |global, _, _| {
        assert!(matches!(
            global,
            Err(GlobalPlacementRefusal::NoFeasibleAssignment {
                disposition: GlobalPlacementDisposition::StateLimit,
                states: 2,
                ..
            })
        ));
    });
}

#[test]
fn critical_path_mode_selects_parallel_completion_over_smaller_aggregate_time() {
    let expected = ExactRational::ratio(6, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap();
    exercise_objective(
        false,
        64,
        GlobalPlacementObjective::CriticalPathFulfillmentTime,
        true,
        |global, greedy, _| {
            let global = global.unwrap();
            assert!(verify_plan(&global.plan));
            assert_ne!(global.plan.plan_id, greedy.unwrap().plan.plan_id);
            assert_eq!(
                global.objective,
                GlobalPlacementObjective::CriticalPathFulfillmentTime
            );
            assert_eq!(global.score.critical_path_upper_seconds, Some(expected));
            assert_eq!(
                global.score.known_upper_seconds,
                ExactRational::ratio(12, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap()
            );
        },
    );
}
