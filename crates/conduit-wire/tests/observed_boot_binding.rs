use conduit_core::{
    bind_active_play, BootId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId,
    FragmentId, HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_wire::{SessionBinding, SessionMachine, SessionRole, WireError};

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
        link_binding_id: LinkBindingId::from("link/exact"),
        provider: ConnectionProvider::UsbCdc,
        provider_instance_id: ConnectionProviderInstanceId::from("provider/exact"),
        source,
        sink,
        value_kind: KindId::from("value/signal"),
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 9,
            maximum_buffered_bytes: 9,
            maximum_frame_bytes: 2_048,
        },
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
    assert_eq!(observed.link_binding_id, planned.link_binding_id);
    assert_eq!(observed.provider, planned.provider);
    assert_eq!(observed.provider_instance_id, planned.provider_instance_id);
    assert_eq!(observed.source.host_id, planned.source.host_id);
    assert_eq!(observed.source.endpoint_id, planned.source.endpoint_id);
    assert_eq!(observed.sink.host_id, planned.sink.host_id);
    assert_eq!(observed.sink.endpoint_id, planned.sink.endpoint_id);
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
        Err(WireError::InvalidSession)
    );
}
