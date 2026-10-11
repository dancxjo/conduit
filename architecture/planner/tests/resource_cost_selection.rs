//! Cost preference uses real ordinary Planner hard admission and its records.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

#[derive(Clone)]
struct Case {
    durations: [i128; 2],
    sample_expiry: [u64; 2],
    hard_expiry: u64,
    missing: bool,
    unavailable: UnavailableCostPolicy,
    capacity_b: u32,
    withdraw_all: bool,
    minimum_items: u16,
    calibration_revision: u64,
    different_work: bool,
    budget: u64,
    intent: PerformanceIntent,
}
impl Default for Case {
    fn default() -> Self {
        Self {
            durations: [10, 1],
            sample_expiry: [110, 110],
            hard_expiry: 110,
            missing: false,
            unavailable: UnavailableCostPolicy::PreferKnown,
            capacity_b: 4,
            withdraw_all: false,
            minimum_items: 0,
            calibration_revision: 1,
            different_work: false,
            budget: 100_000_000,
            intent: PerformanceIntent::Interactive,
        }
    }
}
fn window(host: &HostAdvertisement, expiry: u64) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "cost-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(expiry, clock).unwrap(),
    )
    .unwrap()
}
fn forecast(ms: i128) -> FulfillmentCostForecast {
    let duration = |ms| {
        CostDuration::admit(
            RationalQuantity::new(
                ExactRational::from_i128(ms),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap(),
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap()
    };
    let point = |ms| {
        let value = duration(ms);
        CostDurationRange::admit(value, value, &mut ExactNumericWork::new(100_000_000)).unwrap()
    };
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(point(0)),
            queue: Some(point(0)),
            service: Some(point(ms)),
            transfer: Some(point(0)),
        },
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap()
}
fn exercise(
    case: Case,
    inspect: impl FnOnce(Result<CostRealizationSelection<'_>, CostSelectionError>),
) {
    let hosts = common::competing_hosts();
    let gear = common::pulse_gear();
    let mut measured_gear = gear.clone();
    if case.different_work {
        measured_gear
            .configuration
            .iter_mut()
            .find(|entry| entry.key == "count")
            .unwrap()
            .value = ConfigurationValue::U64(200);
    }
    let demand = CostDemand::admit(&gear, &[]).unwrap();
    let measured = CostDemand::admit(&measured_gear, &[]).unwrap();
    let mut observations = common::resource_observations(&hosts);
    for observation in &mut observations {
        if observation.host_id == hosts[1].host_id
            && hosts[1].capabilities[0]
                .resource_requirements
                .iter()
                .any(|requirement| requirement.class_id == observation.class_id)
        {
            observation.unreserved_units = case.capacity_b;
        }
    }
    if case.withdraw_all {
        for observation in &mut observations {
            observation.unreserved_units = 0;
        }
    }
    let timed = observations
        .iter()
        .map(|observation| {
            let host = hosts
                .iter()
                .find(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, window(host, case.hard_expiry)).unwrap()
        })
        .collect::<Vec<_>>();
    let profile = ArtifactId::from("calibration/pulse@1");
    let original = CostCalibrationRevision::new(&profile, 1).unwrap();
    let current = [CostCalibrationRevision::new(&profile, case.calibration_revision).unwrap()];
    let signs = [SignId::from("sign/cost/a"), SignId::from("sign/cost/b")];
    let sources = hosts
        .iter()
        .enumerate()
        .map(|(index, host)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                &signs[index],
                original,
                window(host, case.sample_expiry[index]),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let times = hosts
        .iter()
        .map(|host| {
            MonotonicInstant::new(105, window(host, 110).observed().clock().clone()).unwrap()
        })
        .collect::<Vec<_>>();
    let contexts = hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| CostEvaluationContext {
            execution_observations: &[],
            host,
            calibrations: &current,
            now,
        })
        .collect::<Vec<_>>();
    let forecasts = case.durations.map(forecast);
    let samples = if case.missing {
        Vec::new()
    } else {
        sources
            .iter()
            .zip(&forecasts)
            .map(|(source, forecast)| CostedForecast::bind(source, measured, forecast).unwrap())
            .collect::<Vec<_>>()
    };
    let requirements = HardRealizationRequirements {
        minimum_queue_items: case.minimum_items,
        ..Default::default()
    };
    let policy = RealizationPolicy::default();
    inspect(select_realization_with_costs(
        demand,
        CostSelectionInputs {
            hosts: &hosts,
            contexts: &contexts,
            advertisements: &[],
            requirements: &requirements,
            observations: &timed,
            tie_policy: &policy,
            forecasts: &samples,
            intent: case.intent,
            unavailable: case.unavailable,
        },
        &mut ExactNumericWork::new(case.budget),
    ));
}
#[test]
fn faster_estimate_selects_a_real_admitted_offer_with_sources_and_factors() {
    exercise(Case::default(), |result| {
        let selected = result.unwrap();
        assert_eq!(
            selected.realization.choice.host_id.as_str(),
            "host-b-capable"
        );
        assert!(selected.used_cost_preference);
        assert_eq!(
            selected
                .realization
                .signs
                .iter()
                .filter(|record| record.disposition == RealizationDecisionDisposition::Selected)
                .count(),
            1
        );
        let winner = selected.costs.iter().find(|item| item.selected).unwrap();
        let CostAvailability::Known(sample) = &winner.availability else {
            panic!("known cost required")
        };
        assert_eq!(sample.source().sign().as_str(), "sign/cost/b");
        assert_eq!(
            sample
                .demand()
                .class()
                .gear()
                .configuration
                .iter()
                .find(|entry| entry.key == "count")
                .unwrap()
                .value,
            ConfigurationValue::U64(2)
        );
        assert_eq!(
            sample.forecast().upper_seconds(),
            ExactRational::ratio(1, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap()
        );
    });
}
#[test]
fn costs_cannot_erase_fresh_capacity_or_hard_queue_refusals() {
    for (case, rejected_host, rejection) in [
        (
            Case {
                capacity_b: 0,
                ..Default::default()
            },
            "host-b-capable",
            RealizationRejection::CurrentResourceObservation,
        ),
        (
            Case {
                minimum_items: 6,
                durations: [1, 100],
                ..Default::default()
            },
            "host-a-efficient",
            RealizationRejection::QueueItemBound,
        ),
    ] {
        exercise(case, |result| {
            let selected = result.unwrap();
            let rejected = selected
                .realization
                .signs
                .iter()
                .find(|record| record.host_id.as_str() == rejected_host)
                .unwrap();
            assert_eq!(
                rejected.disposition,
                RealizationDecisionDisposition::Rejected(rejection)
            );
            assert_ne!(selected.realization.choice.host_id.as_str(), rejected_host);
            assert!(selected
                .costs
                .iter()
                .all(|record| record.host_id.as_str() != rejected_host));
        });
    }
}
#[test]
fn stale_costs_and_changed_calibration_or_work_are_unusable_not_zero() {
    exercise(
        Case {
            sample_expiry: [110, 104],
            ..Default::default()
        },
        |result| {
            let selected = result.unwrap();
            assert_eq!(
                selected.realization.choice.host_id.as_str(),
                "host-a-efficient"
            );
            assert!(matches!(
                selected.costs[1].availability,
                CostAvailability::Unusable(CostEvidenceRefusal::Stale)
            ));
        },
    );
    for case in [
        Case {
            calibration_revision: 2,
            ..Default::default()
        },
        Case {
            different_work: true,
            ..Default::default()
        },
    ] {
        exercise(
            Case {
                unavailable: UnavailableCostPolicy::RequireKnown,
                ..case
            },
            |result| {
                assert!(matches!(
                    result,
                    Err(CostSelectionError::NoUsableForecast { .. })
                ));
            },
        );
    }
}
#[test]
fn missing_costs_follow_explicit_fallback_or_refusal() {
    exercise(
        Case {
            missing: true,
            unavailable: UnavailableCostPolicy::IgnoreCostsIfIncomplete,
            ..Default::default()
        },
        |result| {
            let selected = result.unwrap();
            assert_eq!(
                selected.realization.choice.host_id.as_str(),
                "host-a-efficient"
            );
            assert!(!selected.used_cost_preference);
            assert!(selected
                .costs
                .iter()
                .all(|item| matches!(item.availability, CostAvailability::Missing)));
        },
    );
    exercise(
        Case {
            missing: true,
            unavailable: UnavailableCostPolicy::RequireKnown,
            ..Default::default()
        },
        |result| {
            assert!(matches!(
                result,
                Err(CostSelectionError::NoUsableForecast { .. })
            ))
        },
    );
}
#[test]
fn expired_hard_truth_work_exhaustion_and_uncertified_deadlines_refuse() {
    for (case, expected) in [
        (
            Case {
                hard_expiry: 104,
                ..Default::default()
            },
            CostEvidenceRefusal::Stale,
        ),
        (
            Case {
                budget: 0,
                ..Default::default()
            },
            CostEvidenceRefusal::Numeric(ExactNumericRefusal::WorkLimit),
        ),
        (
            Case {
                intent: PerformanceIntent::BoundedResponse,
                ..Default::default()
            },
            CostEvidenceRefusal::UnsupportedIntent,
        ),
    ] {
        exercise(case, |result| match result {
            Err(CostSelectionError::Evidence(error)) => assert_eq!(error, expected),
            Err(CostSelectionError::Policy(account)) => {
                assert_eq!(account.reason, expected);
                assert_eq!(account.intent, PerformanceIntent::BoundedResponse);
                assert_eq!(account.realization.signs.len(), 2);
            }
            Err(CostSelectionError::Comparison(account)) => {
                assert_eq!(account.reason, expected);
                assert_eq!(account.arithmetic_work_limit, 0);
                assert_eq!(account.arithmetic_work_spent, 0);
                assert_eq!(account.operands.len(), 2);
            }
            other => panic!("unexpected result: {other:?}"),
        });
    }
}
#[test]
fn exact_ties_use_the_existing_deterministic_policy_order() {
    exercise(
        Case {
            durations: [1, 1],
            ..Default::default()
        },
        |result| {
            assert_eq!(
                result.unwrap().realization.choice.host_id.as_str(),
                "host-a-efficient"
            )
        },
    );
}

#[test]
fn every_hard_refusal_is_retained_without_evaluating_favorable_costs() {
    exercise(
        Case {
            minimum_items: u16::MAX,
            budget: 0,
            ..Default::default()
        },
        |result| {
            let Err(CostSelectionError::HardAdmission(refusal)) = result else {
                panic!("hard refusal expected");
            };
            assert!(matches!(
                refusal.reason,
                PlannerError::HardRealizationRequirementUnsatisfied(_)
            ));
            assert_eq!(refusal.signs.len(), 2);
            assert!(refusal.signs.iter().all(|record| record.disposition
                == RealizationDecisionDisposition::Rejected(RealizationRejection::QueueItemBound)));
        },
    );
}

#[test]
fn every_current_resource_refusal_is_retained_before_cost_comparison() {
    exercise(
        Case {
            withdraw_all: true,
            budget: 0,
            ..Default::default()
        },
        |result| {
            let Err(CostSelectionError::HardAdmission(refusal)) = result else {
                panic!("resource refusal expected");
            };
            assert!(matches!(
                refusal.reason,
                PlannerError::CurrentResourceObservationUnavailable(_)
            ));
            assert_eq!(refusal.signs.len(), 2);
            assert!(refusal.signs.iter().all(|record| record.disposition
                == RealizationDecisionDisposition::Rejected(
                    RealizationRejection::CurrentResourceObservation
                )));
        },
    );
}

#[test]
fn exhausted_comparison_retains_sources_with_no_cost_winner() {
    exercise(
        Case {
            budget: 0,
            ..Default::default()
        },
        |result| {
            let error = result.unwrap_err();
            let account = format!("{error:?}");
            assert!(
                account.contains("sign/cost/a"),
                "comparison lost first forecast source: {account}"
            );
            assert!(
                account.contains("sign/cost/b"),
                "comparison lost second forecast source: {account}"
            );
            assert!(account.contains("host-a-efficient") && account.contains("host-b-capable"));
        },
    );
}
