use conduit_core::{
    process_owned_link_binding, BootId, ConnectionId, ConnectionProvider, FragmentId,
    HostAdvertisement, HostId, HostProfileId, KindId, LinkAvailability, LinkBindingId,
    OfferGeneration, PlanId, PlannedConnection, PortId, PortTemporal, PROTOCOL_VERSION,
};
use conduit_wire::{SessionBinding, WireError};

fn host(id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(id),
        boot_id: BootId::from(format!("{id}/boot")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test"),
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

fn connection_with_routes(
    selected: conduit_core::LinkBinding,
    routes: Vec<conduit_core::BoundLink>,
) -> PlannedConnection {
    PlannedConnection {
        connection_id: ConnectionId::from("connection"),
        source_placement_id: conduit_core::PlacementId::from("source-placement"),
        source_port_id: PortId::from("out"),
        sink_placement_id: conduit_core::PlacementId::from("sink-placement"),
        sink_port_id: PortId::from("in"),
        value_kind: KindId::from("value"),
        temporal: PortTemporal::Flow { closes: true },
        provider: selected.provider,
        link_binding: Some(selected),
        route_candidates: routes,
        item_capacity: 1,
        byte_capacity: 64,
    }
}

#[test]
fn session_rejects_selected_link_outside_sealed_candidates() {
    let source = host("source");
    let sink = host("sink");
    let sealed = process_owned_link_binding(
        "sealed",
        ConnectionProvider::UsbCdc,
        "usb/0",
        &source,
        &sink,
        1,
        64,
    );
    let mut unsealed = process_owned_link_binding(
        "unsealed",
        ConnectionProvider::UsbCdc,
        "usb/1",
        &source,
        &sink,
        1,
        64,
    );
    unsealed.binding_id = LinkBindingId::from("unsealed");
    let connection = connection_with_routes(unsealed, vec![sealed.bound_link()]);

    assert_eq!(
        SessionBinding::from_planned_connection(
            PlanId::from("plan"),
            FragmentId::from("source-fragment"),
            FragmentId::from("sink-fragment"),
            &connection,
        ),
        Err(WireError::InvalidSession)
    );
}

#[test]
fn two_sealed_carriers_share_one_logical_identity_but_keep_exact_attachments() {
    let source = host("source");
    let sink = host("sink");
    let mut usb = process_owned_link_binding(
        "usb",
        ConnectionProvider::UsbCdc,
        "usb/0",
        &source,
        &sink,
        1,
        64,
    );
    let mut websocket = process_owned_link_binding(
        "websocket",
        ConnectionProvider::WebSocket,
        "websocket/0",
        &source,
        &sink,
        1,
        64,
    );
    usb.limits.maximum_frame_bytes = 2_048;
    websocket.limits.maximum_frame_bytes = 2_048;
    let connection =
        connection_with_routes(usb.clone(), vec![usb.bound_link(), websocket.bound_link()]);
    let make = |link| {
        SessionBinding::from_planned_connection_with_link(
            PlanId::from("plan"),
            FragmentId::from("source-fragment"),
            FragmentId::from("sink-fragment"),
            &connection,
            link,
        )
        .expect("sealed ready attachment")
    };
    let usb_session = make(&usb);
    let websocket_session = make(&websocket);

    assert_eq!(usb_session.identity(), websocket_session.identity());
    assert_ne!(usb_session.attachment, websocket_session.attachment);
    assert_eq!(usb_session.attachment.provider, ConnectionProvider::UsbCdc);
    assert_eq!(
        websocket_session.attachment.provider,
        ConnectionProvider::WebSocket
    );

    let mut unavailable = websocket;
    unavailable.availability = LinkAvailability::Unavailable;
    assert_eq!(
        SessionBinding::from_planned_connection_with_link(
            PlanId::from("plan"),
            FragmentId::from("source-fragment"),
            FragmentId::from("sink-fragment"),
            &connection,
            &unavailable,
        ),
        Err(WireError::InvalidSession)
    );

    let mut mismatched = usb_session;
    mismatched.attachment.sink_boot_id = BootId::from("different-boot");
    assert_eq!(mismatched.validate(), Err(WireError::InvalidSession));
}
