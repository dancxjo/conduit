//! The canonical Little Seismograph processing Form through one planned browser kernel.

use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, CapabilityLimits, CapabilityOffer,
    ImplementationOffer, LinkLimits, PortDescriptor, PortDirection, PortTemporal, Quantity,
    QuantityUnit, StructuredInfoType, StructuredInfoValue, TemporalInstant, TemporalScale,
};
use conduit_data::{
    FullWindowPolicy, MeasurementHysteresisProfile, MeasurementRange, MeasurementSample,
    MeasurementThresholdPolicy, MeasurementThresholdState, MeasurementThresholdTransition,
    MeasurementWindowProfile,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

const SOURCE_KIND: &str = "fixture/little-seismograph-inputs";

fn output(port: &str, value_type: StructuredInfoType, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(port),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction: PortDirection::Output,
        temporal,
    }
}

fn source_offer() -> CapabilityOffer {
    CapabilityOffer {
        kind_id: SOURCE_KIND.into(),
        kind_contract_revision: "fixture/little-seismograph-inputs@1".into(),
        capability_id: SOURCE_KIND.into(),
        startup_parameters: Vec::new(),
        shorthand: None,
        inputs: Vec::new(),
        outputs: vec![
            output(
                "profile",
                conduit_data::measurement_window_profile_type(),
                PortTemporal::Value,
            ),
            output(
                "measurement",
                conduit_data::measurement_sample_type(),
                PortTemporal::Flow { closes: true },
            ),
            output(
                "threshold-profile",
                conduit_data::measurement_hysteresis_profile_type(),
                PortTemporal::Value,
            ),
        ],
        implementation: ImplementationOffer {
            execution_profile_id: SOURCE_KIND.into(),
            implementation_id: SOURCE_KIND.into(),
            artifact_id: SOURCE_KIND.into(),
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_BROWSER_VALUE_BYTES * 4) as u32,
        },
    }
}

