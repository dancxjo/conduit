//! Exact profiled measurement samples through the planned browser production kernel.

use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, LinkLimits, PortDirection,
    Quantity, QuantityUnit, StructuredInfoValue, TemporalInstant, TemporalScale,
};
use conduit_data::{
    FullWindowPolicy, MeasurementRange, MeasurementSample, MeasurementWindowProfile,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

fn fragment() -> PlanFragment {
    let (mut startup, mut catalog) = crate::installed_browser::catalogs().unwrap();
    let mut browser =
        crate::installed_browser::advertisement("window-browser".into(), "window-boot".into());
    let sink = crate::installed_browser::test_measurement_sink::offer();
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

    let window = browser
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == "data/measurement-count-window")
        .unwrap();
    let mut source_offer = window.clone();
    source_offer.kind_id = "fixture/measurement-inputs".into();
    source_offer.kind_contract_revision = "fixture/measurement-inputs@1".into();
    source_offer.capability_id = "fixture/measurement-inputs".into();
    source_offer.outputs = source_offer.inputs.clone();
    for output in &mut source_offer.outputs {
        output.direction = PortDirection::Output;
    }
    source_offer.inputs.clear();
    source_offer.host_operations.clear();
    source_offer.startup_parameters.clear();
    source_offer.implementation.implementation_id = "fixture/measurement-inputs@1".into();
    source_offer.implementation.artifact_id = "fixture/measurement-inputs@1".into();
    startup
        .insert(KindSignature {
            kind: "fixture/measurement-inputs".into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    catalog
        .insert(KindDefinition {
            kind_id: source_offer.kind_id.clone(),
            kind_contract_revision: source_offer.kind_contract_revision.clone(),
            inputs: Vec::new(),
            outputs: source_offer.outputs.clone(),
            configuration: Vec::new(),
        })
        .unwrap();
    let mut source_host = browser.clone();
    source_host.host_id = "fixture/measurement-source".into();
    source_host.boot_id = "fixture/measurement-source-boot".into();
    source_host.capabilities = vec![source_offer];

    let syntax = parse_syntax_document(
        "form window {\n source: fixture/measurement-inputs\n retain: data/measurement-count-window\n project: data/measurement-plot(points = 2, when-full = \"evenly-spaced\")\n result: conduit-test/measurement-plot-sink\n source.profile > retain.profile\n source.measurement > retain.measurement\n retain.window > project.window\n project.series > result.series\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "window", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == "fixture/measurement-inputs" {
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
            "fixture/measurement-profile-line",
            "fixture/measurement-profile-binding",
            "fixture/measurement-profile-base",
        ),
        (
            "fixture/measurement-samples-line",
            "fixture/measurement-samples-binding",
            "fixture/measurement-samples-base",
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
    let mut candidates = BTreeMap::new();
    for (crossing, line) in expanded
        .connections
        .iter()
        .filter(|cord| cord.source_gear_id.as_str() == "window/source")
        .zip(&lines)
    {
        candidates.insert(
            (
                crossing.source_gear_id.clone(),
                crossing.sink_gear_id.clone(),
            ),
            vec![line.line_id.clone()],
        );
    }
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

fn leaf(value_type: conduit_core::StructuredInfoType, payload: Vec<u8>) -> Vec<u8> {
    StructuredInfoValue::leaf(value_type, payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn drain_to_remote_idle(scheduler: &mut TourScheduler, fragment: &PlanFragment) {
    match drive(scheduler, fragment) {
        Err(error) => assert_eq!(error, "Tour Play became idle"),
        Ok(_) => panic!("expected the kernel to await the next remote value"),
    }
}

#[test]
fn planned_browser_window_retains_exact_profile_samples_and_drop_evidence() {
    let fragment = fragment();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert_eq!(lowered.remote_endpoints.len(), 2);
    let mut scheduler = prepare_scheduler(&fragment, &lowered).unwrap();
    let profile_type = conduit_data::measurement_window_profile_type();
    let profile_endpoint = lowered
        .remote_endpoints
        .iter()
        .find(|endpoint| endpoint.temporal == conduit_core::PortTemporal::Value)
        .unwrap();
    let sample_type = conduit_data::measurement_sample_type();
    let sample_endpoint = lowered
        .remote_endpoints
        .iter()
        .find(|endpoint| endpoint.temporal == conduit_core::PortTemporal::Flow { closes: true })
        .unwrap();
    let profile = MeasurementWindowProfile {
        capacity: 2,
        unit: QuantityUnit::Millivolt,
        range: MeasurementRange {
            minimum: Quantity::new(0, QuantityUnit::Millivolt),
            maximum: Quantity::new(100, QuantityUnit::Millivolt),
        },
        clock_basis: "fixture-clock".into(),
        full_policy: FullWindowPolicy::DropOldest,
    };
    let profile = leaf(
        profile_type,
        conduit_data::encode_measurement_window_profile(&profile).unwrap(),
    );
    scheduler
        .admit_remote_input(
            profile_endpoint.endpoint,
            profile_endpoint.cord,
            0,
            &profile,
        )
        .unwrap();
    scheduler
        .close_remote_input(profile_endpoint.endpoint, profile_endpoint.cord)
        .unwrap();
    drain_to_remote_idle(&mut scheduler, &fragment);
    for (sequence, (value, ticks)) in [(0, (0, 1)), (1, (50, 2)), (2, (100, 3))] {
        let sample = MeasurementSample {
            value: Quantity::new(value, QuantityUnit::Millivolt),
            observed_at: TemporalInstant {
                ticks,
                scale: TemporalScale::Milliseconds,
                clock_basis: "fixture-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            uncertainty: None,
        };
        let sample = leaf(
            sample_type.clone(),
            conduit_data::encode_measurement_sample(&sample).unwrap(),
        );
        scheduler
            .admit_remote_input(
                sample_endpoint.endpoint,
                sample_endpoint.cord,
                sequence,
                &sample,
            )
            .unwrap();
        drain_to_remote_idle(&mut scheduler, &fragment);
    }
    scheduler
        .close_remote_input(sample_endpoint.endpoint, sample_endpoint.cord)
        .unwrap();

    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected plot manifestation")
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected plot manifestation")
    };
    let value = StructuredInfoValue::from_canonical_bytes(&output.canonical_value).unwrap();
    let conduit_core::StructuredInfoValueShape::Leaf(payload) = value.shape() else {
        panic!("expected plot leaf")
    };
    let series = conduit_data::decode_measurement_plot_series(payload).unwrap();
    assert_eq!((series.source_samples(), series.omitted_samples()), (2, 0));
    assert_eq!(
        series
            .points()
            .iter()
            .map(|point| point.value_millionths)
            .collect::<Vec<_>>(),
        [500_000, 1_000_000]
    );
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Complete
    ));
}
