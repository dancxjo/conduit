//! A supplied-cost connected placement trap, not measured physical performance.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_signal_conformance::triple;
use std::collections::BTreeMap;

fn duration(ms: i128) -> CostDurationRange {
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
fn fulfillment(ms: i128) -> FulfillmentCostForecast {
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(duration(0)),
            queue: Some(duration(0)),
            service: Some(duration(ms)),
            transfer: Some(duration(0)),
        },
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap()
}
fn exercise(measure: bool) {
    let fixture = triple::exact_plan().unwrap();
    let plot = conduit_plot::parse_with_startup(
        "plot transfer-trap {\n producer: flow/pulse(count = 1, period-ms = 0, initial = false)\n sink: face/show(\"Transfer sink\")\n producer >> sink\n}\n",
        &conduit_signal::signal_startup_catalog(), &conduit_signal::signal_profile_catalog()).unwrap();
    let mut remote = fixture.source_advertisement.clone();
    remote.host_id = "fixture/remote-producer".into();
    remote.boot_id = "fixture/remote-boot".into();
    remote
        .capabilities
        .retain(|offer| offer.capability_id.as_str() == triple::PULSE_CAPABILITY_ID);
    let hosts = [fixture.source_advertisement, remote];
    let mut line = fixture.browser_line;
    line.line_id = "fixture/remote-producer-line".into();
    line.binding.binding_id = "fixture/remote-producer-binding".into();
    line.binding.base_instance_id = "fixture/remote-producer-websocket".into();
    line.binding.source.host_id = hosts[1].host_id.clone();
    line.binding.source.boot_id = hosts[1].boot_id.clone();
    line.binding.source.endpoint_id = "fixture/remote-egress".into();
    line.binding.sink.host_id = hosts[0].host_id.clone();
    line.binding.sink.boot_id = hosts[0].boot_id.clone();
    line.binding.sink.endpoint_id = "fixture/local-ingress".into();
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    assert!(line.validate_sign_identity());
    let lines = [line];
    let bases: Vec<BaseImplementationId> =
        ["conduit.base/local@1", "conduit.base/websocket-rfc6455@1"]
            .map(Into::into)
            .into();
    let empty = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: conduit_signal::SIGNAL_ENCODED_LEN,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &lines,
    };
    let prepared: Vec<_> = hosts
        .iter()
        .map(|host| {
            plan_with_options(
                &plot,
                &hosts,
                &PlacementChoices {
                    by_gear: BTreeMap::from([
                        (
                            plot.gears[0].gear_id.clone(),
                            PlacementChoice {
                                host_id: host.host_id.clone(),
                                capability_id: triple::PULSE_CAPABILITY_ID.into(),
                            },
                        ),
                        (
                            plot.gears[1].gear_id.clone(),
                            PlacementChoice {
                                host_id: hosts[0].host_id.clone(),
                                capability_id: triple::STDOUT_CAPABILITY_ID.into(),
                            },
                        ),
                    ]),
                },
                &bases,
                options,
            )
            .unwrap()
        })
        .collect();
    assert!(prepared.iter().all(verify_plan));
    let windows: Vec<_> = hosts
        .iter()
        .map(|host| {
            let clock = MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "transfer-trap-clock".into(),
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
    let model = ArtifactId::from("fixture/transfer-trap-model@1");
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
    let sign = SignId::from("fixture/transfer-trap-observation");
    let producer_sources: Vec<_> = hosts
        .iter()
        .enumerate()
        .map(|(i, host)| {
            let offer = host
                .capabilities
                .iter()
                .find(|o| o.capability_id.as_str() == triple::PULSE_CAPABILITY_ID)
                .unwrap();
            CostEvidenceSource::bind(
                host,
                offer,
                None,
                &sign,
                calibrations[0],
                windows[i].clone(),
            )
            .unwrap()
        })
        .collect();
    let sink_offer = hosts[0]
        .capabilities
        .iter()
        .find(|o| o.capability_id.as_str() == triple::STDOUT_CAPABILITY_ID)
        .unwrap();
    let sink_source = CostEvidenceSource::bind(
        &hosts[0],
        sink_offer,
        None,
        &sign,
        calibrations[0],
        windows[0].clone(),
    )
    .unwrap();
    let payload = conduit_signal::encode_signal(&conduit_signal::Signal::new(false, 0).unwrap());
    let codec_revision = ArtifactId::from("fixture/signal-codec@1");
    let codecs = [CostInputCodec::admit(
        &payload.value_kind,
        &codec_revision,
        conduit_signal::SIGNAL_ENCODED_LEN as usize,
        |bytes| {
            conduit_signal::decode_signal_bytes(bytes)
                .map(|_| ())
                .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
        },
    )
    .unwrap()];
    let sink_inputs = [CostInput {
        port: &plot.gears[1].inputs[0].port_id,
        value: &payload,
    }];
    let demands = [
        CostDemand::admit(&plot.gears[0], &[]).unwrap(),
        CostDemand::admit_with_catalog(&plot.gears[1], &sink_inputs, &[], &codecs).unwrap(),
    ];
    let models = [fulfillment(10), fulfillment(2), fulfillment(1)];
    let producer_forecasts = [
        CostedForecast::bind(&producer_sources[0], demands[0], &models[0]).unwrap(),
        CostedForecast::bind(&producer_sources[1], demands[0], &models[1]).unwrap(),
    ];
    let sink_forecasts = [CostedForecast::bind(&sink_source, demands[1], &models[2]).unwrap()];
    let requirements = HardRealizationRequirements::default();
    let tie = RealizationPolicy::default();
    let operations = [
        CostPlanningOperation {
            demand: demands[0],
            forecasts: &producer_forecasts,
            requirements: &requirements,
            tie_policy: &tie,
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::RequireKnown,
        },
        CostPlanningOperation {
            demand: demands[1],
            forecasts: &sink_forecasts,
            requirements: &requirements,
            tie_policy: &tie,
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::RequireKnown,
        },
    ];
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
    let mut edge_forecasts = Vec::new();
    for (i, plan) in prepared.iter().enumerate() {
        let connection = plan
            .fragments
            .iter()
            .flat_map(|f| &f.connections)
            .next()
            .unwrap();
        let source = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| p.placement_id == connection.source_placement_id)
            .unwrap();
        let sink = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| p.placement_id == connection.sink_placement_id)
            .unwrap();
        edge_forecasts.push(
            CostTransferForecast::bind(
                connection,
                source,
                sink,
                &hosts[i],
                &hosts[0],
                extent,
                &sign,
                calibrations[0],
                windows[i].clone(),
                duration(if i == 0 { 1 } else { 100 }),
            )
            .unwrap(),
        );
    }
    let transfer_demands = [CostCordTransferDemand {
        connection: &plot.connections[0],
        extent,
    }];
    let inputs = CostPlanningInputs {
        hosts: &hosts,
        contexts: &contexts,
        operations: &operations,
        advertisements: &[],
        observations: &timed,
        bases: &bases,
        options,
    };
    let greedy = plan_selected_realizations_with_costs(
        &plot,
        inputs,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let producer = |plan: &Plan| {
        plan.fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| p.gear_id == plot.gears[0].gear_id)
            .unwrap()
            .host_id
            .clone()
    };
    assert!(verify_plan(&greedy.plan));
    assert_eq!(producer(&greedy.plan), hosts[1].host_id);
    let global = plan_global_realizations_with_transfer_costs(
        &plot,
        inputs,
        GlobalPlacementBudget {
            maximum_states: 128,
        },
        GlobalTransferInputs {
            demands: &transfer_demands,
            forecasts: &edge_forecasts,
        },
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    assert!(verify_plan(&global.plan));
    assert_eq!(global.plan.checked_plot_id, greedy.plan.checked_plot_id);
    assert_eq!(producer(&global.plan), hosts[0].host_id);
    assert_eq!(
        global.score.unknown_operations + global.score.unknown_transfers,
        0
    );
    assert_eq!(
        global.score.critical_path_upper_seconds,
        Some(ExactRational::ratio(12, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap())
    );
    // The ordinary feasible greedy arrangement has a supplied 2+100+1 ms model;
    // the joint result has 10+1+1 ms. These are explicit fixture estimates.
    assert_eq!(global.disposition, GlobalPlacementDisposition::Exhaustive);
    if measure {
        // This measures complete preparation and retirement of result Plans,
        // with fixed admitted fixture facts. It does not time model inference.
        for row in 0..8 {
            let order = if row % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            };
            for joint in order {
                let mut numeric_work = 0u64;
                let mut states = 0u64;
                let start = std::time::Instant::now();
                for _ in 0..256 {
                    let mut work = ExactNumericWork::new(100_000_000);
                    if joint {
                        let result = plan_global_realizations_with_transfer_costs(
                            &plot,
                            inputs,
                            GlobalPlacementBudget {
                                maximum_states: 128,
                            },
                            GlobalTransferInputs {
                                demands: &transfer_demands,
                                forecasts: &edge_forecasts,
                            },
                            &mut work,
                        )
                        .unwrap();
                        states += u64::from(result.states);
                        drop(std::hint::black_box(result));
                    } else {
                        drop(std::hint::black_box(
                            plan_selected_realizations_with_costs(&plot, inputs, &mut work)
                                .unwrap(),
                        ));
                    }
                    numeric_work += work.spent();
                }
                let elapsed = start.elapsed().as_nanos();
                println!("PLACEMENT_PROCESSING row={row} mode={} decisions=256 elapsed_ns={elapsed} numeric_work={numeric_work} states={}",
                    if joint { "joint_transfer" } else { "greedy" },
                    if joint { states.to_string() } else { "not_applicable".to_string() });
            }
        }
    }
}

#[test]
fn slower_local_producer_beats_faster_remote_producer_after_transfer() {
    exercise(false);
}

#[test]
#[ignore = "manual finite planner processing measurement; not inference or physical acceptance"]
fn measure_connected_placement_processing() {
    exercise(true);
}
