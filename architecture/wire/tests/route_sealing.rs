use conduit_core::{
    process_owned_line_offer, AdmittedLine, BaseImplementationId, BootId, ConnectionId, FragmentId,
    HostAdvertisement, HostId, HostProfileId, KindId, OfferGeneration, PlanId, PlannedConnection,
    PortId, PortTemporal, PROTOCOL_VERSION,
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

fn connection_with_routes(selected: AdmittedLine, lines: Vec<AdmittedLine>) -> PlannedConnection {
    PlannedConnection {
        connection_id: ConnectionId::from("connection"),
        source_placement_id: conduit_core::PlacementId::from("source-placement"),
        source_port_id: PortId::from("out"),
        sink_placement_id: conduit_core::PlacementId::from("sink-placement"),
        sink_port_id: PortId::from("in"),
        value_kind: KindId::from("value"),
        temporal: PortTemporal::Flow { closes: true },
        selected_line: Some(selected),
        admitted_lines: lines,
        item_capacity: 1,
        byte_capacity: 64,
    }
}

#[test]
fn session_rejects_selected_link_outside_sealed_candidates() {
    let source = host("source");
    let sink = host("sink");
    let sealed = process_owned_line_offer(
        "line/sealed",
        "sealed",
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
        "usb/0",
        &source,
        &sink,
        1,
        64,
    );
    let unsealed = process_owned_line_offer(
        "line/unsealed",
        "unsealed",
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
        "usb/1",
        &source,
        &sink,
        1,
        64,
    );
    let connection = connection_with_routes((&unsealed).into(), vec![(&sealed).into()]);

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
fn two_sealed_lines_share_one_cord_identity_but_keep_exact_attachments() {
    let source = host("source");
    let sink = host("sink");
    let mut usb = process_owned_line_offer(
        "line/usb",
        "usb",
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
        "usb/0",
        &source,
        &sink,
        1,
        64,
    );
    let mut websocket = process_owned_line_offer(
        "line/websocket",
        "websocket",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "websocket/0",
        &source,
        &sink,
        1,
        64,
    );
    usb.binding.limits.maximum_frame_bytes = 2_048;
    websocket.binding.limits.maximum_frame_bytes = 2_048;
    usb.contract = remote_contract(conduit_core::LineTrafficShape::ByteStream);
    websocket.contract = remote_contract(conduit_core::LineTrafficShape::Message);
    let usb_admitted: AdmittedLine = (&usb).into();
    let websocket_admitted: AdmittedLine = (&websocket).into();
    let connection = connection_with_routes(
        usb_admitted.clone(),
        vec![usb_admitted.clone(), websocket_admitted.clone()],
    );
    let make = |line| {
        SessionBinding::from_planned_connection_with_line(
            PlanId::from("plan"),
            FragmentId::from("source-fragment"),
            FragmentId::from("sink-fragment"),
            &connection,
            line,
        )
        .expect("sealed ready attachment")
    };
    let usb_session = make(&usb_admitted);
    let websocket_session = make(&websocket_admitted);

    assert_eq!(usb_session.identity(), websocket_session.identity());
    assert_ne!(usb_session.attachment, websocket_session.attachment);
    assert_eq!(
        usb_session.attachment.base,
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
    );
    assert_eq!(
        websocket_session.attachment.base,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1")
    );

    assert_ne!(websocket.availability, usb.availability);

    let mut mismatched = usb_session;
    mismatched.attachment.sink_boot_id = BootId::from("different-boot");
    assert_eq!(mismatched.validate(), Err(WireError::InvalidSession));
}

fn remote_contract(traffic_shape: conduit_core::LineTrafficShape) -> conduit_core::LineContract {
    conduit_core::LineContract {
        scope: conduit_core::LineScope::LocalNetwork,
        traffic_shape,
        duplex: conduit_core::LineDuplex::FullDuplex,
        ordering: conduit_core::LineOrdering::Ordered,
        reliability: conduit_core::LineReliability::Reliable,
        continuation: conduit_core::LineContinuation::None,
        security: conduit_core::LineSecurity::PlaintextNetwork,
    }
}
