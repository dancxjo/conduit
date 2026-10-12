//! Machine-local full-duplex Lines carry the same exact semantic sessions.
use conduit_core::*;
use conduit_wire::{SessionBinding, WireError};
fn host(id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(id),
        boot_id: BootId::from(format!("boot/{id}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("fixture/local"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn connection() -> PlannedConnection {
    let mut offer = process_owned_line_offer_with_limits(
        "line/owned-machine",
        "binding/owned-machine",
        BaseImplementationId::from("fixture/owned-pipes"),
        "instance/owned-machine",
        &host("source"),
        &host("sink"),
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 64,
            maximum_buffered_bytes: 64,
            maximum_frame_bytes: 4096,
        },
    );
    offer.contract.scope = LineScope::Machine;
    let line = offer.admitted_line();
    PlannedConnection {
        connection_id: ConnectionId::from("connection"),
        source_placement_id: PlacementId::from("source"),
        source_port_id: PortId::from("out"),
        sink_placement_id: PlacementId::from("sink"),
        sink_port_id: PortId::from("in"),
        value_kind: KindId::from("value/fixture"),
        resource: None,
        track: Default::default(),
        temporal: PortTemporal::Flow { closes: true },
        pressure_policy: Default::default(),
        selected_line: Some(line.clone()),
        admitted_lines: vec![line],
        item_capacity: 1,
        byte_capacity: 64,
        abnormal_kind: None,
    }
}
fn bind(connection: &PlannedConnection) -> Result<SessionBinding, WireError> {
    SessionBinding::from_planned_connection(
        PlanId::from("plan"),
        FragmentId::from("source-fragment"),
        FragmentId::from("sink-fragment"),
        connection,
    )
}
#[test]
fn machine_local_owned_line_preserves_scope_security_and_exact_binding() {
    let connection = connection();
    let binding = bind(&connection).unwrap();
    assert_eq!(binding.attachment.contract.scope, LineScope::Machine);
    assert_eq!(
        binding.attachment.contract.security,
        LineSecurity::ProcessBoundary
    );
    assert_eq!(
        binding.attachment.link_binding_id.as_str(),
        "binding/owned-machine"
    );
}
#[test]
fn unsupported_local_shapes_and_unsealed_identity_still_refuse() {
    for contract in [
        LineContract {
            duplex: LineDuplex::HalfDuplex,
            ..connection().selected_line.unwrap().contract
        },
        LineContract {
            ordering: LineOrdering::Unordered,
            ..connection().selected_line.unwrap().contract
        },
    ] {
        let mut connection = connection();
        connection.selected_line.as_mut().unwrap().contract = contract;
        connection.admitted_lines[0].contract = contract;
        assert!(bind(&connection).is_err());
    }
    let mut connection = connection();
    connection
        .selected_line
        .as_mut()
        .unwrap()
        .binding
        .binding_id = LinkBindingId::from("binding/foreign");
    assert!(bind(&connection).is_err());
}
