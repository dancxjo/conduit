//! Connected ordinary Plan fixtures; supplied costs are not physical timings.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_signal_conformance::{triple, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS};
use std::collections::BTreeMap;

fn point(ms: i128) -> CostDurationRange {
    let mut work = ExactNumericWork::new(100_000_000);
    let value = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::from_i128(ms),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work,
    )
    .unwrap();
    CostDurationRange::admit(value, value, &mut work).unwrap()
}
fn connected(
    missing_local: bool,
    stale_remote: bool,
    double_count: bool,
) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
    let exact = triple::exact_plan().unwrap();
    assert!(verify_plan(&exact.plan));
    let plot = conduit_plot::parse_with_startup(
        include_str!("../../../proof/fixtures/plots/triple-signal.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap();
    let hosts = [
        exact.source_advertisement,
        exact.browser_advertisement,
        exact.pico_advertisement,
    ];
    let lines = [exact.browser_line, exact.pico_line];
    let placements: Vec<_> = exact
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .collect();
    let windows: Vec<_> = hosts
        .iter()
        .map(|host| {
            let clock = MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "connected-cost".into(),
                TemporalScale::Milliseconds,
                1,
                0,
            )
            .unwrap();
            CostObservationWindow::admit(
                MonotonicInstant::new(100, clock.clone()).unwrap(),
                MonotonicInstant::new(1000, clock).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let now: Vec<_> = windows
        .iter()
        .map(|w| MonotonicInstant::new(101, w.observed().clock().clone()).unwrap())
        .collect();
    let model = ArtifactId::from("fixture/connected-transfer@1");
    let calibrations = [CostCalibrationRevision::new(&model, 1).unwrap()];
    let contexts: Vec<_> = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| CostEvaluationContext {
            host,
            now,
            calibrations: &calibrations,
            execution_observations: &[],
        })
        .collect();
    let signs: Vec<SignId> = (0..plot.gears.len())
        .map(|i| format!("fixture/connected/{i}").into())
        .collect();
    let sources: Vec<_> = plot
        .gears
        .iter()
        .enumerate()
        .map(|(i, gear)| {
            let placement = placements
                .iter()
                .find(|p| p.gear_id == gear.gear_id)
                .unwrap();
            let h = hosts
                .iter()
                .position(|h| h.host_id == placement.host_id)
                .unwrap();
            let offer = hosts[h]
                .capabilities
                .iter()
                .find(|o| o.capability_id == placement.capability_id)
                .unwrap();
            CostEvidenceSource::bind(
                &hosts[h],
                offer,
                None,
                &signs[i],
                calibrations[0],
                windows[h].clone(),
            )
            .unwrap()
        })
        .collect();
    let fulfillment = FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(point(0)),
            queue: Some(point(0)),
            service: Some(point(1)),
            transfer: Some(point(if double_count { 1 } else { 0 })),
        },
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let payload = conduit_signal::encode_signal(&conduit_signal::Signal::new(false, 0).unwrap());
    let codec_revision = ArtifactId::from("fixture/signal-owner-codec@1");
    let codecs = [CostInputCodec::admit(
        &payload.value_kind,
        &codec_revision,
        conduit_signal::SIGNAL_ENCODED_LEN as usize,
        |encoded| {
            conduit_signal::decode_signal_bytes(encoded)
                .map(|_| ())
                .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
        },
    )
    .unwrap()];
    let request_inputs: Vec<Vec<_>> = plot
        .gears
        .iter()
        .map(|gear| {
            gear.inputs
                .iter()
                .map(|port| CostInput {
                    port: &port.port_id,
                    value: &payload,
                })
                .collect()
        })
        .collect();
    let demands: Vec<_> = plot
        .gears
        .iter()
        .enumerate()
        .map(|(i, gear)| {
            CostDemand::admit_with_catalog(gear, &request_inputs[i], &[], &codecs).unwrap()
        })
        .collect();
    let forecasts: Vec<_> = plot
        .gears
        .iter()
        .enumerate()
        .map(|(i, _gear)| [CostedForecast::bind(&sources[i], demands[i], &fulfillment).unwrap()])
        .collect();
    let requirements = HardRealizationRequirements::default();
    let tie = RealizationPolicy::default();
    let operations: Vec<_> = plot
        .gears
        .iter()
        .enumerate()
        .map(|(i, _gear)| CostPlanningOperation {
            demand: demands[i],
            forecasts: &forecasts[i],
            requirements: &requirements,
            tie_policy: &tie,
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::RequireKnown,
        })
        .collect();
    let observations = common::resource_observations(&hosts);
    let timed: Vec<_> = observations
        .iter()
        .map(|o| {
            let h = hosts.iter().position(|h| h.host_id == o.host_id).unwrap();
            TimedResourceObservation::admit(o, windows[h].clone()).unwrap()
        })
        .collect();
    let extent = CostTransferExtent {
        encoded_bytes: conduit_signal::SIGNAL_ENCODED_LEN,
        items: 1,
    };
    let demands: Vec<_> = plot
        .connections
        .iter()
        .map(|connection| CostCordTransferDemand { connection, extent })
        .collect();
    let transfer_sign = SignId::from("fixture/connected/edge");
    let connections: Vec<_> = exact
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.connections)
        .collect();
    let mut edge_forecasts: Vec<CostTransferForecast<'_>> = Vec::new();
    for connection in connections {
        // A remote Cord is present in both endpoint fragments. It is one
        // immutable route and receives one supplied observation.
        if edge_forecasts.iter().any(|f| f.connection() == connection) {
            continue;
        }
        let source = *placements
            .iter()
            .find(|p| p.placement_id == connection.source_placement_id)
            .unwrap();
        let sink = *placements
            .iter()
            .find(|p| p.placement_id == connection.sink_placement_id)
            .unwrap();
        let source_h = hosts
            .iter()
            .position(|h| h.host_id == source.host_id)
            .unwrap();
        let sink_h = hosts
            .iter()
            .position(|h| h.host_id == sink.host_id)
            .unwrap();
        if missing_local && connection.selected_line.is_none() {
            continue;
        }
        let ms = if connection.selected_line.is_none() {
            3
        } else if sink_h == 1 {
            7
        } else {
            12
        };
        let window = if stale_remote && sink_h == 1 {
            CostObservationWindow::admit(
                windows[source_h].observed().clone(),
                now[source_h].clone(),
            )
            .unwrap()
        } else {
            windows[source_h].clone()
        };
        edge_forecasts.push(
            CostTransferForecast::bind(
                connection,
                source,
                sink,
                &hosts[source_h],
                &hosts[sink_h],
                extent,
                &transfer_sign,
                calibrations[0],
                window,
                point(ms),
            )
            .unwrap(),
        );
    }
    let empty = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let bases: Vec<BaseImplementationId> = [
        "conduit.base/local@1",
        "conduit.base/websocket-rfc6455@1",
        "conduit.base/usb-cdc-acm@1",
    ]
    .map(Into::into)
    .into();
    let inputs = CostPlanningInputs {
        hosts: &hosts,
        contexts: &contexts,
        operations: &operations,
        advertisements: &[],
        observations: &timed,
        bases: &bases,
        options: PlanningOptions {
            connection_bases: &empty,
            line_candidates: &empty_lines,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: conduit_signal::SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    };
    plan_global_realizations_with_transfer_costs(
        &plot,
        inputs,
        GlobalPlacementBudget {
            maximum_states: 128,
        },
        GlobalTransferInputs {
            demands: &demands,
            forecasts: &edge_forecasts,
        },
        &mut ExactNumericWork::new(100_000_000),
    )
}
#[test]
fn connected_ordinary_plan_adds_scoped_edges_before_join() {
    let result = connected(false, false, false).unwrap();
    assert!(verify_plan(&result.plan));
    assert!(result.includes_transfer);
    assert_eq!(result.score.unknown_operations, 0);
    assert_eq!(result.score.unknown_transfers, 0);
    assert_eq!(
        result.score.critical_path_upper_seconds,
        Some(ExactRational::ratio(14, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap())
    );
}
#[test]
fn missing_local_and_expired_remote_edges_remain_unknown() {
    for (missing_local, stale_remote) in [(true, false), (false, true)] {
        let result = connected(missing_local, stale_remote, false).unwrap();
        assert!(verify_plan(&result.plan));
        assert_eq!(result.score.unknown_operations, 0);
        assert_eq!(result.score.unknown_transfers, 1);
        assert_eq!(result.score.critical_path_upper_seconds, None);
    }
}
#[test]
fn operation_transfer_is_not_counted_twice() {
    assert!(matches!(
        connected(false, false, true),
        Err(GlobalPlacementRefusal::NoFeasibleAssignment { .. })
    ));
}
