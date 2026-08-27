use conduit_core::{
    bind_active_play, BaseImplementationId, BaseInstanceId, BootId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_wire::{
    LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits, SessionMachine,
    SessionRole, WireError,
};

fn planned_binding() -> SessionBinding {
    let plan_id = PlanId::from("plan/exact");
    let source = LinkEndpoint {
        host_id: HostId::from("host/source"),
        boot_id: BootId::from("boot/source-planned"),
        endpoint_id: LinkEndpointId::from("endpoint/source"),
    };
    let sink = LinkEndpoint {
        host_id: HostId::from("host/sink"),
        boot_id: BootId::from("boot/sink-image"),
        endpoint_id: LinkEndpointId::from("endpoint/sink"),
    };
    SessionBinding {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        source_fragment_id: FragmentId::from("fragment/source"),
        sink_fragment_id: FragmentId::from("fragment/sink"),
        source_active_play_id: bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0)
            .active_play_id,
        connection_id: ConnectionId::from("connection/exact"),
        source: SessionEndpointIdentity {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        },
        value_kind: KindId::from("value/signal"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 9,
            maximum_buffered_bytes: 9,
        },
        attachment: LineAttachment {
            line_id: "line/session".into(),
            link_binding_id: LinkBindingId::from("link/exact"),
            base: BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
            contract: remote_session_contract(),
            base_instance_id: BaseInstanceId::from("base/exact"),
            source_host_id: source.host_id,
            source_boot_id: source.boot_id,
            source_endpoint_id: LinkEndpointId::from("endpoint/source"),
            sink_host_id: sink.host_id,
            sink_boot_id: sink.boot_id,
            sink_endpoint_id: LinkEndpointId::from("endpoint/sink"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 9,
                maximum_buffered_bytes: 9,
                maximum_frame_bytes: 2_048,
            },
        },
    }
}

fn remote_session_contract() -> conduit_core::LineContract {
    conduit_core::LineContract {
        scope: conduit_core::LineScope::LocalNetwork,
        traffic_shape: conduit_core::LineTrafficShape::Message,
        duplex: conduit_core::LineDuplex::FullDuplex,
        ordering: conduit_core::LineOrdering::Ordered,
        reliability: conduit_core::LineReliability::Reliable,
        continuation: conduit_core::LineContinuation::None,
        security: conduit_core::LineSecurity::PlaintextNetwork,
    }
}

#[test]
fn observed_boots_change_only_boot_scoped_session_facts() {
    let planned = planned_binding();
    let observed = planned
        .clone()
        .with_observed_boots(
            BootId::from("boot/source-runtime"),
            BootId::from("boot/sink-runtime"),
        )
        .expect("runtime binding");
    assert_eq!(observed.plan_id, planned.plan_id);
    assert_eq!(observed.source_fragment_id, planned.source_fragment_id);
    assert_eq!(observed.sink_fragment_id, planned.sink_fragment_id);
    assert_eq!(observed.connection_id, planned.connection_id);
    assert_eq!(
        observed.attachment.link_binding_id,
        planned.attachment.link_binding_id
    );
    assert_eq!(observed.attachment.base, planned.attachment.base);
    assert_eq!(
        observed.attachment.base_instance_id,
        planned.attachment.base_instance_id
    );
    assert_eq!(
        observed.attachment.source_endpoint_id,
        planned.attachment.source_endpoint_id
    );
    assert_eq!(
        observed.attachment.sink_endpoint_id,
        planned.attachment.sink_endpoint_id
    );
    assert_eq!(observed.attachment.limits, planned.attachment.limits);
    assert_eq!(observed.source.host_id, planned.source.host_id);
    assert_eq!(observed.sink.host_id, planned.sink.host_id);
    assert_eq!(observed.value_kind, planned.value_kind);
    assert_eq!(observed.limits, planned.limits);
    assert_ne!(observed.source.boot_id, planned.source.boot_id);
    assert_ne!(observed.sink.boot_id, planned.sink.boot_id);
    assert_ne!(
        observed.source_active_play_id,
        planned.source_active_play_id
    );
    assert_ne!(observed.sink_active_play_id, planned.sink_active_play_id);
}

#[test]
fn stale_planned_boot_frame_is_rejected_by_runtime_session() {
    let planned = planned_binding();
    let observed = planned
        .clone()
        .with_observed_boots(
            BootId::from("boot/source-runtime"),
            BootId::from("boot/sink-runtime"),
        )
        .expect("runtime binding");
    let mut machine = SessionMachine::new(observed, SessionRole::Sink).expect("session");
    assert_eq!(
        machine.admit_inbound(planned.hello_frame()),
        Err(WireError::BootMismatch)
    );
}
