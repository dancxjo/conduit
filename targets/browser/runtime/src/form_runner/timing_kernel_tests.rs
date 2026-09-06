//! Deterministic planned remote ingress; no live Line or human-input claim.
use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, LinkLimits, PortDirection,
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
        crate::installed_browser::advertisement("timing-browser".into(), "timing-boot".into());
    let sink = crate::installed_browser::test_timing_sink::offer();
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
    let mut source_offer = browser
        .capabilities
        .iter()
        .find(|o| o.kind_id.as_str() == conduit_semantic_catalog::ORDERED_EVENT_INTERVALS_KIND)
        .unwrap()
        .clone();
    source_offer.kind_id = "fixture/timed-events".into();
    source_offer.kind_contract_revision = "fixture/timed-events@1".into();
    source_offer.capability_id = "fixture/timed-events".into();
    source_offer.outputs = source_offer.inputs.clone();
    source_offer.outputs[0].direction = PortDirection::Output;
    source_offer.inputs.clear();
    source_offer.host_operations.clear();
    source_offer.implementation.implementation_id = "fixture/timed-events@1".into();
    source_offer.implementation.artifact_id = "fixture/timed-events@1".into();
    startup
        .insert(KindSignature {
            kind: "fixture/timed-events".into(),
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
    source_host.host_id = "fixture/timing-source".into();
    source_host.boot_id = "fixture/timing-boot".into();
    source_host.capabilities = vec![source_offer];
    let syntax = parse_syntax_document("form timing {\n source: fixture/timed-events\n derive: time/ordered-event-intervals\n normalize: sequence/normalize-relative-duration\n result: conduit-test/timing-sink\n source.events > derive.events\n derive.intervals > normalize.intervals\n normalize.normalized > result.normalized\n}\n");
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "timing", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == "fixture/timed-events" {
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
                            .find(|o| o.kind_id == gear.kind_id)
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
        "fixture/timing-line",
        "fixture/timing-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "fixture/timing-base",
        &source_host,
        &browser,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: maximum,
            maximum_buffered_bytes: maximum * 4,
            maximum_frame_bytes: maximum * 2,
        },
    );
    let crossing = expanded
        .connections
        .iter()
        .find(|cord| cord.source_gear_id.as_str() == "timing/source")
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

#[test]
fn planned_browser_timing_ingress_runs_both_host_operations_to_completion() {
    let fragment = fragment();
    let (mut scheduler, lowered) = prepare_remote_fragment(&fragment).unwrap();
    let remote = &lowered.remote_endpoints[0];
    let capacity = scheduler.values().allocation_capacities();
    let bytes =
        conduit_semantic_catalog::timed_event_sequence_value("fixture/clock", &[100, 200, 500])
            .unwrap()
            .canonical_bytes()
            .unwrap();
    scheduler
        .admit_remote_input(remote.endpoint, remote.cord, 0, &bytes)
        .unwrap();
    scheduler
        .close_remote_input(remote.endpoint, remote.cord)
        .unwrap();
    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected the typed fixture observation");
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected fixture manifestation");
    };
    assert_eq!(
        output.canonical_value,
        conduit_semantic_catalog::normalized_value(&[333_333, 1_000_000])
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Complete
    ));
    assert_eq!(scheduler.values().allocation_capacities(), capacity);
    assert_eq!(
        scheduler
            .signs()
            .events()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted)
            .count(),
        3
    );
}

#[test]
fn invalid_remote_timing_preserves_kernel_failure_and_does_not_run_normalization() {
    let fragment = fragment();
    let (mut scheduler, lowered) = prepare_remote_fragment(&fragment).unwrap();
    let remote = &lowered.remote_endpoints[0];
    scheduler
        .admit_remote_input(remote.endpoint, remote.cord, 0, &[0])
        .unwrap();
    scheduler
        .close_remote_input(remote.endpoint, remote.cord)
        .unwrap();
    assert!(
        matches!(drive(&mut scheduler, &fragment), Err(error) if error == "OperationFailed(1)")
    );
    assert_eq!(
        scheduler
            .signs()
            .events()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted)
            .count(),
        1
    );
}
