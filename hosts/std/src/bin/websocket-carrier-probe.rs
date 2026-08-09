use std::io::Write;

use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
    PROTOCOL_VERSION,
};
use conduit_std_host::websocket::NativeWebSocketListener;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, LineAttachment, SessionBinding,
    SessionEndpointIdentity, SessionLimits, SessionMachine, SessionMessage, SessionRole,
};

const MAXIMUM_FRAME_BYTES: u32 = 1_024;
const MAXIMUM_PAYLOAD_BYTES: u32 = 16;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = NativeWebSocketListener::bind_loopback(MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("bind: {error:?}"))?;
    println!(
        "{}",
        listener.url().map_err(|error| format!("url: {error:?}"))?
    );
    std::io::stdout().flush()?;

    let wire_binding = binding();
    let mut machine = SessionMachine::new(wire_binding.clone(), SessionRole::Source)
        .map_err(|error| format!("session: {error:?}"))?;
    let mut carrier = listener
        .accept()
        .map_err(|error| format!("accept: {error:?}"))?;
    let mut outbound = [0_u8; MAXIMUM_FRAME_BYTES as usize];
    let mut inbound = [0_u8; MAXIMUM_FRAME_BYTES as usize];

    let hello = wire_binding.hello_frame();
    machine
        .admit_outbound(hello)
        .map_err(|error| format!("hello outbound: {error:?}"))?;
    let length = encode_session_frame_into(
        hello,
        &mut outbound,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("hello encode: {error:?}"))?;
    carrier
        .send_binary(&outbound[..length])
        .map_err(|error| format!("hello send: {error:?}"))?;
    let length = carrier
        .receive_binary(&mut inbound)
        .map_err(|error| format!("hello receive: {error:?}"))?;
    let echoed = decode_session_frame(
        &inbound[..length],
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("hello decode: {error:?}"))?;
    machine
        .admit_inbound(echoed)
        .map_err(|error| format!("hello inbound: {error:?}"))?;

    let ready = wire_binding.frame(SessionMessage::Ready);
    machine
        .admit_outbound(ready)
        .map_err(|error| format!("ready outbound: {error:?}"))?;
    let length = encode_session_frame_into(
        ready,
        &mut outbound,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("ready encode: {error:?}"))?;
    carrier
        .send_binary(&outbound[..length])
        .map_err(|error| format!("ready send: {error:?}"))?;
    let length = carrier
        .receive_binary(&mut inbound)
        .map_err(|error| format!("ready receive: {error:?}"))?;
    let echoed = decode_session_frame(
        &inbound[..length],
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("ready decode: {error:?}"))?;
    machine
        .admit_inbound(echoed)
        .map_err(|error| format!("ready inbound: {error:?}"))?;
    if !machine.is_active() {
        return Err("both exact handshakes were not ready".into());
    }
    println!("ready protocol=1 base=websocket frame_limit={MAXIMUM_FRAME_BYTES}");
    carrier
        .close()
        .map_err(|error| format!("close: {error:?}"))?;
    Ok(())
}

fn binding() -> SessionBinding {
    let plan_id = PlanId::from("probe/plan");
    let source = LinkEndpoint {
        host_id: HostId::from("probe/source-host"),
        boot_id: BootId::from("probe/source-boot"),
        endpoint_id: LinkEndpointId::from("probe/source-endpoint"),
    };
    let sink = LinkEndpoint {
        host_id: HostId::from("probe/sink-host"),
        boot_id: BootId::from("probe/sink-boot"),
        endpoint_id: LinkEndpointId::from("probe/sink-endpoint"),
    };
    let source_active_play_id =
        bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0).active_play_id;
    let sink_active_play_id =
        bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0).active_play_id;
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        plan_id,
        source_fragment_id: FragmentId::from("probe/source-fragment"),
        sink_fragment_id: FragmentId::from("probe/sink-fragment"),
        source_active_play_id,
        sink_active_play_id,
        connection_id: ConnectionId::from("probe/connection"),
        source: SessionEndpointIdentity {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        },
        value_kind: KindId::from("probe/value"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAXIMUM_PAYLOAD_BYTES,
            maximum_buffered_bytes: MAXIMUM_PAYLOAD_BYTES,
        },
        attachment: LineAttachment {
            line_id: "line/websocket-probe".into(),
            link_binding_id: LinkBindingId::from("probe/link"),
            base: ConnectionBase::WebSocket,
            base_instance_id: ConnectionBaseInstanceId::from("probe/websocket/instance"),
            source_host_id: source.host_id,
            source_boot_id: source.boot_id,
            source_endpoint_id: LinkEndpointId::from("probe/source-endpoint"),
            sink_host_id: sink.host_id,
            sink_boot_id: sink.boot_id,
            sink_endpoint_id: LinkEndpointId::from("probe/sink-endpoint"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: MAXIMUM_PAYLOAD_BYTES,
                maximum_buffered_bytes: MAXIMUM_PAYLOAD_BYTES,
                maximum_frame_bytes: MAXIMUM_FRAME_BYTES,
            },
        },
    }
}
