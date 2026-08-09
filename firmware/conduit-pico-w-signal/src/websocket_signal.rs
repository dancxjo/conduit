//! Exact generated Signal execution over the planned WebSocket Line.

use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, RouteAttachment, SessionBinding,
    SessionEndpointIdentity, SessionLimits, SessionMachine, SessionMessage, SessionRole,
};
use embassy_net::tcp::TcpSocket;

use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::remote_kernel::RemoteSignalKernel;
use crate::usb_link::UsbLinkError;
use crate::websocket_transport::{WebSocketTransport, WebSocketTransportError};

pub async fn run(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    control: &mut cyw43::Control<'_>,
    clue: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), WebSocketTransportError> {
    let binding = binding(runtime).map_err(|_| WebSocketTransportError::Frame)?;
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink)
        .map_err(|_| WebSocketTransportError::Frame)?;
    let mut kernel = RemoteSignalKernel::new().map_err(|_| WebSocketTransportError::Frame)?;
    let mut bytes = [0_u8; conduit_net::R1_MAXIMUM_FRAME_BYTES as usize];

    let hello = receive(socket, transport, &mut bytes).await?;
    machine.admit_inbound(hello).map_err(|_| WebSocketTransportError::Frame)?;
    send(socket, transport, &mut machine, binding.hello_frame(), &mut bytes).await?;
    let ready = receive(socket, transport, &mut bytes).await?;
    machine.admit_inbound(ready).map_err(|_| WebSocketTransportError::Frame)?;
    send(
        socket,
        transport,
        &mut machine,
        binding.frame(SessionMessage::Ready),
        &mut bytes,
    )
    .await?;
    if !machine.is_active() {
        return Err(WebSocketTransportError::Frame);
    }

    loop {
        let frame = receive(socket, transport, &mut bytes).await?;
        match frame.message {
            SessionMessage::Offered { sequence, payload } => {
                machine.admit_inbound(frame).map_err(|_| WebSocketTransportError::Frame)?;
                match kernel.admit(sequence, payload).map_err(kernel_error)? {
                    RemoteIngressOutcome::Full { .. } => {
                        send(
                            socket,
                            transport,
                            &mut machine,
                            binding.frame(SessionMessage::Pressure { sequence }),
                            &mut bytes,
                        )
                        .await?;
                        continue;
                    }
                    RemoteIngressOutcome::Accepted { .. } => {}
                }
                send(
                    socket,
                    transport,
                    &mut machine,
                    binding.frame(SessionMessage::Accepted { sequence }),
                    &mut bytes,
                )
                .await?;
                kernel
                    .present_accepted(sequence, control, clue, runtime)
                    .await
                    .map_err(kernel_error)?;
                send(
                    socket,
                    transport,
                    &mut machine,
                    binding.frame(SessionMessage::Delivered { sequence }),
                    &mut bytes,
                )
                .await?;
            }
            SessionMessage::InputClosed { final_sequence } => {
                machine.admit_inbound(frame).map_err(|_| WebSocketTransportError::Frame)?;
                kernel.close_and_complete(final_sequence).map_err(kernel_error)?;
            }
            SessionMessage::Terminal { disposition, final_sequence } => {
                machine.admit_inbound(frame).map_err(|_| WebSocketTransportError::Frame)?;
                send(
                    socket,
                    transport,
                    &mut machine,
                    binding.frame(SessionMessage::Terminal { disposition, final_sequence }),
                    &mut bytes,
                )
                .await?;
            }
            SessionMessage::Cancelled { .. } | SessionMessage::Failed { .. } => {
                machine.admit_inbound(frame).map_err(|_| WebSocketTransportError::Frame)?;
                kernel.cancel().map_err(kernel_error)?;
                return Err(WebSocketTransportError::Frame);
            }
            _ => machine.admit_inbound(frame).map_err(|_| WebSocketTransportError::Frame)?,
        }
        if machine.is_terminal() {
            clue
                .write_terminal(true, crate::kernel::terminal_identity(), runtime)
                .await
                .map_err(|_| WebSocketTransportError::Disconnected)?;
            return Ok(());
        }
    }
}

