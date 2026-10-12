//! Binding tests use the ordinary verified three-Host deterministic fixture.
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_signal_conformance::triple;

#[test]
fn exact_route_extent_boot_and_freshness_are_independent_cost_gates() {
    let exact = triple::exact_plan().unwrap();
    assert!(verify_plan(&exact.plan));
    let connection = exact
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.connections)
        .find(|c| {
            c.selected_line
                .as_ref()
                .is_some_and(|l| l.line_id.as_str() == triple::BROWSER_LINE_ID)
        })
        .unwrap();
    let placements: Vec<_> = exact
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .collect();
    let source = *placements
        .iter()
        .find(|p| p.placement_id == connection.source_placement_id)
        .unwrap();
    let sink = *placements
        .iter()
        .find(|p| p.placement_id == connection.sink_placement_id)
        .unwrap();
    let clock = MonotonicClockIdentity::new(
        exact.source_advertisement.host_id.clone(),
        exact.source_advertisement.boot_id.clone(),
        "transfer-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let window = CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(200, clock.clone()).unwrap(),
    )
    .unwrap();
    let now = MonotonicInstant::new(150, clock.clone()).unwrap();
    let expired = MonotonicInstant::new(200, clock).unwrap();
    let artifact = ArtifactId::from("fixture/transfer-model@1");
    let calibration = CostCalibrationRevision::new(&artifact, 1).unwrap();
    let calibrations = [calibration];
    let context = CostEvaluationContext {
        host: &exact.source_advertisement,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    let mut work = ExactNumericWork::new(100_000_000);
    let duration = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::from_i128(7),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work,
    )
    .unwrap();
    let range = CostDurationRange::admit(duration, duration, &mut work).unwrap();
    let extent = CostTransferExtent {
        encoded_bytes: connection.byte_capacity,
        items: 1,
    };
    let sign = SignId::from("fixture/transfer-observation");
    let forecast = CostTransferForecast::bind(
        connection,
        source,
        sink,
        &exact.source_advertisement,
        &exact.browser_advertisement,
        extent,
        &sign,
        calibration,
        window.clone(),
        range,
    )
    .unwrap();
    assert_eq!(
        forecast.validate_at(connection, extent, &context, &exact.browser_advertisement),
        Ok(range)
    );

    let mut changed_route = connection.clone();
    changed_route
        .selected_line
        .as_mut()
        .unwrap()
        .binding
        .base_instance_id = "different-instance".into();
    assert_eq!(
        forecast.validate_at(
            &changed_route,
            extent,
            &context,
            &exact.browser_advertisement
        ),
        Err(CostTransferRefusal::ConnectionChanged)
    );
    let changed_extent = CostTransferExtent {
        encoded_bytes: extent.encoded_bytes - 1,
        ..extent
    };
    assert_eq!(
        forecast.validate_at(
            connection,
            changed_extent,
            &context,
            &exact.browser_advertisement
        ),
        Err(CostTransferRefusal::ExtentChanged)
    );
    let mut restarted = exact.browser_advertisement.clone();
    restarted.boot_id = "new-boot".into();
    assert_eq!(
        forecast.validate_at(connection, extent, &context, &restarted),
        Err(CostTransferRefusal::ProducerChanged)
    );
    let expired_context = CostEvaluationContext {
        now: &expired,
        ..context
    };
    assert_eq!(
        forecast.validate_at(
            connection,
            extent,
            &expired_context,
            &exact.browser_advertisement
        ),
        Err(CostTransferRefusal::Evidence(CostEvidenceRefusal::Stale))
    );
    let no_calibration = CostEvaluationContext {
        calibrations: &[],
        ..expired_context
    };
    assert_eq!(
        forecast.validate_at(
            connection,
            extent,
            &no_calibration,
            &exact.browser_advertisement
        ),
        Err(CostTransferRefusal::MissingCalibration)
    );
    let too_large = CostTransferExtent {
        encoded_bytes: extent.encoded_bytes + 1,
        ..extent
    };
    assert!(matches!(
        CostTransferForecast::bind(
            connection,
            source,
            sink,
            &exact.source_advertisement,
            &exact.browser_advertisement,
            too_large,
            &sign,
            calibration,
            window,
            range
        ),
        Err(CostTransferRefusal::ExtentLimit)
    ));
}
