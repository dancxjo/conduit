use crate::*;
use alloc::collections::BTreeMap;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

fn prepare_comparison(
    missing: bool,
    stale: bool,
    states: u32,
    inspect: impl FnOnce(&GlobalPlacementComparison<'_, '_>),
) {
    let plot = conduit_plot::parse_with_startup(
        "plot costs {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap();
    let mut host = conduit_signal_conformance::pico_local_advertisement();
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() == conduit_signal::PULSE_KIND);
    let hosts = [host];
    let clock = MonotonicClockIdentity::new(
        hosts[0].host_id.clone(),
        hosts[0].boot_id.clone(),
        "plan-explanation/clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let window = CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(110, clock.clone()).unwrap(),
    )
    .unwrap();
    let now = MonotonicInstant::new(if stale { 120 } else { 105 }, clock).unwrap();
    let calibration_id: ArtifactId = "plan-explanation/calibration".into();
    let revision = [CostCalibrationRevision::new(&calibration_id, 1).unwrap()];
    let contexts = [CostEvaluationContext {
        host: &hosts[0],
        calibrations: &revision,
        now: &now,
        execution_observations: &[],
    }];
    let source_sign: SignId = "plan-explanation/forecast".into();
    let source = CostEvidenceSource::bind(
        &hosts[0],
        &hosts[0].capabilities[0],
        None,
        &source_sign,
        revision[0],
        window.clone(),
    )
    .unwrap();
    let mut work = ExactNumericWork::new(100_000_000);
    let mut point = |value| {
        let value = RationalQuantity::new(
            ExactRational::from_i128(value),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap();
        let duration = CostDuration::admit(value, &mut work).unwrap();
        CostDurationRange::admit(duration, duration, &mut work).unwrap()
    };
    let stages = CostForecastStages {
        setup: Some(point(0)),
        queue: Some(point(0)),
        service: Some(point(1)),
        transfer: Some(point(0)),
    };
    let forecast = FulfillmentCostForecast::sequential(stages, &mut work).unwrap();
    let demand = CostDemand::admit(&plot.gears[0], &[]).unwrap();
    let forecasts = [CostedForecast::bind(&source, demand, &forecast).unwrap()];
    let requirements = HardRealizationRequirements::default();
    let tie_policy = RealizationPolicy::default();
    let operations = [CostPlanningOperation {
        demand,
        forecasts: if missing { &[] } else { &forecasts },
        requirements: &requirements,
        tie_policy: &tie_policy,
        intent: PerformanceIntent::Interactive,
        unavailable: if missing {
            UnavailableCostPolicy::IgnoreCostsIfIncomplete
        } else {
            UnavailableCostPolicy::RequireKnown
        },
    }];
    let observations: alloc::vec::Vec<_> = hosts[0]
        .resources
        .iter()
        .enumerate()
        .map(|(i, pool)| ResourceObservation {
            host_id: hosts[0].host_id.clone(),
            boot_id: hosts[0].boot_id.clone(),
            offer_generation: hosts[0].offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: alloc::format!("plan-explanation/resource-{i}").into(),
        })
        .collect();
    let timed: alloc::vec::Vec<_> = observations
        .iter()
        .map(|observation| TimedResourceObservation::admit(observation, window.clone()).unwrap())
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 4,
        connection_byte_capacity: 4096,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let planned = compare_global_placement(
        &plot,
        CostPlanningInputs {
            hosts: &hosts,
            contexts: &contexts,
            operations: &operations,
            advertisements: &[],
            observations: &timed,
            bases: &[],
            options,
        },
        GlobalPlacementBudget {
            maximum_states: states,
        },
        GlobalPlacementObjective::AggregateFulfillmentTime,
        None,
        &mut work,
        &mut ExactNumericWork::new(100_000_000),
    );
    inspect(&planned);
}

#[test]
fn same_inputs_same_metric_and_exact_plan_identity_render_without_mutation() {
    prepare_comparison(false, false, 32, |comparison| {
        let baseline = comparison.baseline().as_ref().unwrap();
        let global = comparison.global().as_ref().unwrap();
        assert_eq!(
            comparison.baseline_score().unwrap().as_ref().unwrap(),
            &global.score
        );
        let before = global.plan.clone();
        let resources = comparison.inputs().hosts[0].resources.clone();
        let report = render_global_placement_comparison(comparison).unwrap();
        assert!(report.contains("same immutable admitted inputs"));
        assert!(report.contains(baseline.plan.plan_id.as_str()));
        assert!(report.contains(global.plan.plan_id.as_str()));
        assert!(report.contains("plan-explanation/forecast"));
        assert!(report.contains("optimal for the finite admitted cost model"));
        assert_eq!(global.plan, before);
        assert_eq!(comparison.inputs().hosts[0].resources, resources);
    });
}
#[test]
fn missing_and_stale_forecasts_are_never_a_zero_cost_optimum() {
    prepare_comparison(true, false, 32, |comparison| {
        let report = render_global_placement_comparison(comparison).unwrap();
        assert!(report.contains("unknown_operations=1"));
        assert!(report.contains("complete objective unavailable"));
        assert!(!report.contains("optimal for the finite admitted cost model"));
    });
    prepare_comparison(false, true, 32, |comparison| {
        let report = render_global_placement_comparison(comparison).unwrap();
        assert!(report.contains("baseline refused"));
        assert!(report.contains("global placement refused"));
        assert!(!report.contains("optimal for the finite admitted cost model"));
    });
}
#[test]
fn search_exhaustion_before_assignment_keeps_no_feasible_plan_explicit() {
    prepare_comparison(false, false, 1, |comparison| {
        let report = render_global_placement_comparison(comparison).unwrap();
        assert!(report.contains("maximum_states=1"));
        assert!(report.contains("StateLimit"));
        assert!(report.contains("no feasible executable Plan claimed"));
        assert!(!report.contains("optimal for the finite admitted cost model"));
    });
}