pub(crate) fn binding(
    runtime: &RuntimeTranscriptIdentity,
) -> Result<SessionBinding, UsbLinkError> {
    let endpoint = crate::signal_image::generated_remote_endpoint()
        .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    let base = ConnectionBase::from_canonical_code(endpoint.base_code)
        .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    if base != ConnectionBase::WebSocket
        || endpoint.local_host != crate::signal_image::HOST_ID
        || endpoint.local_boot != crate::signal_image::BOOT_ID
        || endpoint.sink_fragment_id != crate::signal_image::FRAGMENT_ID
    {
        return Err(UsbLinkError::InvalidGeneratedEndpoint);
    }
    let plan = PlanId::from(crate::signal_image::PLAN_ID);
    let source_host = HostId::from(endpoint.peer_host);
    let source_boot = BootId::from(endpoint.peer_boot);
    let sink_host = HostId::from(endpoint.local_host);
    let sink_boot = BootId::from(endpoint.local_boot);
    SessionBinding {
        protocol_version: 1,
        plan_id: plan.clone(),
        source_fragment_id: FragmentId::from(endpoint.source_fragment_id),
        sink_fragment_id: FragmentId::from(endpoint.sink_fragment_id),
        source_active_play_id: bind_active_play(&plan, &source_host, &source_boot, 0).active_play_id,
        sink_active_play_id: bind_active_play(&plan, &sink_host, &sink_boot, 0).active_play_id,
        connection_id: ConnectionId::from(endpoint.connection_id),
        source: SessionEndpointIdentity { host_id: source_host.clone(), boot_id: source_boot.clone() },
        sink: SessionEndpointIdentity { host_id: sink_host.clone(), boot_id: sink_boot.clone() },
        value_kind: KindId::from(endpoint.value_kind),
        limits: SessionLimits {
            maximum_in_flight_items: endpoint.maximum_in_flight_items,
            maximum_payload_bytes: endpoint.maximum_payload_bytes,
            maximum_buffered_bytes: endpoint.maximum_buffered_bytes,
        },
        attachment: RouteAttachment {
            link_binding_id: LinkBindingId::from(endpoint.link_binding_id),
            base,
            base_instance_id: ConnectionBaseInstanceId::from(endpoint.base_instance_id),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from(endpoint.peer_endpoint),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from(endpoint.local_endpoint),
            limits: LinkLimits {
                maximum_in_flight_items: endpoint.maximum_in_flight_items,
                maximum_payload_bytes: endpoint.maximum_payload_bytes,
                maximum_buffered_bytes: endpoint.maximum_buffered_bytes,
                maximum_frame_bytes: endpoint.maximum_frame_bytes,
            },
        },
    }
    .with_observed_boots(BootId::from(endpoint.peer_boot), BootId::from(runtime.boot_id()))
    .map_err(UsbLinkError::Codec)
}

async fn receive<'a>(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    bytes: &'a mut [u8],
) -> Result<conduit_wire::SessionFrame<'a>, WebSocketTransportError> {
    let len = transport.receive_binary(socket, bytes).await?;
    decode_session_frame(&bytes[..len], 1024, conduit_net::R1_MAXIMUM_FRAME_BYTES)
        .map_err(|_| WebSocketTransportError::Frame)
}

async fn send(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    machine: &mut SessionMachine,
    frame: conduit_wire::SessionFrame<'_>,
    bytes: &mut [u8],
) -> Result<(), WebSocketTransportError> {
    machine.admit_outbound(frame).map_err(|_| WebSocketTransportError::Frame)?;
    let len = encode_session_frame_into(frame, bytes, 1024, conduit_net::R1_MAXIMUM_FRAME_BYTES)
        .map_err(|_| WebSocketTransportError::Frame)?;
    transport.send_binary(socket, &bytes[..len]).await
}

fn kernel_error(_: UsbLinkError) -> WebSocketTransportError {
    WebSocketTransportError::Frame
}
