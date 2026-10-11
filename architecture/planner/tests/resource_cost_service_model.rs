//! Exact service models derive finite work from real checked meaning.
mod common;
use conduit_core::*;
use conduit_planner::resource_cost::*;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn pulse_events(demand: CostDemand<'_>) -> Result<u64, CostEvidenceRefusal> {
    if demand.class().gear().kind_id.as_str() != conduit_signal::PULSE_KIND {
        return Err(CostEvidenceRefusal::InvalidWorkInput);
    }
    demand
        .class()
        .gear()
        .configuration
        .iter()
        .find_map(|entry| {
            if entry.key == "count" {
                if let ConfigurationValue::U64(value) = entry.value {
                    return Some(value);
                }
            }
            None
        })
        .ok_or(CostEvidenceRefusal::InvalidWorkInput)
}
fn point(numerator: i128, denominator: i128) -> CostDurationRange {
    let mut work = work();
    let value = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ratio(numerator, denominator, &mut work).unwrap(),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work,
    )
    .unwrap();
    CostDurationRange::admit(value, value, &mut work).unwrap()
}
fn window(host: &HostAdvertisement) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "service-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(200, clock).unwrap(),
    )
    .unwrap()
}
#[test]
fn source_bound_service_estimate_preserves_fractional_time_and_work_definition() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let gear = common::pulse_gear();
    let profile = ArtifactId::from("fixture/pulse-profile@1");
    let metric_id = ArtifactId::from("fixture/pulse-event-count@1");
    let metric = CostWorkMetric::admit(&metric_id, 1, 100, pulse_events).unwrap();
    let source_sign = SignId::from("forecast/pulse");
    let source = CostEvidenceSource::bind(
        &host,
        &host.capabilities[0],
        None,
        &source_sign,
        CostCalibrationRevision::new(&profile, 1).unwrap(),
        window(&host),
    )
    .unwrap();
    let demand = CostDemand::admit(&gear, &[])
        .unwrap()
        .with_work_metric(&metric)
        .unwrap();
    assert_eq!(demand.work_estimate().unwrap().units(), 2);
    assert_eq!(
        demand.work_estimate().unwrap().metric_identity(),
        &metric_id
    );
    let model = CostServiceModel::admit(
        &source,
        demand.class(),
        &metric,
        point(1, 3),
        CostNonServiceStages {
            setup: Some(point(1, 1)),
            queue: Some(point(0, 1)),
            transfer: Some(point(0, 1)),
        },
    )
    .unwrap();
    let predicted = model.predict(demand, &mut work()).unwrap();
    let sample = predicted.bind().unwrap();
    let service = sample
        .forecast()
        .stages()
        .service
        .unwrap()
        .lower()
        .original();
    assert_eq!(service.unit(), Unit::resolve("ms").unwrap());
    assert_eq!(
        service.coordinate(),
        ExactRational::ratio(2, 3, &mut work()).unwrap()
    );
    assert_eq!(
        sample.upper_seconds(),
        ExactRational::ratio(1, 600, &mut work()).unwrap()
    );
    assert_eq!(sample.source().sign(), &source_sign);
    assert_eq!(sample.service_basis().unwrap(), point(1, 3));
}
#[test]
fn unknown_work_and_revised_metrics_cannot_reuse_models_or_calibration_demand() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let gear = common::pulse_gear();
    let profile = ArtifactId::from("fixture/pulse-profile@1");
    let metric_id = ArtifactId::from("fixture/pulse-event-count@1");
    let metric = CostWorkMetric::admit(&metric_id, 1, 100, pulse_events).unwrap();
    let next = CostWorkMetric::admit(&metric_id, 2, 100, pulse_events).unwrap();
    let limited = CostWorkMetric::admit(&metric_id, 1, 1, pulse_events).unwrap();
    let sign = SignId::from("forecast/pulse");
    let source = CostEvidenceSource::bind(
        &host,
        &host.capabilities[0],
        None,
        &sign,
        CostCalibrationRevision::new(&profile, 1).unwrap(),
        window(&host),
    )
    .unwrap();
    let raw = CostDemand::admit(&gear, &[]).unwrap();
    let measured = raw.with_work_metric(&metric).unwrap();
    let revised = raw.with_work_metric(&next).unwrap();
    assert_eq!(
        measured.validate_match(revised),
        Err(CostEvidenceRefusal::WorkMetricChanged)
    );
    assert!(matches!(
        raw.with_work_metric(&limited),
        Err(CostEvidenceRefusal::WorkLimit)
    ));
    assert!(matches!(
        measured.with_work_metric(&metric),
        Err(CostEvidenceRefusal::AmbiguousEvidence)
    ));
    let model = CostServiceModel::admit(
        &source,
        raw.class(),
        &metric,
        point(1, 1),
        CostNonServiceStages {
            setup: Some(point(0, 1)),
            queue: Some(point(0, 1)),
            transfer: Some(point(0, 1)),
        },
    )
    .unwrap();
    let mut empty = ExactNumericWork::new(0);
    assert!(matches!(
        model.predict(raw, &mut empty),
        Err(CostEvidenceRefusal::MissingWorkEstimate)
    ));
    assert!(matches!(
        model.predict(revised, &mut empty),
        Err(CostEvidenceRefusal::WorkMetricChanged)
    ));
    assert_eq!(empty.spent(), 0);
    assert!(matches!(
        model.predict(measured, &mut empty),
        Err(CostEvidenceRefusal::Numeric(_))
    ));
    assert!(model.predict(measured, &mut work()).is_ok());
    assert!(matches!(
        CostServiceModel::admit(
            &source,
            raw.class(),
            &metric,
            point(1, 1),
            CostNonServiceStages {
                setup: None,
                queue: Some(point(0, 1)),
                transfer: Some(point(0, 1))
            }
        ),
        Err(CostEvidenceRefusal::MissingForecastStage)
    ));
}

