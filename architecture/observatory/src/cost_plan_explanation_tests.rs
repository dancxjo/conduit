use crate::*;
use alloc::{collections::BTreeMap, vec};
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

fn exercise(inspect: impl FnOnce(CostPlannedRealizations<'_>)) {
    prepare(PerformanceIntent::Interactive, |result| {
        inspect(result.unwrap())
    });
}
fn prepare(
    intent: PerformanceIntent,
    inspect: impl FnOnce(Result<CostPlannedRealizations<'_>, CostPlanningError<'_>>),
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
    let now = MonotonicInstant::new(105, clock).unwrap();
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
        forecasts: &forecasts,
        requirements: &requirements,
        tie_policy: &tie_policy,
        intent,
        unavailable: UnavailableCostPolicy::RequireKnown,
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
    let planned = plan_selected_realizations_with_costs(
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
        &mut work,
    );
    inspect(planned);
}

#[test]
fn ordinary_cost_plan_report_keeps_exact_plan_and_producing_sign() {
    exercise(|planned| {
        let report = render_cost_planned_realizations(&planned).unwrap();
        assert!(report.contains(planned.plan.plan_id.as_str()));
        assert!(report.contains("plan-explanation/forecast"));
        assert!(report.contains("historical planner cost decision"));
        assert!(report.len() <= MAXIMUM_COST_EXPLANATION_BYTES);
    });
}

#[test]
fn changed_plan_or_decision_subject_cannot_borrow_another_cost_explanation() {
    exercise(|mut planned| {
        planned.plan.fragments[0].placements[0].boot_id = "wrong-boot".into();
        assert_eq!(
            render_cost_planned_realizations(&planned),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
    });
    exercise(|mut planned| {
        let selected = planned.decisions[0]
            .realization
            .signs
            .iter_mut()
            .find(|record| record.disposition == RealizationDecisionDisposition::Selected)
            .unwrap();
        selected.boot_id = "wrong-boot".into();
        assert_eq!(
            render_cost_planned_realizations(&planned),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
    });
}

#[test]
fn foreign_gear_missing_decision_and_oversized_plan_refuse() {
    exercise(|mut planned| {
        planned.decisions[0].realization.signs[0].gear_id = "foreign-gear".into();
        assert_eq!(
            render_cost_planned_realizations(&planned),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
    });
    exercise(|mut planned| {
        planned.decisions.clear();
        assert_eq!(
            render_cost_planned_realizations(&planned),
            Err(CostExplanationRefusal::Extent)
        );
    });
    exercise(|mut planned| {
        planned.plan.fragments =
            vec![planned.plan.fragments[0].clone(); MAXIMUM_COST_PLANNING_OPERATIONS + 1];
        assert_eq!(
            render_cost_planned_realizations(&planned),
            Err(CostExplanationRefusal::Extent)
        );
    });
}

#[test]
fn actual_unsupported_intent_refusal_retains_ordinary_account_without_claiming_a_plan() {
    prepare(PerformanceIntent::BoundedResponse, |result| {
        let error = result.unwrap_err();
        let report = render_cost_planning_refusal(&error).unwrap();
        assert!(report.contains("no committed Plan was produced"));
        assert!(report.contains("BoundedResponse"));
        assert!(report.len() <= MAXIMUM_COST_EXPLANATION_BYTES);
    });
    assert_eq!(
        render_cost_planning_refusal(&CostPlanningError::Input(CostEvidenceRefusal::InputLimit)),
        Err(CostExplanationRefusal::NoRetainedDecision)
    );
}
