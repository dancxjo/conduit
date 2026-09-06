use super::*;
use conduit_core::*;
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../../../../forms/button-across-room/main.conduit");

fn fixture() -> (Plan, HostAdvertisement, HostAdvertisement, SessionBinding) {
    let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
    let checked =
        conduit_form::check_syntax_document(&conduit_form::parse_syntax_document(SOURCE), &startup)
            .unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "button_across_room", &catalog).unwrap();
    let source = crate::installed_browser::advertisement("remote/a".into(), "boot/a".into());
    let sink = crate::installed_browser::advertisement("remote/b".into(), "boot/b".into());
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == "input/button" {
                    &source
                } else {
                    &sink
                };
                let offer = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.kind_id == gear.kind_id)
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: offer.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let mut line = process_owned_line_offer_with_limits(
        "remote/line",
        "remote/binding",
        "conduit.base/webrtc-data-channel@1".into(),
        "remote/base",
        &source,
        &sink,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 512,
            maximum_buffered_bytes: 512,
            maximum_frame_bytes: 4096,
        },
    );
    // This is an independently offered WebRTC route before planning, not a
    // rewritten selected memory route or a transport attached after Plan seal.
    line.contract = LineContract {
        scope: LineScope::PointToPoint,
        traffic_shape: LineTrafficShape::Message,
        duplex: LineDuplex::FullDuplex,
        ordering: LineOrdering::Ordered,
        reliability: LineReliability::Reliable,
        continuation: LineContinuation::None,
        security: LineSecurity::AuthenticatedEncrypted,
    };
    let root = expanded
        .connections
        .iter()
        .find(|connection| {
            placements.by_gear[&connection.source_gear_id].host_id
                != placements.by_gear[&connection.sink_gear_id].host_id
        })
        .unwrap();
    let candidates = BTreeMap::from([(
        (root.source_gear_id.clone(), root.sink_gear_id.clone()),
        vec![line.line_id.clone()],
    )]);
    let mut bases = crate::installed_browser::local_bases().to_vec();
    bases.push("conduit.base/webrtc-data-channel@1".into());
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &[source.clone(), sink.clone()],
        &placements,
        &bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 512,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .unwrap();
    let a = plan
        .fragments
        .iter()
        .find(|f| f.host_id == source.host_id)
        .unwrap();
    let b = plan
        .fragments
        .iter()
        .find(|f| f.host_id == sink.host_id)
        .unwrap();
    let connection = a
        .connections
        .iter()
        .find(|c| c.selected_line.is_some())
        .unwrap();
    let binding = SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        a.fragment_id.clone(),
        b.fragment_id.clone(),
        connection,
    )
    .unwrap();
    (plan, source, sink, binding)
}

fn observations(host: &HostAdvertisement) -> Vec<ResourceObservation> {
    host.resources
        .iter()
        .enumerate()
        .map(|(index, pool)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: format!("sign/resource-{index}").into(),
        })
        .collect()
}

#[test]
fn exact_web_rtc_fragments_execute_button_transitions_through_kernel() {
    let (plan, a, b, binding) = fixture();
    let mut source = RemoteExecution::prepare(
        &plan,
        &a,
        &binding,
        &binding.source_active_play_id,
        &observations(&a),
    )
    .unwrap();
    let mut sink = RemoteExecution::prepare(
        &plan,
        &b,
        &binding,
        &binding.sink_active_play_id,
        &observations(&b),
    )
    .unwrap();
    assert!(matches!(
        sink.drive().unwrap(),
        DriveStatus::Waiting { pending_effects: 0 }
    ));
    assert!(sink.offer().is_err());
    assert!(source.admit(0, &[1]).is_err());
    for (sequence, pressed) in [(0_u64, true), (1, false)] {
        let DriveStatus::Effect(input) = source.drive().unwrap() else {
            panic!("expected button request")
        };
        let bytes =
            conduit_semantic_catalog::button_transition_value("button/primary", pressed, sequence)
                .unwrap()
                .canonical_bytes()
                .unwrap();
        source.complete_effect(&input, Some(&bytes)).unwrap();
        assert!(matches!(
            source.drive().unwrap(),
            DriveStatus::Waiting { .. }
        ));
        let offer = source.offer().unwrap().unwrap();
        assert_eq!(offer.sequence, sequence);
        assert_eq!(offer.payload, bytes);
        assert!(source.accepted(sequence + 1).is_err());
        assert!(matches!(
            source.drive().unwrap(),
            DriveStatus::Waiting { .. }
        ));
        assert_eq!(source.offer().unwrap().unwrap(), offer);
        assert!(source.delivered(sequence).is_err());
        assert!(matches!(
            sink.admit(sequence, &offer.payload).unwrap(),
            RemoteIngressOutcome::Accepted { .. }
        ));
        source.accepted(sequence).unwrap();
        let DriveStatus::Effect(presentation) = sink.drive().unwrap() else {
            panic!("expected indicator request")
        };
        let engine::BrowserHostEffect::Manifestation(value) = &presentation.effect else {
            panic!("expected indicator manifestation")
        };
        assert_eq!(
            value.kind_id,
            conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_KIND
        );
        assert_eq!(
            InfoBool::decode(&value.canonical_value).unwrap(),
            if pressed {
                InfoBool::TRUE
            } else {
                InfoBool::FALSE
            }
        );
        sink.complete_effect(&presentation, None).unwrap();
        source.delivered(sequence).unwrap();
    }
    assert!(matches!(source.drive().unwrap(), DriveStatus::Complete));
    assert!(source.terminal().unwrap());
    sink.close_input().unwrap();
    assert!(matches!(sink.drive().unwrap(), DriveStatus::Complete));
    source.cancel().unwrap();
}

#[test]
fn preparation_refuses_stale_identity_and_missing_resource_admission() {
    let (plan, a, b, binding) = fixture();
    assert!(
        RemoteExecution::prepare(&plan, &b, &binding, &binding.sink_active_play_id, &[]).is_err()
    );
    assert!(RemoteExecution::prepare(
        &plan,
        &a,
        &binding,
        &binding.sink_active_play_id,
        &observations(&a)
    )
    .is_err());
    let mut stale = a.clone();
    stale.boot_id = "stale/boot".into();
    assert!(RemoteExecution::prepare(
        &plan,
        &stale,
        &binding,
        &binding.source_active_play_id,
        &observations(&stale)
    )
    .is_err());
    let mut stale_binding = binding.clone();
    stale_binding.connection_id = "stale/connection".into();
    assert!(RemoteExecution::prepare(
        &plan,
        &a,
        &stale_binding,
        &binding.source_active_play_id,
        &observations(&a)
    )
    .is_err());
    let mut altered = plan.clone();
    altered.fragments[0].placements[0].artifact_id = "wrong/artifact".into();
    assert!(RemoteExecution::prepare(
        &altered,
        &a,
        &binding,
        &binding.source_active_play_id,
        &observations(&a)
    )
    .is_err());
}
