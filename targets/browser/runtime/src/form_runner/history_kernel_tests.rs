//! Deterministic planned history ingress; no live Line or storage claim.
use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, BoundedResourceRef, LinkLimits,
    PortDirection, ResourceClassId, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity, StructuredInfoType, TemporalInstant, TemporalScale,
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
        crate::installed_browser::advertisement("history-browser".into(), "history-boot".into());
    let sink = crate::installed_browser::test_replay_sink::offer();
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

    let history = browser
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_time::HISTORICAL_TIMELINE_KIND)
        .unwrap();
    let mut source_offer = history.clone();
    source_offer.kind_id = "fixture/history-command".into();
    source_offer.kind_contract_revision = "fixture/history-command@1".into();
    source_offer.capability_id = "fixture/history-command".into();
    source_offer.outputs = source_offer.inputs.clone();
    source_offer.outputs[0].direction = PortDirection::Output;
    source_offer.inputs.clear();
    source_offer.host_operations.clear();
    source_offer.startup_parameters.clear();
    source_offer.implementation.implementation_id = "fixture/history-command@1".into();
    source_offer.implementation.artifact_id = "fixture/history-command@1".into();
    startup
        .insert(KindSignature {
            kind: "fixture/history-command".into(),
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
    source_host.host_id = "fixture/history-source".into();
    source_host.boot_id = "fixture/history-source-boot".into();
    source_host.capabilities = vec![source_offer];

    let syntax = parse_syntax_document(
        "form history {\n source: fixture/history-command\n history: history/bounded-typed(maximum-entries = 4)\n replay: history/replay-source\n result: conduit-test/replay-sink\n source.command > history.command\n history.timeline > replay.timeline\n replay.replay > result.replay\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "history", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == "fixture/history-command" {
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
        "fixture/history-line",
        "fixture/history-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "fixture/history-base",
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
        .find(|cord| cord.source_gear_id.as_str() == "history/source")
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

fn append_command() -> Vec<u8> {
    let mut command = [0; conduit_time::MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES];
    let length = conduit_time::encode_historical_timeline_command_into(
        &conduit_time::HistoricalTimelineCommand::Append {
            identity: "memory/one".into(),
            event_time: TemporalInstant {
                ticks: 100,
                scale: TemporalScale::Milliseconds,
                clock_basis: "history/event-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            origin: conduit_time::HistoricalEntryOrigin::OperatorAuthored,
            value: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([1; 32]),
                content_profile: conduit_core::kind_id("value/text@1"),
                access_class: ResourceClassId::from("conduit.resource/history-value@1"),
                extent: ResourceExtent {
                    bytes: 4,
                    items: Some(1),
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([2; 32]),
                    expires_at: None,
                },
            },
        },
        &mut command,
    )
    .unwrap();
    conduit_core::StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(
            conduit_time::HISTORICAL_TIMELINE_COMMAND_INFO_ID,
        ))
        .unwrap(),
        command[..length].to_vec(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn exact_leaf<'a>(canonical: &'a [u8], value_type: &[u8]) -> Option<&'a [u8]> {
    let node = canonical.strip_prefix(value_type)?;
    if node.first() != Some(&0) || node.len() < 5 {
        return None;
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().ok()?)).ok()?;
    (node.len() == 5 + length).then_some(&node[5..])
}

#[test]
fn planned_browser_history_projects_replay_through_the_production_kernel() {
    let fragment = fragment();
    let (mut scheduler, lowered) = prepare_remote_fragment(&fragment).unwrap();
    let remote = &lowered.remote_endpoints[0];
    let capacities = scheduler.values().allocation_capacities();
    scheduler
        .admit_remote_input(remote.endpoint, remote.cord, 0, &append_command())
        .unwrap();
    scheduler
        .close_remote_input(remote.endpoint, remote.cord)
        .unwrap();

    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected the typed replay manifestation");
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected replay manifestation");
    };
    let replay_type = StructuredInfoType::leaf(conduit_core::kind_id("history/replay-timeline@1"))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let replay = conduit_time::decode_replay_timeline(
        exact_leaf(&output.canonical_value, &replay_type).unwrap(),
    )
    .unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].identity, "memory/one");
    assert_eq!(replay[0].event_time.ticks, 100);
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
        3
    );
}
