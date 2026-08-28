//! Current `conduit-wire` Hello/Ready handshake over the Pico WebSocket Base.

use conduit_core::BootId;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionMachine, SessionMessage, SessionRole,
};
use embassy_net::tcp::TcpSocket;

use crate::receipts::RuntimeTranscriptIdentity;
use crate::websocket_transport::{WebSocketTransport, WebSocketTransportError};

pub async fn accept_probe(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), WebSocketTransportError> {
    let binding = conduit_r1_network_conformance::r1_websocket_probe_binding(BootId::from(runtime.boot_id()));
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink)
        .map_err(|_| WebSocketTransportError::Frame)?;
    let mut encoded = [0_u8; conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES as usize];
    let mut decoded = [0_u8; conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES as usize];

    let length = transport.receive_binary(socket, &mut encoded).await?;
    let hello = decode_session_frame(
        &encoded[..length],
        conduit_r1_network_conformance::R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
        conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|_| WebSocketTransportError::Frame)?;
    machine
        .admit_inbound(hello)
        .map_err(|_| WebSocketTransportError::Frame)?;
    send(
        socket,
        transport,
        &mut machine,
        binding.hello_frame(),
        &mut decoded,
    )
    .await?;

    let length = transport.receive_binary(socket, &mut encoded).await?;
    let ready = decode_session_frame(
        &encoded[..length],
        conduit_r1_network_conformance::R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
        conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|_| WebSocketTransportError::Frame)?;
    machine
        .admit_inbound(ready)
        .map_err(|_| WebSocketTransportError::Frame)?;
    let response = binding.frame(SessionMessage::Ready);
    send(socket, transport, &mut machine, response, &mut decoded).await?;
    if !machine.is_active() {
        return Err(WebSocketTransportError::Frame);
    }
    Ok(())
}

async fn send(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    machine: &mut SessionMachine,
    frame: conduit_wire::SessionFrame<'_>,
    output: &mut [u8],
) -> Result<(), WebSocketTransportError> {
    machine
        .admit_outbound(frame)
        .map_err(|_| WebSocketTransportError::Frame)?;
    let length = encode_session_frame_into(
        frame,
        output,
        conduit_r1_network_conformance::R1_ROUTE_PROBE_MAXIMUM_PAYLOAD_BYTES,
        conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|_| WebSocketTransportError::Frame)?;
    transport.send_binary(socket, &output[..length]).await
}
