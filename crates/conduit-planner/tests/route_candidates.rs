use std::collections::BTreeMap;

use conduit_core::{
    verify_plan, CapabilityId, ConnectionProvider, LinkBindingId, LinkEndpointId, OperationId,
};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_signal::{
    signal_profile_catalog, triple, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
};

fn plan_with_policy(
    ordered_ids: Vec<LinkBindingId>,
    mutate: impl FnOnce(&mut Vec<conduit_core::LinkBinding>),
) -> Result<conduit_core::Plan, conduit_planner::PlannerError> {
    let exact = triple::exact_plan().expect("baseline triple plan");
    let form = conduit_form::parse(
        include_str!("../../../examples/triple-signal.form"),
        &signal_profile_catalog(),
    )
    .expect("checked form");
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("pulse"),
                PlacementChoice {
                    host_id: exact.source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::PULSE_CAPABILITY_ID),
                },
            ),
            (
                OperationId::from("local"),
                PlacementChoice {
                    host_id: exact.source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::STDOUT_CAPABILITY_ID),
                },
            ),
            (
                OperationId::from("web"),
                PlacementChoice {
                    host_id: exact.browser_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::BROWSER_CAPABILITY_ID),
                },
            ),
            (
                OperationId::from("light"),
                PlacementChoice {
                    host_id: exact.pico_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(triple::PICO_CAPABILITY_ID),
                },
            ),
        ]),
    };
    let mut usb_alternative = exact.browser_link.clone();
    usb_alternative.binding_id = LinkBindingId::from("s4/triple-browser-usb-link");
    usb_alternative.provider = ConnectionProvider::UsbCdc;
    usb_alternative.provider_instance_id =
        conduit_core::ConnectionProviderInstanceId::from("s4/triple-browser-usb-0");
    usb_alternative.source.endpoint_id = LinkEndpointId::from("s4/triple-browser-usb-egress");
    usb_alternative.sink.endpoint_id = LinkEndpointId::from("s4/triple-browser-usb-ingress");
    let mut links = vec![exact.browser_link, usb_alternative, exact.pico_link];
    mutate(&mut links);
    let route_candidates = BTreeMap::from([(
        (OperationId::from("pulse"), OperationId::from("web")),
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
            ConnectionProvider::Local,
            ConnectionProvider::WebSocket,
            ConnectionProvider::UsbCdc,
        ],
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
            route_candidates: &route_candidates,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            link_bindings: &links,
        },
    )
}

fn web_connection(plan: &conduit_core::Plan) -> &conduit_core::PlannedConnection {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| {
            connection.route_candidates.iter().any(|candidate| {
                candidate.binding_id == LinkBindingId::from(triple::BROWSER_LINK_ID)
                    || candidate.binding_id == LinkBindingId::from("s4/triple-browser-usb-link")
            })
        })
        .expect("web connection")
}

#[test]
fn policy_seals_one_or_multiple_ready_routes_in_identity_bound_order() {
    let websocket = LinkBindingId::from(triple::BROWSER_LINK_ID);
    let usb = LinkBindingId::from("s4/triple-browser-usb-link");
    let one = plan_with_policy(vec![websocket.clone()], |_| {}).expect("one route");
    let two = plan_with_policy(vec![websocket.clone(), usb.clone()], |_| {}).expect("two routes");
    let reversed = plan_with_policy(vec![usb, websocket], |_| {}).expect("reversed routes");

    assert_eq!(web_connection(&one).route_candidates.len(), 1);
    assert_eq!(web_connection(&two).route_candidates.len(), 2);
    assert_eq!(
        web_connection(&two).route_candidates[0].provider,
        ConnectionProvider::WebSocket
    );
    assert_eq!(
        web_connection(&two).route_candidates[1].provider,
        ConnectionProvider::UsbCdc
    );
    assert_ne!(one.plan_id, two.plan_id);
    assert_ne!(two.plan_id, reversed.plan_id);
    assert!(verify_plan(&one) && verify_plan(&two) && verify_plan(&reversed));
}

#[test]
fn mutable_availability_does_not_rewrite_the_sealed_plan() {
    let mut plan = plan_with_policy(vec![LinkBindingId::from(triple::BROWSER_LINK_ID)], |_| {})
        .expect("sealed route");
    let original_id = plan.plan_id.clone();
    for fragment in &mut plan.fragments {
        for connection in &mut fragment.connections {
            if connection.route_candidates.iter().any(|candidate| {
                candidate.binding_id == LinkBindingId::from(triple::BROWSER_LINK_ID)
            }) {
                connection
                    .link_binding
                    .as_mut()
                    .expect("observation")
                    .availability = conduit_core::LinkAvailability::Unavailable;
            }
        }
    }
    assert_eq!(plan.plan_id, original_id);
    assert!(verify_plan(&plan));
}

#[test]
fn duplicate_or_underbounded_candidate_policy_fails_closed() {
    let websocket = LinkBindingId::from(triple::BROWSER_LINK_ID);
    let usb = LinkBindingId::from("s4/triple-browser-usb-link");
    assert!(matches!(
        plan_with_policy(vec![websocket.clone(), usb.clone(), websocket], |_| {}),
        Err(conduit_planner::PlannerError::InvalidLinkBinding(_))
    ));
    assert!(plan_with_policy(vec![usb], |links| {
        links[1].limits.maximum_payload_bytes = SIGNAL_ENCODED_LEN - 1;
    })
    .is_err());
}
