//! Deterministic remote point flow into the production bounded-stroke operation.

use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, LinkLimits, PortDirection,
    Quantity, QuantityUnit,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

fn fragment() -> PlanFragment {
    let (mut startup, mut catalog) = crate::installed_browser::catalogs().unwrap();
    let mut browser = crate::installed_browser::advertisement(
        "stroke-browser".into(),
        "stroke-browser-boot".into(),
    );
    let sink = crate::installed_browser::test_stroke_sink::offer();
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

    let capture = browser
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_presentation::CAPTURE_BOUNDED_STROKE_KIND)
        .unwrap();
    let mut source_offer = capture.clone();
    source_offer.kind_id = "fixture/point-flow".into();
    source_offer.kind_contract_revision = "fixture/point-flow@1".into();
    source_offer.capability_id = "fixture/point-flow".into();
    source_offer.outputs = source_offer.inputs.clone();
    source_offer.outputs[0].direction = PortDirection::Output;
    source_offer.inputs.clear();
    source_offer.host_operations.clear();
    source_offer.startup_parameters.clear();
    source_offer.implementation.implementation_id = "fixture/point-flow@1".into();
    source_offer.implementation.artifact_id = "fixture/point-flow@1".into();
    startup
        .insert(KindSignature {
            kind: "fixture/point-flow".into(),
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
    source_host.host_id = "fixture/stroke-source".into();
    source_host.boot_id = "fixture/stroke-source-boot".into();
    source_host.capabilities = vec![source_offer];

    let syntax = parse_syntax_document(
        "form stroke {\n source: fixture/point-flow\n capture: bounded-stroke-capture\n result: conduit-test/stroke-sink\n source.point > capture.point\n capture.stroke > result.stroke\n}\n\nform bounded-stroke-capture (\n point: Point2...| > stroke: Path2Four\n) {\n capture: geometry/capture-bounded-stroke\n point > capture.point\n capture.stroke > stroke\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "stroke", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == "fixture/point-flow" {
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
        "fixture/stroke-line",
        "fixture/stroke-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "fixture/stroke-base",
        &source_host,
        &browser,
        LinkLimits {
            maximum_in_flight_items: 4,
            maximum_payload_bytes: maximum,
            maximum_buffered_bytes: maximum * 4,
            maximum_frame_bytes: maximum * 2,
        },
    );
    let crossing = expanded
        .connections
        .iter()
        .find(|cord| cord.source_gear_id.as_str() == "stroke/source")
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
            connection_item_capacity: 4,
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

fn point(x: i64) -> Vec<u8> {
    conduit_presentation::point2_value(
        "controller/normalized",
        Quantity::new(x, QuantityUnit::Millimeter),
        Quantity::new(0, QuantityUnit::Millimeter),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn planned_browser_capture_retains_four_ordered_points_without_new_allocations() {
    let fragment = fragment();
    let (mut scheduler, lowered) = prepare_remote_fragment(&fragment).unwrap();
    let remote = &lowered.remote_endpoints[0];
    let capacities = scheduler.values().allocation_capacities();
    for (sequence, x) in [3, 5, 8, 13].into_iter().enumerate() {
        scheduler
            .admit_remote_input(
                remote.endpoint,
                remote.cord,
                u64::try_from(sequence).unwrap(),
                &point(x),
            )
            .unwrap();
    }
    scheduler
        .close_remote_input(remote.endpoint, remote.cord)
        .unwrap();

    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected the exact stroke manifestation");
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected stroke manifestation");
    };
    assert_eq!(
        output.kind_id,
        crate::installed_browser::test_stroke_sink::KIND
    );
    let path =
        conduit_core::StructuredInfoValue::from_canonical_bytes(&output.canonical_value).unwrap();
    assert_eq!(
        path.value_type(),
        &conduit_presentation::path2_type(4).unwrap()
    );
    let expected = conduit_presentation::path2_value(
        [3, 5, 8, 13]
            .into_iter()
            .map(|x| conduit_core::StructuredInfoValue::from_canonical_bytes(&point(x)).unwrap())
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert_eq!(output.canonical_value, expected);
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Quiescent
    ));
    assert_eq!(scheduler.values().allocation_capacities(), capacities);
    assert_eq!(
        scheduler
            .signs()
            .events()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted)
            .count(),
        6
    );
}
