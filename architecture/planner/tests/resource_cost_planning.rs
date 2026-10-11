//! Ordinary Plan construction must consume shared capacity after cost preference.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use std::collections::BTreeMap;

fn plot() -> conduit_plot::CheckedPlot {
    conduit_plot::parse_with_startup(
        "plot costs {\n first: flow/pulse(count = 2, period-ms = 0, initial = false)\n second: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(), &conduit_signal::signal_profile_catalog(),
    ).unwrap()
}
fn forecast(ms: i128) -> FulfillmentCostForecast {
    let mut work = ExactNumericWork::new(100_000_000);
    let mut point = |value| {
        let quantity = RationalQuantity::new(
            ExactRational::from_i128(value),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap();
        let duration = CostDuration::admit(quantity, &mut work).unwrap();
        CostDurationRange::admit(duration, duration, &mut work).unwrap()
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
    capacity_a: u32,
    reverse: bool,
    stale: bool,
    budget: u64,
    inspect: impl FnOnce(
        Result<CostPlannedRealizations<'_>, CostPlanningError<'_>>,
        &[HostAdvertisement],
    ),
) {
    let plot = plot();
    let hosts = common::competing_hosts();
    let windows: Vec<_> = hosts
        .iter()
        .map(|host| {
            let clock = MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "cost-planning/clock".into(),
                TemporalScale::Milliseconds,
                1,
                0,
            )
            .unwrap();
            CostObservationWindow::admit(
                MonotonicInstant::new(100, clock.clone()).unwrap(),
                MonotonicInstant::new(110, clock).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let times: Vec<_> = windows
        .iter()
        .map(|window| {
            MonotonicInstant::new(
                if stale { 110 } else { 105 },
                window.observed().clock().clone(),
            )
            .unwrap()
        })
        .collect();
    let calibration_id: ArtifactId = "cost-planning/calibration".into();
    let calibration = [CostCalibrationRevision::new(&calibration_id, 1).unwrap()];
    let contexts: Vec<_> = hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &calibration,
            now,
            execution_observations: &[],
        })
        .collect();
    let signs: Vec<SignId> = (0..2)
        .map(|i| format!("cost-planning/forecast-{i}").into())
        .collect();
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
    let forecasts = [forecast(10), forecast(1)];
    let bound: Vec<Vec<_>> = plot
        .gears
        .iter()
        .map(|gear| {
            let demand = CostDemand::admit(gear, &[]).unwrap();
            sources
                .iter()
                .zip(&forecasts)
                .map(|(source, forecast)| CostedForecast::bind(source, demand, forecast).unwrap())
                .collect()
        })
        .collect();
    let requirements = HardRealizationRequirements::default();
    let policy = RealizationPolicy::default();
    let mut operations: Vec<_> = plot
        .gears
        .iter()
        .zip(&bound)
        .map(|(gear, forecasts)| CostPlanningOperation {
            demand: CostDemand::admit(gear, &[]).unwrap(),
            forecasts,
            requirements: &requirements,
            tie_policy: &policy,
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::RequireKnown,
        })
        .collect();
    if reverse {
        operations.reverse();
    }
    let mut observations = common::resource_observations(&hosts);
    for observation in &mut observations {
        if observation.class_id == hosts[0].capabilities[0].resource_requirements[0].class_id {
            observation.unreserved_units = if observation.host_id == hosts[0].host_id {
                capacity_a
            } else {
                3
            };
        }
    }
    let timed: Vec<_> = observations
        .iter()
        .map(|observation| {
            let index = hosts
                .iter()
                .position(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, windows[index].clone()).unwrap()
        })
        .collect();
    let empty = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty,
        line_candidates: &BTreeMap::new(),
        connection_item_capacity: 4,
        connection_byte_capacity: 4096,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let result = plan_selected_realizations_with_costs(
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
        &mut ExactNumericWork::new(budget),
    );
    inspect(result, &hosts);
    // Preparation cannot turn observations into an authoritative reservation.
    assert_eq!(
        observations
            .iter()
            .find(|o| o.host_id == hosts[1].host_id
                && o.class_id == hosts[1].capabilities[0].resource_requirements[0].class_id)
            .unwrap()
            .unreserved_units,
        3
    );
}

#[test]
fn cheaper_cost_wins_then_remaining_capacity_forces_the_next_gear_elsewhere() {
    exercise(1, false, false, 100_000_000, |result, hosts| {
        let result = result.unwrap();
        assert!(verify_plan(&result.plan));
        assert_eq!(result.decisions.len(), 2);
        assert_eq!(
            result.decisions[0].realization.choice.host_id,
            hosts[1].host_id
        );
        assert_eq!(
            result.decisions[1].realization.choice.host_id,
            hosts[0].host_id
        );
        assert!(result
            .decisions
            .iter()
            .all(|decision| decision.used_cost_preference));
        assert!(result.decisions[1]
            .realization
            .signs
            .iter()
            .any(|record| record.host_id == hosts[1].host_id
                && matches!(
                    record.disposition,
                    RealizationDecisionDisposition::Rejected(_)
                )));
        for decision in &result.decisions {
            let selected: Vec<_> = decision
                .realization
                .signs
                .iter()
                .filter(|record| record.disposition == RealizationDecisionDisposition::Selected)
                .collect();
            assert_eq!(selected.len(), 1);
            let selected = selected[0];
            let exact = result
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .find(|placement| placement.gear_id == selected.gear_id)
                .unwrap();
            assert_eq!(exact.host_id, selected.host_id);
            assert_eq!(exact.boot_id, selected.boot_id);
            assert_eq!(exact.offer_generation, selected.offer_generation);
            assert_eq!(exact.capability_id, selected.capability_id);
            assert_eq!(exact.implementation_id, selected.implementation_id);
            assert_eq!(exact.artifact_id, selected.artifact_id);
            let choice = &decision.realization.choice;
            assert!(result
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .any(|placement| placement.host_id == choice.host_id
                    && placement.capability_id == choice.capability_id));
        }
    });
}

#[test]
fn later_capacity_refusal_retains_prior_attempt_without_returning_a_partial_plan() {
    exercise(0, false, false, 100_000_000, |result, hosts| {
        let Err(CostPlanningError::Selection {
            error, preceding, ..
        }) = result
        else {
            panic!("second Gear must refuse with its admission account")
        };
        let CostSelectionError::HardAdmission(refused) = *error else {
            panic!("must retain the later hard-admission refusal");
        };
        assert_eq!(preceding.len(), 1);
        assert_eq!(preceding[0].realization.choice.host_id, hosts[1].host_id);
        assert_eq!(refused.signs.len(), 2);
    });
}

#[test]
fn reversed_checked_operations_refuse_before_any_selection() {
    exercise(1, true, false, 100_000_000, |result, _| {
        assert!(matches!(
            result,
            Err(CostPlanningError::Input(
                CostEvidenceRefusal::WorkClassChanged
            ))
        ));
    });
}

#[test]
fn expired_capacity_windows_and_exhausted_comparisons_cannot_create_plans() {
    exercise(1, false, true, 100_000_000, |result, _| {
        let Err(CostPlanningError::Selection {
            error, preceding, ..
        }) = result
        else {
            panic!("expired capacity must refuse before Plan construction");
        };
        assert!(preceding.is_empty());
        assert!(matches!(
            *error,
            CostSelectionError::Evidence(CostEvidenceRefusal::Stale)
        ));
    });
    exercise(1, false, false, 0, |result, _| {
        let Err(CostPlanningError::Selection {
            error, preceding, ..
        }) = result
        else {
            panic!("exhausted arithmetic must refuse before Plan construction");
        };
        assert!(preceding.is_empty());
        assert!(matches!(*error, CostSelectionError::Comparison(_)));
    });
}

#[test]
fn incomplete_or_over_bound_checked_operation_sets_refuse_without_arithmetic() {
    for count in [2, MAXIMUM_COST_PLANNING_OPERATIONS + 1] {
        let mut source = String::from("plot bounded_costs {\n");
        for index in 0..count {
            source.push_str(&format!(
                " pulse{index}: flow/pulse(count = 2, period-ms = 0, initial = false)\n"
            ));
        }
        source.push_str("}\n");
        let plot = conduit_plot::parse_with_startup(
            &source,
            &conduit_signal::signal_startup_catalog(),
            &conduit_signal::signal_profile_catalog(),
        )
        .unwrap();
        let requirements = HardRealizationRequirements::default();
        let policy = RealizationPolicy::default();
        let mut operations: Vec<_> = plot
            .gears
            .iter()
            .map(|gear| CostPlanningOperation {
                demand: CostDemand::admit(gear, &[]).unwrap(),
                forecasts: &[],
                requirements: &requirements,
                tie_policy: &policy,
                intent: PerformanceIntent::Interactive,
                unavailable: UnavailableCostPolicy::RequireKnown,
            })
            .collect();
        if count == 2 {
            operations.pop();
        }
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
        let mut work = ExactNumericWork::new(100_000_000);
        let result = plan_selected_realizations_with_costs(
            &plot,
            CostPlanningInputs {
                hosts: &[],
                contexts: &[],
                operations: &operations,
                advertisements: &[],
                observations: &[],
                bases: &[],
                options,
            },
            &mut work,
        );
        assert!(matches!(
            result,
            Err(CostPlanningError::Input(CostEvidenceRefusal::InputLimit))
        ));
        assert_eq!(work.spent(), 0);
    }
}
