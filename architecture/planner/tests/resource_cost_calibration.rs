//! Independent EWMA arithmetic, measured error and real selection consumption.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};

fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn rational(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn window(host: &HostAdvertisement, observed: u64, expiry: u64) -> CostObservationWindow {
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
        MonotonicInstant::new(observed, clock.clone()).unwrap(),
        MonotonicInstant::new(expiry, clock).unwrap(),
    )
    .unwrap()
}
fn forecast(ms: i128) -> FulfillmentCostForecast {
    let point = |ms| {
        let value = CostDuration::admit(
            RationalQuantity::new(
                ExactRational::from_i128(ms),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap(),
            &mut work(),
        )
        .unwrap();
        CostDurationRange::admit(value, value, &mut work()).unwrap()
    };
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(point(0)),
            queue: Some(point(0)),
            service: Some(point(ms)),
            transfer: Some(point(0)),
        },
        &mut work(),
    )
    .unwrap()
}
fn policy(capacity: usize, updates: usize) -> CostCalibrationPolicy {
    CostCalibrationPolicy {
        history_capacity: capacity,
        maximum_updates: updates,
        minimum_update_ticks: 10,
    }
}
#[test]
fn exact_ewma_retains_signed_errors_bounded_history_and_source_expiry() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let gear = common::pulse_gear();
    let profile = ArtifactId::from("calibration/pulse@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let sign = SignId::from("forecast/sign");
    let source = CostEvidenceSource::bind(
        &host,
        &host.capabilities[0],
        None,
        &sign,
        revision,
        window(&host, 90, 2000),
    )
    .unwrap();
    let forecast = forecast(5);
    let demand = CostDemand::admit(&gear, &[]).unwrap();
    let predicted = CostedForecast::bind(&source, demand, &forecast).unwrap();
    let dispatched =
        MonotonicInstant::new(100, source.window().observed().clock().clone()).unwrap();
    let signs = [
        SignId::from("done/one"),
        SignId::from("done/two"),
        SignId::from("done/three"),
    ];
    let sources = [111, 121, 131]
        .into_iter()
        .zip(&signs)
        .map(|(ticks, sign)| {
            CostEvidenceSource::bind(
                &host,
                &host.capabilities[0],
                None,
                sign,
                revision,
                window(&host, ticks, 1000),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let records = sources
        .iter()
        .map(|source| {
            CostCalibrationRecord::admit(
                &predicted,
                source,
                &dispatched,
                CostExecutionCondition::Cold,
                &mut work(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut model = BoundedCostCalibration::admit(policy(2, 3)).unwrap();
    for record in &records {
        model.update(record, &mut work()).unwrap();
    }
    assert_eq!(records[0].elapsed().ticks(), 11);
    assert_eq!(records[0].error_bounds().0.coordinate(), rational(4, 1000));
    assert_eq!(records[0].error_bounds().1.coordinate(), rational(8, 1000));
    let now = MonotonicInstant::new(140, dispatched.clock().clone()).unwrap();
    let estimate = model.estimate_at(&host, revision, &now).unwrap().unwrap();
    // [9,13]ms -> [11.5,15.5]ms -> [15.875,19.875]ms, alpha=1/4.
    assert_eq!(estimate.lower().seconds(), rational(127, 8000));
    assert_eq!(estimate.upper().seconds(), rational(159, 8000));
    assert_eq!(model.updates(), 3);
    assert_eq!(
        model
            .history()
            .map(|record| record.completion().sign().as_str())
            .collect::<Vec<_>>(),
        vec!["done/two", "done/three"]
    );
    assert_eq!(
        model.update(&records[0], &mut work()),
        Err(CostEvidenceRefusal::CalibrationUpdateLimit)
    );
    let expired = MonotonicInstant::new(1000, dispatched.clock().clone()).unwrap();
    assert_eq!(
        model.estimate_at(&host, revision, &expired),
        Err(CostEvidenceRefusal::Stale)
    );
    let changed = CostCalibrationRevision::new(&profile, 2).unwrap();
    assert_eq!(
        model.estimate_at(&host, changed, &now),
        Err(CostEvidenceRefusal::Scope(CostScopeChange::Calibration))
    );
}
#[test]
fn update_refusals_are_atomic_and_zero_ticks_keep_clock_uncertainty() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let gear = common::pulse_gear();
    let profile = ArtifactId::from("calibration/pulse@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let sign = SignId::from("forecast/sign");
    let source = CostEvidenceSource::bind(
        &host,
        &host.capabilities[0],
        None,
        &sign,
        revision,
        window(&host, 90, 2000),
    )
    .unwrap();
    let forecast = forecast(5);
    let predicted =
        CostedForecast::bind(&source, CostDemand::admit(&gear, &[]).unwrap(), &forecast).unwrap();
    let dispatched =
        MonotonicInstant::new(100, source.window().observed().clock().clone()).unwrap();
    let signs = [
        SignId::from("done/zero"),
        SignId::from("done/soon"),
        SignId::from("done/next"),
        SignId::from("done/warm"),
    ];
    let sources = [100, 105, 110, 120]
        .into_iter()
        .zip(&signs)
        .map(|(ticks, sign)| {
            CostEvidenceSource::bind(
                &host,
                &host.capabilities[0],
                None,
                sign,
                revision,
                window(&host, ticks, 1000),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let records = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            CostCalibrationRecord::admit(
                &predicted,
                source,
                &dispatched,
                if index == 3 {
                    CostExecutionCondition::Warm {
                        residency_revision: 1,
                    }
                } else {
                    CostExecutionCondition::Cold
                },
                &mut work(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(records[0].measured().lower().seconds(), ExactRational::ZERO);
    assert_eq!(records[0].measured().upper().seconds(), rational(2, 1000));
    assert_eq!(records[0].error_bounds().0.coordinate(), rational(-5, 1000));
    assert_eq!(records[0].error_bounds().1.coordinate(), rational(-3, 1000));
    let mut model = BoundedCostCalibration::admit(policy(2, 10)).unwrap();
    model.update(&records[0], &mut work()).unwrap();
    assert_eq!(
        model.update(&records[1], &mut work()),
        Err(CostEvidenceRefusal::CalibrationUpdateTooSoon)
    );
    assert_eq!(
        model.update(&records[2], &mut ExactNumericWork::new(0)),
        Err(CostEvidenceRefusal::Numeric(ExactNumericRefusal::WorkLimit))
    );
    assert_eq!(model.updates(), 1);
    assert_eq!(model.history().count(), 1);
    model.update(&records[2], &mut work()).unwrap();
    assert_eq!(
        model.update(&records[2], &mut work()),
        Err(CostEvidenceRefusal::DuplicateCompletion)
    );
    assert_eq!(
        model.update(&records[3], &mut work()),
        Err(CostEvidenceRefusal::ExecutionConditionChanged)
    );
    assert_eq!(model.updates(), 2);
    assert!(matches!(
        BoundedCostCalibration::admit(policy(0, 10)),
        Err(CostEvidenceRefusal::InvalidCalibrationPolicy)
    ));
    assert!(matches!(
        BoundedCostCalibration::admit(policy(2, MAXIMUM_COST_CALIBRATION_UPDATES + 1)),
        Err(CostEvidenceRefusal::InvalidCalibrationPolicy)
    ));
}
#[test]
fn measured_misprediction_changes_selection_and_expired_calibration_cannot_be_reused() {
    let hosts = common::competing_hosts();
    let gear = common::pulse_gear();
    let demand = CostDemand::admit(&gear, &[]).unwrap();
    let profile = ArtifactId::from("calibration/pulse@1");
    let revisions = [CostCalibrationRevision::new(&profile, 1).unwrap()];
    let signs = [
        SignId::from("forecast/a"),
        SignId::from("forecast/b"),
        SignId::from("completion/a"),
    ];
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
                window(host, 90, 2000),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let original = [forecast(5), forecast(10)];
    let predicted = CostedForecast::bind(&sources[0], demand, &original[0]).unwrap();
    let completion = CostEvidenceSource::bind(
        &hosts[0],
        &hosts[0].capabilities[0],
        None,
        &signs[2],
        revisions[0],
        window(&hosts[0], 200, 210),
    )
    .unwrap();
    let dispatched =
        MonotonicInstant::new(100, completion.window().observed().clock().clone()).unwrap();
    let record = CostCalibrationRecord::admit(
        &predicted,
        &completion,
        &dispatched,
        CostExecutionCondition::Cold,
        &mut work(),
    )
    .unwrap();
    let mut model = BoundedCostCalibration::admit(policy(2, 10)).unwrap();
    model.update(&record, &mut work()).unwrap();
    let now = MonotonicInstant::new(205, dispatched.clock().clone()).unwrap();
    let estimate = model
        .snapshot_at(&hosts[0], revisions[0], &now)
        .unwrap()
        .unwrap();
    let samples = [
        CostedForecast::bind(&sources[0], demand, &original[0])
            .unwrap()
            .with_calibration(&estimate, CostExecutionCondition::Cold, &now)
            .unwrap(),
        CostedForecast::bind(&sources[1], demand, &original[1]).unwrap(),
    ];
    assert_eq!(samples[0].forecast().upper_seconds(), rational(5, 1000));
    assert_eq!(samples[0].upper_seconds(), rational(102, 1000));
    assert_eq!(samples[0].calibration().unwrap().source().sign(), &signs[2]);
    let observations = common::resource_observations(&hosts);
    let timed = observations
        .iter()
        .map(|observation| {
            let host = hosts
                .iter()
                .find(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, window(host, 200, 2000)).unwrap()
        })
        .collect::<Vec<_>>();
    let requirements = HardRealizationRequirements::default();
    let tie_policy = RealizationPolicy::default();
    for tick in [205, 215] {
        let times = hosts
            .iter()
            .map(|host| {
                MonotonicInstant::new(tick, window(host, 90, 2000).observed().clock().clone())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let contexts = hosts
            .iter()
            .zip(&times)
            .map(|(host, now)| CostEvaluationContext {
                host,
                calibrations: &revisions,
                now,
            })
            .collect::<Vec<_>>();
        let selected = select_realization_with_costs(
            demand,
            CostSelectionInputs {
                hosts: &hosts,
                contexts: &contexts,
                advertisements: &[],
                requirements: &requirements,
                observations: &timed,
                tie_policy: &tie_policy,
                forecasts: &samples,
                intent: PerformanceIntent::Interactive,
                unavailable: UnavailableCostPolicy::PreferKnown,
            },
            &mut work(),
        )
        .unwrap();
        assert_eq!(selected.realization.choice.host_id, hosts[1].host_id);
        if tick == 215 {
            assert!(matches!(
                selected.costs[0].availability,
                CostAvailability::Unusable(CostEvidenceRefusal::Stale)
            ));
        } else {
            assert!(matches!(
                selected.costs[0].availability,
                CostAvailability::Known(_)
            ));
        }
    }
}

#[test]
fn identical_kind_on_one_host_cannot_mix_realization_measurements() {
    let mut host = common::competing_hosts().into_iter().next().unwrap();
    let mut second = host.capabilities[0].clone();
    second.capability_id = "second/pulse".into();
    second.implementation.implementation_id = "second/pulse@1".into();
    second.implementation.artifact_id = "second/pulse/artifact".into();
    host.capabilities.push(second);
    let gear = common::pulse_gear();
    let demand = CostDemand::admit(&gear, &[]).unwrap();
    let profile = ArtifactId::from("calibration/pulse@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let signs: [SignId; 4] = [
        "forecast/first",
        "forecast/second",
        "done/first",
        "done/second",
    ]
    .map(Into::into);
    let bind = |offer, sign, observed| {
        CostEvidenceSource::bind(
            &host,
            &host.capabilities[offer],
            None,
            &signs[sign],
            revision,
            window(&host, observed, 2000),
        )
        .unwrap()
    };
    let first = bind(0, 0, 90);
    let second = bind(1, 1, 90);
    let done_first = bind(0, 2, 110);
    let done_second = bind(1, 3, 120);
    let forecast = forecast(5);
    let predicted_first = CostedForecast::bind(&first, demand, &forecast).unwrap();
    let predicted_second = CostedForecast::bind(&second, demand, &forecast).unwrap();
    let dispatched = MonotonicInstant::new(100, first.window().observed().clock().clone()).unwrap();
    assert!(matches!(
        CostCalibrationRecord::admit(
            &predicted_first,
            &done_second,
            &dispatched,
            CostExecutionCondition::Cold,
            &mut work()
        ),
        Err(CostEvidenceRefusal::Scope(CostScopeChange::Offer))
    ));
    let first_record = CostCalibrationRecord::admit(
        &predicted_first,
        &done_first,
        &dispatched,
        CostExecutionCondition::Cold,
        &mut work(),
    )
    .unwrap();
    let second_record = CostCalibrationRecord::admit(
        &predicted_second,
        &done_second,
        &dispatched,
        CostExecutionCondition::Cold,
        &mut work(),
    )
    .unwrap();
    let mut model = BoundedCostCalibration::admit(policy(2, 10)).unwrap();
    model.update(&first_record, &mut work()).unwrap();
    assert_eq!(
        model.update(&second_record, &mut work()),
        Err(CostEvidenceRefusal::Scope(CostScopeChange::Offer))
    );
    let now = MonotonicInstant::new(130, dispatched.clock().clone()).unwrap();
    let estimate = model.snapshot_at(&host, revision, &now).unwrap().unwrap();
    assert!(matches!(
        CostedForecast::bind(&second, demand, &forecast)
            .unwrap()
            .with_calibration(&estimate, CostExecutionCondition::Cold, &now),
        Err(CostEvidenceRefusal::Scope(CostScopeChange::Offer))
    ));
}
