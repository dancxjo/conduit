//! Exact generated Signal execution over the planned WebSocket Line.

use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionMachine, SessionMessage,
};
use embassy_net::tcp::TcpSocket;

use crate::continuable_signal::ContinuableSignalSink;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb_link::UsbLinkError;
use crate::websocket_transport::{WebSocketTransport, WebSocketTransportError};

pub async fn run(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    control: &mut cyw43::Control<'_>,
    sign: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
    state: &mut ContinuableSignalSink,
) -> Result<(), WebSocketTransportError> {
    let binding = &state.binding;
    let identity = state.identity;
    let mut bytes = [0_u8; conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES as usize];

    let hello = receive(socket, transport, &mut bytes).await?;
    state
        .machine
        .admit_inbound(hello)
        .map_err(|_| WebSocketTransportError::Frame)?;
    send(
        socket,
        transport,
        &mut state.machine,
        binding.hello_frame(),
        &mut bytes,
    )
    .await?;
    let ready = receive(socket, transport, &mut bytes).await?;
    state
        .machine
        .admit_inbound(ready)
        .map_err(|_| WebSocketTransportError::Frame)?;
    send(
        socket,
        transport,
        &mut state.machine,
        binding.frame(SessionMessage::Ready),
        &mut bytes,
    )
    .await?;
    if !state.machine.is_active() {
        return Err(WebSocketTransportError::Frame);
    }

    loop {
        let frame = receive(socket, transport, &mut bytes).await?;
        match frame.message {
            SessionMessage::Offered { sequence, payload } => {
                state
                    .machine
                    .admit_inbound(frame)
                    .map_err(|_| WebSocketTransportError::Frame)?;
                match state
                    .kernel
                    .admit(sequence, payload)
                    .map_err(kernel_error)?
                {
                    RemoteIngressOutcome::Full { .. } => {
                        send(
                            socket,
                            transport,
                            &mut state.machine,
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
                    &mut state.machine,
                    binding.frame(SessionMessage::Accepted { sequence }),
                    &mut bytes,
                )
                .await?;
                state
                    .kernel
                    .present_accepted(sequence, control, sign, runtime)
                    .await
                    .map_err(kernel_error)?;
                send(
                    socket,
                    transport,
                    &mut state.machine,
                    binding.frame(SessionMessage::Delivered { sequence }),
                    &mut bytes,
                )
                .await?;
            }
            SessionMessage::InputClosed { final_sequence } => {
                state
                    .machine
                    .admit_inbound(frame)
                    .map_err(|_| WebSocketTransportError::Frame)?;
                state
                    .kernel
                    .close_and_complete(final_sequence)
                    .map_err(kernel_error)?;
            }
            SessionMessage::Terminal {
                disposition,
                final_sequence,
            } => {
                state
                    .machine
                    .admit_inbound(frame)
                    .map_err(|_| WebSocketTransportError::Frame)?;
                send(
                    socket,
                    transport,
                    &mut state.machine,
                    binding.frame(SessionMessage::Terminal {
                        disposition,
                        final_sequence,
                    }),
                    &mut bytes,
                )
                .await?;
            }
            SessionMessage::Cancelled { .. } | SessionMessage::Failed { .. } => {
                state
                    .machine
                    .admit_inbound(frame)
                    .map_err(|_| WebSocketTransportError::Frame)?;
                state.kernel.cancel().map_err(kernel_error)?;
                return Err(WebSocketTransportError::Frame);
            }
            _ => state
                .machine
                .admit_inbound(frame)
                .map_err(|_| WebSocketTransportError::Frame)?,
        }
        if state.machine.is_terminal() {
            sign.write_terminal(true, identity.terminal(), runtime)
                .await
                .map_err(|_| WebSocketTransportError::Disconnected)?;
            return Ok(());
        }
    }
}

async fn receive<'a>(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    bytes: &'a mut [u8],
) -> Result<conduit_wire::SessionFrame<'a>, WebSocketTransportError> {
    let len = transport.receive_binary(socket, bytes).await?;
    decode_session_frame(&bytes[..len], 1024, conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES)
        .map_err(|_| WebSocketTransportError::Frame)
}

async fn send(
    socket: &mut TcpSocket<'_>,
    transport: &mut WebSocketTransport,
    machine: &mut SessionMachine,
    frame: conduit_wire::SessionFrame<'_>,
    bytes: &mut [u8],
) -> Result<(), WebSocketTransportError> {
    machine
        .admit_outbound(frame)
        .map_err(|_| WebSocketTransportError::Frame)?;
    let len = encode_session_frame_into(frame, bytes, 1024, conduit_r1_network_conformance::R1_MAXIMUM_FRAME_BYTES)
        .map_err(|_| WebSocketTransportError::Frame)?;
    transport.send_binary(socket, &bytes[..len]).await
}

fn kernel_error(_: UsbLinkError) -> WebSocketTransportError {
    WebSocketTransportError::Frame
}
