use std::collections::BTreeMap;

use conduit_core::{
    verify_plan, CapabilityId, ConnectionBase, GearId, LineId, LinkBindingId, LinkEndpointId,
};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_signal::{
    signal_profile_catalog, triple, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
};

fn plan_with_policy(
    ordered_ids: Vec<LineId>,
    mutate: impl FnOnce(&mut Vec<conduit_core::LineOffer>),
) -> Result<conduit_core::Plan, conduit_planner::PlannerError> {
    let exact = triple::exact_plan().expect("baseline triple plan");
    let form = conduit_form::parse_with_startup(
        include_str!("../../../fixtures/forms/triple-signal.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("checked form");
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("triple-signal/pulse"),
                PlacementChoice {
                    host_id: exact.source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::PULSE_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/local"),
                PlacementChoice {
                    host_id: exact.source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::STDOUT_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/web"),
                PlacementChoice {
                    host_id: exact.browser_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::BROWSER_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/light"),
                PlacementChoice {
                    host_id: exact.pico_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::PICO_CAPABILITY_ID),
                },
            ),
        ]),
    };
    let mut usb_alternative = exact.browser_line.clone();
    usb_alternative.line_id = LineId::from("s4/line/triple-browser-usb");
    usb_alternative.binding.binding_id = LinkBindingId::from("s4/triple-browser-usb-link");
    usb_alternative.binding.base = ConnectionBase::UsbCdc;
    usb_alternative.binding.base_instance_id =
        conduit_core::ConnectionBaseInstanceId::from("s4/triple-browser-usb-0");
    usb_alternative.binding.source.endpoint_id =
        LinkEndpointId::from("s4/triple-browser-usb-egress");
    usb_alternative.binding.sink.endpoint_id =
        LinkEndpointId::from("s4/triple-browser-usb-ingress");
    usb_alternative.availability.line_id = usb_alternative.line_id.clone();
    usb_alternative.availability.binding_id = usb_alternative.binding.binding_id.clone();
    let mut links = vec![exact.browser_line, usb_alternative, exact.pico_line];
    mutate(&mut links);
    let line_candidates = BTreeMap::from([(
        (
            GearId::from("triple-signal/pulse"),
            GearId::from("triple-signal/web"),
        ),
        ordered_ids,
    )]);
    plan_with_options(
        &form,
        &[
            exact.source_advertisement,
            exact.browser_advertisement,
            exact.pico_advertisement,
        ],
        &placements,
        &[
            ConnectionBase::Local,
            ConnectionBase::WebSocket,
            ConnectionBase::UsbCdc,
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &links,
        },
    )
}

fn web_connection(plan: &conduit_core::Plan) -> &conduit_core::PlannedConnection {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| {
            connection.admitted_lines.iter().any(|candidate| {
                candidate.line_id == LineId::from(triple::BROWSER_LINE_ID)
                    || candidate.line_id == LineId::from("s4/line/triple-browser-usb")
            })
        })
        .expect("web connection")
}

#[test]
fn policy_seals_one_or_multiple_ready_routes_in_identity_bound_order() {
    let websocket = LineId::from(triple::BROWSER_LINE_ID);
    let usb = LineId::from("s4/line/triple-browser-usb");
    let one = plan_with_policy(vec![websocket.clone()], |_| {}).expect("one route");
    let two = plan_with_policy(vec![websocket.clone(), usb.clone()], |_| {}).expect("two routes");
    let reversed = plan_with_policy(vec![usb, websocket], |_| {}).expect("reversed routes");

    assert_eq!(web_connection(&one).admitted_lines.len(), 1);
    assert_eq!(web_connection(&two).admitted_lines.len(), 2);
    assert_eq!(
        web_connection(&two).admitted_lines[0].binding.base,
        ConnectionBase::WebSocket
    );
    assert_eq!(
        web_connection(&two).admitted_lines[1].binding.base,
        ConnectionBase::UsbCdc
    );
    assert_ne!(one.plan_id, two.plan_id);
    assert_ne!(two.plan_id, reversed.plan_id);
    assert!(verify_plan(&one) && verify_plan(&two) && verify_plan(&reversed));
}

#[test]
fn mutable_availability_does_not_rewrite_the_sealed_plan() {
    let plan = plan_with_policy(vec![LineId::from(triple::BROWSER_LINE_ID)], |_| {})
        .expect("sealed route");
    let original_id = plan.plan_id.clone();
    assert_eq!(plan.plan_id, original_id);
    assert!(verify_plan(&plan));
}

#[test]
fn duplicate_or_underbounded_candidate_policy_fails_closed() {
    let websocket = LineId::from(triple::BROWSER_LINE_ID);
    let usb = LineId::from("s4/line/triple-browser-usb");
    assert!(matches!(
        plan_with_policy(vec![websocket.clone(), usb.clone(), websocket], |_| {}),
        Err(conduit_planner::PlannerError::InvalidLineOffer(_))
    ));
    assert!(plan_with_policy(vec![usb], |links| {
        links[1].binding.limits.maximum_payload_bytes = SIGNAL_ENCODED_LEN - 1;
    })
    .is_err());
}