#[test]
fn metric_revision_change_refuses_calibration_updates_and_snapshot_reuse_atomically() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let gear = common::pulse_gear();
    let profile = ArtifactId::from("fixture/pulse-profile@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let metric_id = ArtifactId::from("fixture/pulse-event-count@1");
    let metric = CostWorkMetric::admit(&metric_id, 1, 100, pulse_events).unwrap();
    let next = CostWorkMetric::admit(&metric_id, 2, 100, pulse_events).unwrap();
    let raw = CostDemand::admit(&gear, &[]).unwrap();
    let measured = raw.with_work_metric(&metric).unwrap();
    let revised = raw.with_work_metric(&next).unwrap();
    let forecast_sign = SignId::from("forecast/pulse");
    let source = CostEvidenceSource::bind(
        &host,
        &host.capabilities[0],
        None,
        &forecast_sign,
        revision,
        window(&host),
    )
    .unwrap();
    let stages = CostNonServiceStages {
        setup: Some(point(0, 1)),
        queue: Some(point(0, 1)),
        transfer: Some(point(0, 1)),
    };
    let first = CostServiceModel::admit(&source, raw.class(), &metric, point(1, 1), stages)
        .unwrap()
        .predict(measured, &mut work())
        .unwrap();
    let second = CostServiceModel::admit(&source, raw.class(), &next, point(1, 1), stages)
        .unwrap()
        .predict(revised, &mut work())
        .unwrap();
    let forecasts = [first.bind().unwrap(), second.bind().unwrap()];
    let completion_signs: [SignId; 2] =
        ["completion/pulse-1", "completion/pulse-2"].map(Into::into);
    let clock = source.window().observed().clock().clone();
    let completions = completion_signs
        .iter()
        .zip([111, 121])
        .map(|(sign, ticks)| {
            CostEvidenceSource::bind(
                &host,
                &host.capabilities[0],
                None,
                sign,
                revision,
                CostObservationWindow::admit(
                    MonotonicInstant::new(ticks, clock.clone()).unwrap(),
                    MonotonicInstant::new(200, clock.clone()).unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let dispatched = MonotonicInstant::new(110, clock.clone()).unwrap();
    let records = forecasts
        .iter()
        .zip(&completions)
        .map(|(forecast, completion)| {
            CostCalibrationRecord::admit(
                forecast,
                completion,
                &dispatched,
                CostExecutionCondition::Cold,
                &mut work(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut model = BoundedCostCalibration::admit(CostCalibrationPolicy {
        history_capacity: 2,
        maximum_updates: 4,
        minimum_update_ticks: 1,
    })
    .unwrap();
    model.update(&records[0], &mut work()).unwrap();
    assert_eq!(
        model.update(&records[1], &mut work()),
        Err(CostEvidenceRefusal::WorkMetricChanged)
    );
    assert_eq!(model.updates(), 1);
    assert_eq!(model.history().count(), 1);
    let now = MonotonicInstant::new(150, clock).unwrap();
    let estimate = model.snapshot_at(&host, revision, &now).unwrap().unwrap();
    assert!(matches!(
        second
            .bind()
            .unwrap()
            .with_calibration(&estimate, CostExecutionCondition::Cold, &now),
        Err(CostEvidenceRefusal::WorkMetricChanged)
    ));
}
