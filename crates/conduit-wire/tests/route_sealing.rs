use conduit_core::{
    process_owned_link_binding, BootId, ConnectionId, ConnectionProvider, FragmentId,
    HostAdvertisement, HostId, HostProfileId, KindId, LinkBindingId, OfferGeneration, PlanId,
    PlannedConnection, PortId, PortTemporal, PROTOCOL_VERSION,
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
    let connection = PlannedConnection {
        connection_id: ConnectionId::from("connection"),
        source_placement_id: conduit_core::PlacementId::from("source-placement"),
        source_port_id: PortId::from("out"),
        sink_placement_id: conduit_core::PlacementId::from("sink-placement"),
        sink_port_id: PortId::from("in"),
        value_kind: KindId::from("value"),
        temporal: PortTemporal::Flow { closes: true },
        provider: ConnectionProvider::UsbCdc,
        link_binding: Some(unsealed),
        route_candidates: vec![sealed.bound_link()],
        item_capacity: 1,
        byte_capacity: 64,
    };

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