fn fragment() -> PlanFragment {
    let (mut startup, mut catalog) = crate::installed_browser::catalogs().unwrap();
    let mut browser = crate::installed_browser::advertisement(
        "little-seismograph-browser".into(),
        "little-seismograph-boot".into(),
    );
    for sink in [
        crate::installed_browser::test_measurement_decision_sink::offer(),
        crate::installed_browser::test_measurement_sink::offer(),
    ] {
        startup
            .insert(KindSignature {
                kind: sink.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        catalog
            .insert(KindDefinition {
                kind_id: sink.kind_id.clone(),
                kind_contract_revision: sink.kind_contract_revision.clone(),
                inputs: sink.inputs.clone(),
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .unwrap();
        browser.capabilities.push(sink);
    }

    let source = source_offer();
    startup
        .insert(KindSignature {
            kind: SOURCE_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    catalog
        .insert(KindDefinition {
            kind_id: source.kind_id.clone(),
            kind_contract_revision: source.kind_contract_revision.clone(),
            inputs: Vec::new(),
            outputs: source.outputs.clone(),
            configuration: Vec::new(),
        })
        .unwrap();
    let mut source_host = browser.clone();
    source_host.host_id = "fixture/little-seismograph-source".into();
    source_host.boot_id = "fixture/little-seismograph-source-boot".into();
    source_host.capabilities = vec![source];

    let canonical = include_str!("../../../../../forms/little-seismograph/main.conduit");
    let syntax = parse_syntax_document(&format!(
        "{canonical}\nform browser-proof {{\n source: {SOURCE_KIND}\n processing: little-seismograph-processing\n decision: conduit-test/measurement-decision-sink\n plot: conduit-test/measurement-plot-sink\n source.profile > processing.profile\n source.measurement > processing.measurement\n source.threshold-profile > processing.threshold-profile\n processing.decision > decision.decision\n processing.series > plot.series\n}}\n"
    ));
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "browser-proof", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == SOURCE_KIND {
                    &source_host
                } else {
                    &browser
                };
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: host
                            .capabilities
                            .iter()
                            .find(|offer| offer.kind_id == gear.kind_id)
                            .unwrap()
                            .capability_id
                            .clone(),
                    },
                )
            })
            .collect(),
    };
    let maximum = MAXIMUM_BROWSER_VALUE_BYTES as u32;
    let lines = [
        (
            "fixture/little-seismograph-profile-line",
            "fixture/little-seismograph-profile-binding",
            "fixture/little-seismograph-profile-base",
        ),
        (
            "fixture/little-seismograph-measurement-line",
            "fixture/little-seismograph-measurement-binding",
            "fixture/little-seismograph-measurement-base",
        ),
        (
            "fixture/little-seismograph-threshold-profile-line",
            "fixture/little-seismograph-threshold-profile-binding",
            "fixture/little-seismograph-threshold-profile-base",
        ),
    ]
    .map(|(line, binding, base)| {
        process_owned_line_offer_with_limits(
            line,
            binding,
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            base,
            &source_host,
            &browser,
            LinkLimits {
                maximum_in_flight_items: 4,
                maximum_payload_bytes: maximum,
                maximum_buffered_bytes: maximum * 4,
                maximum_frame_bytes: maximum * 2,
            },
        )
    });
    let candidates = expanded
        .connections
        .iter()
        .filter(|cord| cord.source_gear_id.as_str() == "browser-proof/source")
        .zip(&lines)
        .map(|(cord, line)| {
            (
                (cord.source_gear_id.clone(), cord.sink_gear_id.clone()),
                vec![line.line_id.clone()],
            )
        })
        .collect::<BTreeMap<_, _>>();
    conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: maximum,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .unwrap()
    .fragments
    .into_iter()
    .find(|fragment| fragment.host_id == browser.host_id)
    .unwrap()
}

fn leaf(value_type: StructuredInfoType, payload: Vec<u8>) -> Vec<u8> {
    StructuredInfoValue::leaf(value_type, payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn sample(value: i64, ticks: u64) -> Vec<u8> {
    leaf(
        conduit_data::measurement_sample_type(),
        conduit_data::encode_measurement_sample(&MeasurementSample {
            value: Quantity::new(value, QuantityUnit::Millivolt),
            observed_at: TemporalInstant {
                ticks,
                scale: TemporalScale::Milliseconds,
                clock_basis: "fixture-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            uncertainty: None,
        })
        .unwrap(),
    )
}

fn manifestation(
    pending: &PendingHostEffect,
) -> (
    Option<conduit_data::MeasurementThresholdDecision>,
    Option<conduit_data::MeasurementPlotSeries>,
) {
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected typed measurement manifestation")
    };
    let value = StructuredInfoValue::from_canonical_bytes(&output.canonical_value).unwrap();
    let conduit_core::StructuredInfoValueShape::Leaf(payload) = value.shape() else {
        panic!("expected typed leaf")
    };
    match output.kind_id {
        crate::installed_browser::test_measurement_decision_sink::KIND => (
            Some(conduit_data::decode_measurement_threshold_decision(payload).unwrap()),
            None,
        ),
        crate::installed_browser::test_measurement_sink::KIND => (
            None,
            Some(conduit_data::decode_measurement_plot_series(payload).unwrap()),
        ),
        other => panic!("unexpected manifestation {other}"),
    }
}

#[test]
fn canonical_processing_runs_window_summary_hysteresis_and_plot_in_one_play() {
    let fragment = fragment();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert_eq!(lowered.remote_endpoints.len(), 3);
    let mut scheduler = prepare_scheduler(&fragment, &lowered).unwrap();

    let endpoint = |value_type: StructuredInfoType| {
        let kind = value_type.profile().unwrap().value_kind().clone();
        lowered
            .remote_endpoints
            .iter()
            .find(|remote| remote.value_kind == kind)
            .unwrap()
    };
    let window_profile = endpoint(conduit_data::measurement_window_profile_type());
    let threshold_profile = endpoint(conduit_data::measurement_hysteresis_profile_type());
    let measurements = endpoint(conduit_data::measurement_sample_type());

    let profile = leaf(
        conduit_data::measurement_window_profile_type(),
        conduit_data::encode_measurement_window_profile(&MeasurementWindowProfile {
            capacity: 2,
            unit: QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(0, QuantityUnit::Millivolt),
                maximum: Quantity::new(100, QuantityUnit::Millivolt),
            },
            clock_basis: "fixture-clock".into(),
            full_policy: FullWindowPolicy::DropOldest,
        })
        .unwrap(),
    );
    scheduler
        .admit_remote_input(window_profile.endpoint, window_profile.cord, 0, &profile)
        .unwrap();
    scheduler
        .close_remote_input(window_profile.endpoint, window_profile.cord)
        .unwrap();
    let profile = leaf(
        conduit_data::measurement_hysteresis_profile_type(),
        conduit_data::encode_measurement_hysteresis_profile(MeasurementHysteresisProfile {
            policy: MeasurementThresholdPolicy {
                lower: Quantity::new(40, QuantityUnit::Millivolt),
                upper: Quantity::new(60, QuantityUnit::Millivolt),
            },
            initial_state: MeasurementThresholdState::Below,
        })
        .unwrap(),
    );
    scheduler
        .admit_remote_input(
            threshold_profile.endpoint,
            threshold_profile.cord,
            0,
            &profile,
        )
        .unwrap();
    scheduler
        .close_remote_input(threshold_profile.endpoint, threshold_profile.cord)
        .unwrap();

    let mut final_decision = None;
    let mut final_series = None;
    for (sequence, value) in [0, 100, 100].into_iter().enumerate() {
        let value = sample(value, sequence as u64 + 1);
        scheduler
            .admit_remote_input(
                measurements.endpoint,
                measurements.cord,
                sequence as u64,
                &value,
            )
            .unwrap();
        loop {
            match drive(&mut scheduler, &fragment) {
                Ok(DriveStatus::Effect(pending)) => {
                    let (decision, series) = manifestation(&pending);
                    if decision.is_some() {
                        final_decision = decision;
                    }
                    if series.is_some() {
                        final_series = series;
                    }
                    complete_host_effect(&mut scheduler, &pending).unwrap();
                }
                Err(error) => {
                    assert_eq!(error, "Tour Play became idle");
                    break;
                }
                Ok(DriveStatus::Quiescent) => panic!("measurement flow quiesced before its source"),
                Ok(DriveStatus::SemanticCompleted) => {
                    panic!("measurement flow completed before its source")
                }
                Ok(DriveStatus::Waiting { .. }) => panic!("no Host effect was left incomplete"),
            }
        }
    }
    scheduler
        .close_remote_input(measurements.endpoint, measurements.cord)
        .unwrap();
    loop {
        match drive(&mut scheduler, &fragment).unwrap() {
            DriveStatus::Effect(pending) => {
                let (decision, series) = manifestation(&pending);
                if decision.is_some() {
                    final_decision = decision;
                }
                if series.is_some() {
                    final_series = series;
                }
                complete_host_effect(&mut scheduler, &pending).unwrap();
            }
            DriveStatus::Quiescent => break,
            DriveStatus::SemanticCompleted => panic!("living measurement flow completed"),
            DriveStatus::Waiting { .. } => panic!("no Host effect was left incomplete"),
        }
    }

    let decision = final_decision.unwrap();
    assert_eq!(decision.state, MeasurementThresholdState::Above);
    assert_eq!(
        decision.transition,
        Some(MeasurementThresholdTransition::RoseAbove)
    );
    let series = final_series.unwrap();
    assert_eq!((series.source_samples(), series.omitted_samples()), (2, 0));
    assert_eq!(
        series
            .points()
            .iter()
            .map(|point| point.value_millionths)
            .collect::<Vec<_>>(),
        [1_000_000, 1_000_000]
    );
}
