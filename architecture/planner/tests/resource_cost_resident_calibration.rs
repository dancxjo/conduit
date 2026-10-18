//! Residency observations cannot borrow another execution stratum's history.
mod common;
use conduit_core::*;
use conduit_planner::resource_cost::*;

#[test]
fn observed_residency_calibration_is_separate_from_unknown_cold_and_epoch_bound_warm() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let gear = common::pulse_gear();
    let profile = ArtifactId::from("resident-calibration@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "resident-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let instant = |tick| MonotonicInstant::new(tick, clock.clone()).unwrap();
    let window = |tick| CostObservationWindow::admit(instant(tick), instant(1000)).unwrap();
    let signs = [
        SignId::from("forecast"),
        SignId::from("resident-done"),
        SignId::from("other-done"),
    ];
    let sources = signs
        .iter()
        .zip([90, 120, 140])
        .map(|(sign, tick)| {
            CostEvidenceSource::bind(
                &host,
                &host.capabilities[0],
                None,
                sign,
                revision,
                window(tick),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut work = ExactNumericWork::new(1_000_000_000);
    let duration = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::from_i128(20),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work,
    )
    .unwrap();
    let range = CostDurationRange::admit(duration, duration, &mut work).unwrap();
    let forecast = FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(range),
            queue: Some(range),
            service: Some(range),
            transfer: Some(range),
        },
        &mut work,
    )
    .unwrap();
    let demand = CostDemand::admit(&gear, &[]).unwrap();
    let predicted = CostedForecast::bind(&sources[0], demand, &forecast).unwrap();
    let dispatched = instant(100);
    let resident = CostCalibrationRecord::admit(
        &predicted,
        &sources[1],
        &dispatched,
        CostExecutionCondition::ResidentObserved,
        &mut work,
    )
    .unwrap();
    let conditions = [
        CostExecutionCondition::Unknown,
        CostExecutionCondition::Cold,
        CostExecutionCondition::Warm {
            residency_revision: 1,
        },
    ];
    let records = conditions
        .iter()
        .map(|condition| {
            CostCalibrationRecord::admit(
                &predicted,
                &sources[2],
                &dispatched,
                *condition,
                &mut work,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for record in &records {
        let policy = CostCalibrationPolicy {
            history_capacity: 2,
            maximum_updates: 4,
            minimum_update_ticks: 1,
        };
        let mut model = BoundedCostCalibration::admit(policy).unwrap();
        model.update(&resident, &mut work).unwrap();
        let before = model
            .snapshot_at(&host, revision, &instant(150))
            .unwrap()
            .unwrap();
        assert_eq!(before.condition(), CostExecutionCondition::ResidentObserved);
        assert_eq!(
            before.validate_use(
                &sources[0],
                demand,
                CostExecutionCondition::ResidentObserved,
                &instant(150)
            ),
            Ok(())
        );
        assert_eq!(
            before.validate_use(&sources[0], demand, record.condition(), &instant(150)),
            Err(CostEvidenceRefusal::ExecutionConditionChanged)
        );
        assert_eq!(
            model.update(record, &mut work),
            Err(CostEvidenceRefusal::ExecutionConditionChanged)
        );
        assert_eq!(model.updates(), 1);
        assert_eq!(model.history().count(), 1);
        let after = model
            .snapshot_at(&host, revision, &instant(150))
            .unwrap()
            .unwrap();
        assert_eq!(after.duration(), before.duration());
        assert_eq!(after.source().sign(), resident.completion().sign());
    }
}
