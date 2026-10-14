//! Modeled current-provider evidence; no physical timing claim.
use crate::*;
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
fn compare(states: u32, inspect: impl FnOnce(&GlobalPlacementComparison<'_, '_>)) {
    let plot = conduit_plot::parse_with_startup(
        "plot compare {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap();
    let hosts: Vec<_> = [
        ("fixture/provider-a", "fixture/boot-a"),
        ("fixture/provider-b", "fixture/boot-b"),
    ]
    .into_iter()
    .map(|(host, boot)| {
        let mut ad = conduit_signal_conformance::distributed_source_advertisement_for(
            host.into(),
            boot.into(),
        );
        ad.capabilities
            .retain(|c| c.kind_id.as_str() == conduit_signal::PULSE_KIND);
        ad
    })
    .collect();
    let clocks: Vec<_> = hosts
        .iter()
        .map(|h| {
            MonotonicClockIdentity::new(
                h.host_id.clone(),
                h.boot_id.clone(),
                "fixture/compare-clock".into(),
                TemporalScale::Milliseconds,
                1,
                0,
            )
            .unwrap()
        })
        .collect();
    let windows: Vec<_> = clocks
        .iter()
        .map(|c| {
            CostObservationWindow::admit(
                MonotonicInstant::new(100, c.clone()).unwrap(),
                MonotonicInstant::new(200, c.clone()).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let now: Vec<_> = clocks
        .iter()
        .map(|c| MonotonicInstant::new(150, c.clone()).unwrap())
        .collect();
    let artifact: ArtifactId = "fixture/compare-calibration".into();
    let revision = [CostCalibrationRevision::new(&artifact, 1).unwrap()];
    let contexts: Vec<_> = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &revision,
            now,
            execution_observations: &[],
        })
        .collect();
    let signs: [SignId; 2] = ["fixture/forecast-a", "fixture/forecast-b"].map(Into::into);
    let sources: Vec<_> = hosts
        .iter()
        .enumerate()
        .map(|(i, h)| {
            CostEvidenceSource::bind(
                h,
                &h.capabilities[0],
                None,
                &signs[i],
                revision[0],
                windows[i].clone(),
            )
            .unwrap()
        })
        .collect();
    let mut work = ExactNumericWork::new(100_000_000);
    let mut point = |n| {
        let q = RationalQuantity::new(
            ExactRational::from_i128(n),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap();
        let t = CostDuration::admit(q, &mut work).unwrap();
        CostDurationRange::admit(t, t, &mut work).unwrap()
    };
    let stages: Vec<_> = [2, 1]
        .into_iter()
        .map(|n| CostForecastStages {
            setup: Some(point(0)),
            queue: Some(point(0)),
            service: Some(point(n)),
            transfer: Some(point(0)),
        })
        .collect();
    let models: Vec<_> = stages
        .into_iter()
        .map(|s| FulfillmentCostForecast::sequential(s, &mut work).unwrap())
        .collect();
    let demand = CostDemand::admit(&plot.gears[0], &[]).unwrap();
    let forecasts: Vec<_> = sources
        .iter()
        .zip(&models)
        .map(|(source, model)| CostedForecast::bind(source, demand, model).unwrap())
        .collect();
    let requirements = HardRealizationRequirements::default();
    let tie = RealizationPolicy::default();
    let operations = [CostPlanningOperation {
        demand,
        forecasts: &forecasts,
        requirements: &requirements,
        tie_policy: &tie,
        intent: PerformanceIntent::Interactive,
        unavailable: UnavailableCostPolicy::RequireKnown,
    }];
    let observations: Vec<_> = hosts
        .iter()
        .flat_map(|h| {
            h.resources
                .iter()
                .enumerate()
                .map(|(i, r)| ResourceObservation {
                    host_id: h.host_id.clone(),
                    boot_id: h.boot_id.clone(),
                    offer_generation: h.offer_generation,
                    pool_id: r.pool_id.clone(),
                    class_id: r.class_id.clone(),
                    health: ResourceHealth::Ready,
                    unreserved_units: r.capacity_units,
                    utilized_units: 0,
                    sign_id: alloc::format!("fixture/resource/{}/{i}", h.host_id.as_str()).into(),
                })
        })
        .collect();
    let timed: Vec<_> = observations
        .iter()
        .map(|o| {
            let i = hosts.iter().position(|h| h.host_id == o.host_id).unwrap();
            TimedResourceObservation::admit(o, windows[i].clone()).unwrap()
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
            connection_item_capacity: 4,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    };
    let comparison = compare_global_placement(
        &plot,
        inputs,
        GlobalPlacementBudget {
            maximum_states: states,
        },
        GlobalPlacementObjective::AggregateFulfillmentTime,
        None,
        &mut work,
        &mut ExactNumericWork::new(100_000_000),
    );
    inspect(&comparison);
}
#[test]
fn feasible_state_limited_search_is_best_known_never_proven_optimal() {
    compare(2, |c| {
        let global = c.global().as_ref().unwrap();
        assert_eq!(global.disposition, GlobalPlacementDisposition::StateLimit);
        assert_eq!(global.complete_assignments, 1);
        assert_eq!(global.candidate_count, 2);
        let report = render_global_placement_comparison(c).unwrap();
        assert!(report.contains("best-known feasible assignment, optimality unproven"));
        assert!(!report.contains("optimal for the finite admitted cost model"));
    });
}
#[test]
fn exhaustive_two_provider_search_reports_same_metric_for_both_sealed_plans() {
    compare(32, |c| {
        let global = c.global().as_ref().unwrap();
        assert_eq!(global.disposition, GlobalPlacementDisposition::Exhaustive);
        assert_eq!(*c.baseline_score().unwrap().as_ref().unwrap(), global.score);
        let report = render_global_placement_comparison(c).unwrap();
        assert!(report.contains("fixture/forecast-a"));
        assert!(report.contains("fixture/forecast-b"));
        assert!(report.contains("optimal for the finite admitted cost model"));
    });
}
