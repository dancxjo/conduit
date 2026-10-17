//! Independent current-state custody through the existing generic selector.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn window(host: &HostAdvertisement, observed: u64, expires: u64) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "current-state-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    CostObservationWindow::admit(
        MonotonicInstant::new(observed, clock.clone()).unwrap(),
        MonotonicInstant::new(expires, clock).unwrap(),
    )
    .unwrap()
}
fn forecast(ms: i128) -> FulfillmentCostForecast {
    let stage = |value| {
        let time = CostDuration::admit(
            RationalQuantity::new(
                ExactRational::from_i128(value),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap(),
            &mut work(),
        )
        .unwrap();
        CostDurationRange::admit(time, time, &mut work()).unwrap()
    };
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(stage(0)),
            queue: Some(stage(0)),
            service: Some(stage(ms)),
            transfer: Some(stage(0)),
        },
        &mut work(),
    )
    .unwrap()
}
#[test]
fn independent_current_residency_controls_use_and_retains_refusal_custody() {
    let hosts = common::competing_hosts();
    let gear = common::pulse_gear();
    let demand = CostDemand::admit(&gear, &[]).unwrap();
    let metric = ArtifactId::from("fixture/execution-metric");
    let revisions = [CostCalibrationRevision::new(&metric, 1).unwrap()];
    let signs = [SignId::from("forecast/a"), SignId::from("forecast/b")];
    let sources = hosts
        .iter()
        .zip(&signs)
        .map(|(host, sign)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                sign,
                revisions[0],
                window(host, 90, 200),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let assumption_sign = SignId::from("condition/forecast-assumption");
    let assumption_source = CostEvidenceSource::bind(
        &hosts[0],
        &hosts[0].capabilities[0],
        None,
        &assumption_sign,
        revisions[0],
        window(&hosts[0], 90, 100),
    )
    .unwrap();
    let warm = CostExecutionCondition::Warm {
        residency_revision: 7,
    };
    let assumption = CostExecutionObservation::admit(&assumption_source, warm).unwrap();
    let durations = [forecast(1), forecast(10)];
    let samples = [
        CostedForecast::bind(&sources[0], demand, &durations[0])
            .unwrap()
            .with_execution_assumption(&assumption)
            .unwrap(),
        CostedForecast::bind(&sources[1], demand, &durations[1]).unwrap(),
    ];
    let observations = common::resource_observations(&hosts);
    let timed = observations
        .iter()
        .map(|observation| {
            let host = hosts
                .iter()
                .find(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, window(host, 100, 200)).unwrap()
        })
        .collect::<Vec<_>>();
    let times = hosts
        .iter()
        .map(|host| {
            MonotonicInstant::new(105, window(host, 100, 200).observed().clock().clone()).unwrap()
        })
        .collect::<Vec<_>>();
    let requirements = HardRealizationRequirements::default();
    let tie = RealizationPolicy::default();
    for (condition, observed, expires, refusal) in [
        (Some(warm), 100, 200, None),
        (
            Some(CostExecutionCondition::Cold),
            100,
            200,
            Some(CostEvidenceRefusal::ExecutionConditionChanged),
        ),
        (
            Some(CostExecutionCondition::Unknown),
            100,
            200,
            Some(CostEvidenceRefusal::ExecutionConditionChanged),
        ),
        (
            Some(CostExecutionCondition::Warm {
                residency_revision: 8,
            }),
            100,
            200,
            Some(CostEvidenceRefusal::ExecutionConditionChanged),
        ),
        (Some(warm), 100, 105, Some(CostEvidenceRefusal::Stale)),
        (
            Some(warm),
            106,
            200,
            Some(CostEvidenceRefusal::FutureObservation),
        ),
        (
            None,
            100,
            200,
            Some(CostEvidenceRefusal::MissingExecutionObservation),
        ),
    ] {
        let current_sign = SignId::from("condition/current-independent");
        let current_source = CostEvidenceSource::bind(
            &hosts[0],
            &hosts[0].capabilities[0],
            None,
            &current_sign,
            revisions[0],
            window(&hosts[0], observed, expires),
        )
        .unwrap();
        let current =
            condition.map(|state| CostExecutionObservation::admit(&current_source, state).unwrap());
        let contexts = hosts
            .iter()
            .zip(&times)
            .map(|(host, now)| CostEvaluationContext {
                host,
                calibrations: &revisions,
                now,
                execution_observations: if host.host_id == hosts[0].host_id {
                    current.as_slice()
                } else {
                    &[]
                },
            })
            .collect::<Vec<_>>();
        let selection = select_realization_with_costs(
            demand,
            CostSelectionInputs {
                hosts: &hosts,
                contexts: &contexts,
                advertisements: &[],
                requirements: &requirements,
                observations: &timed,
                tie_policy: &tie,
                forecasts: &samples,
                intent: PerformanceIntent::Interactive,
                unavailable: UnavailableCostPolicy::PreferKnown,
            },
            &mut work(),
        )
        .unwrap();
        let winner = if refusal.is_none() { 0 } else { 1 };
        assert_eq!(selection.realization.choice.host_id, hosts[winner].host_id);
        let cost = selection
            .costs
            .iter()
            .find(|cost| cost.host_id == &hosts[0].host_id)
            .unwrap();
        assert_eq!(cost.expected_execution_condition, Some(warm));
        assert_eq!(
            cost.submitted_execution_assumption.unwrap().sign(),
            &assumption_sign
        );
        match &refusal {
            None => assert!(matches!(cost.availability, CostAvailability::Known(_))),
            Some(reason) => assert!(
                matches!(&cost.availability, CostAvailability::Unusable(actual) if actual == reason)
            ),
        }
        assert_eq!(
            cost.current_execution.map(|sample| sample.source().sign()),
            current.as_ref().map(|_| &current_sign)
        );
        if let Some(reason) = &refusal {
            let error = select_realization_with_costs(
                demand,
                CostSelectionInputs {
                    hosts: &hosts,
                    contexts: &contexts,
                    advertisements: &[],
                    requirements: &requirements,
                    observations: &timed,
                    tie_policy: &tie,
                    forecasts: &samples[..1],
                    intent: PerformanceIntent::Interactive,
                    unavailable: UnavailableCostPolicy::RequireKnown,
                },
                &mut work(),
            )
            .unwrap_err();
            let CostSelectionError::NoUsableForecast { costs, .. } = error else {
                panic!("missing retained refusal");
            };
            let cost = costs
                .iter()
                .find(|cost| cost.host_id == hosts[0].host_id)
                .unwrap();
            assert_eq!(cost.reason.as_ref(), Some(reason));
            assert_eq!(cost.expected_execution_condition, Some(warm));
            assert_eq!(
                cost.submitted_execution_assumption
                    .as_ref()
                    .unwrap()
                    .sign_id,
                assumption_sign
            );
            assert_eq!(
                cost.current_execution
                    .as_ref()
                    .map(|(source, _)| &source.sign_id),
                current.as_ref().map(|_| &current_sign)
            );
        }
    }
    let contradictory = [
        CostExecutionObservation::admit(&sources[0], warm).unwrap(),
        CostExecutionObservation::admit(&sources[0], CostExecutionCondition::Cold).unwrap(),
    ];
    let contexts = hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &revisions,
            now,
            execution_observations: if host.host_id == hosts[0].host_id {
                &contradictory
            } else {
                &[]
            },
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        select_realization_with_costs(
            demand,
            CostSelectionInputs {
                hosts: &hosts,
                contexts: &contexts,
                advertisements: &[],
                requirements: &requirements,
                observations: &timed,
                tie_policy: &tie,
                forecasts: &samples,
                intent: PerformanceIntent::Interactive,
                unavailable: UnavailableCostPolicy::PreferKnown,
            },
            &mut work()
        ),
        Err(CostSelectionError::Evidence(
            CostEvidenceRefusal::AmbiguousEvidence
        ))
    ));
    assert!(matches!(
        CostExecutionObservation::admit(
            &assumption_source,
            CostExecutionCondition::Warm {
                residency_revision: 0
            }
        ),
        Err(CostEvidenceRefusal::InvalidIdentity)
    ));
    let foreign = CostExecutionObservation::admit(&sources[1], warm).unwrap();
    assert!(matches!(
        CostedForecast::bind(&sources[0], demand, &durations[0])
            .unwrap()
            .with_execution_assumption(&foreign),
        Err(CostEvidenceRefusal::UnadvertisedSubject)
    ));
}
