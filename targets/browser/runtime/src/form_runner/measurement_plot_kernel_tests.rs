//! Deterministic measurement ingress through the installed browser production kernel.

use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, LinkLimits, PortDirection,
    Quantity, QuantityUnit, StructuredInfoValue, TemporalInstant, TemporalScale,
};
use conduit_data::{
    BoundedMeasurementWindow, FullWindowPolicy, MeasurementRange, MeasurementSample,
    MeasurementWindowProfile,
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
        crate::installed_browser::advertisement("plot-browser".into(), "plot-boot".into());
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

    let plot = browser
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_data::MEASUREMENT_PLOT_KIND)
        .unwrap();
    let mut source_offer = plot.clone();
    source_offer.kind_id = "fixture/measurement-window".into();
    source_offer.kind_contract_revision = "fixture/measurement-window@1".into();
    source_offer.capability_id = "fixture/measurement-window".into();
    source_offer.outputs = source_offer.inputs.clone();
    source_offer.outputs[0].direction = PortDirection::Output;
    source_offer.inputs.clear();
    source_offer.host_operations.clear();
    source_offer.startup_parameters.clear();
    source_offer.implementation.implementation_id = "fixture/measurement-window@1".into();
    source_offer.implementation.artifact_id = "fixture/measurement-window@1".into();
    startup
        .insert(KindSignature {
            kind: "fixture/measurement-window".into(),
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

    let syntax = parse_syntax_document("form plot {\n source: fixture/measurement-window\n project: data/measurement-plot(points = 2, when-full = \"evenly-spaced\")\n result: conduit-test/measurement-plot-sink\n source.window > project.window\n project.series > result.series\n}\n");
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "plot", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == "fixture/measurement-window" {
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
    let line = process_owned_line_offer_with_limits(
        "fixture/measurement-line",
        "fixture/measurement-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "fixture/measurement-base",
        &source_host,
        &browser,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: maximum,
            maximum_buffered_bytes: maximum * 2,
            maximum_frame_bytes: maximum * 2,
        },
    );
    let crossing = expanded
        .connections
        .iter()
        .find(|cord| cord.source_gear_id.as_str() == "plot/source")
        .unwrap();
    let candidates = BTreeMap::from([(
        (
            crossing.source_gear_id.clone(),
            crossing.sink_gear_id.clone(),
        ),
        vec![line.line_id.clone()],
    )]);
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
            line_offers: &[line],
        },
    )
    .unwrap()
    .fragments
    .into_iter()
    .find(|fragment| fragment.host_id == browser.host_id)
    .unwrap()
}

fn window_value() -> Vec<u8> {
    let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
        capacity: 3,
        unit: QuantityUnit::Millivolt,
        range: MeasurementRange {
            minimum: Quantity::new(0, QuantityUnit::Millivolt),
            maximum: Quantity::new(100, QuantityUnit::Millivolt),
        },
        clock_basis: "fixture-clock".into(),
        full_policy: FullWindowPolicy::Reject,
    })
    .unwrap();
    for (value, ticks) in [(0, 1), (50, 2), (100, 3)] {
        window
            .push(MeasurementSample {
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
            .unwrap();
    }
    StructuredInfoValue::leaf(
        conduit_data::measurement_window_type(),
        conduit_data::encode_measurement_window(&window).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn planned_browser_plot_projects_through_the_production_kernel() {
    let fragment = fragment();
    let (mut scheduler, lowered) = prepare_remote_fragment(&fragment).unwrap();
    let remote = &lowered.remote_endpoints[0];
    let capacities = scheduler.values().allocation_capacities();
    scheduler
        .admit_remote_input(remote.endpoint, remote.cord, 0, &window_value())
        .unwrap();
    scheduler
        .close_remote_input(remote.endpoint, remote.cord)
        .unwrap();
    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected plot manifestation")
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected plot manifestation")
    };
    let type_bytes = conduit_data::measurement_plot_series_type()
        .canonical_bytes()
        .unwrap();
    let node = output
        .canonical_value
        .strip_prefix(type_bytes.as_slice())
        .unwrap();
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().unwrap())).unwrap();
    let series = conduit_data::decode_measurement_plot_series(&node[5..5 + length]).unwrap();
    assert_eq!((series.source_samples(), series.omitted_samples()), (3, 1));
    assert_eq!(
        series
            .points()
            .iter()
            .map(|point| point.source_index)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Quiescent
    ));
    assert_eq!(scheduler.values().allocation_capacities(), capacities);
}
