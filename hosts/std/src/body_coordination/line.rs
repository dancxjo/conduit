use super::endpoint::CoordinationEndpoint;
use super::receipt::CoordinationFailure;
use crate::websocket::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener};
use conduit_runtime::lowering::RemoteCordDirection;
use conduit_std_catalog::{BODY_COORDINATION_MAXIMUM_FRAME_BYTES, MAX_TEXT_BYTES};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionMessage, SessionTerminalDisposition,
    WireError,
};
use std::net::SocketAddr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoordinationLineError {
    Classified(CoordinationFailure),
    Contract(String),
}

enum ReceivedMessage {
    Hello,
    Ready,
    Offered {
        sequence: u64,
        payload: Vec<u8>,
    },
    Accepted {
        sequence: u64,
    },
    Delivered {
        sequence: u64,
    },
    Pressure,
    InputClosed {
        final_sequence: u64,
    },
    Terminal {
        disposition: SessionTerminalDisposition,
        final_sequence: u64,
    },
    Other,
}

impl core::fmt::Display for CoordinationLineError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CoordinationLineError {}

pub fn run_forebrain(
    endpoint: &mut CoordinationEndpoint,
    outbound_listener: NativeWebSocketListener,
    motherbrain_return: SocketAddr,
) -> Result<(), CoordinationLineError> {
    let mut outbound = outbound_listener.accept().map_err(peer_absent)?;
    let mut returned = NativeWebSocketLine::connect(
        motherbrain_return,
        &format!("ws://{motherbrain_return}/conduit"),
        BODY_COORDINATION_MAXIMUM_FRAME_BYTES,
    )
    .map_err(peer_absent)?;
    source_handshake(endpoint, &mut outbound)?;
    sink_handshake(endpoint, &mut returned)?;
    send_offer(endpoint, &mut outbound)?;
    receive_offer(endpoint, &mut returned)?;
    close_source(endpoint, &mut outbound)?;
    close_sink(endpoint, &mut returned)?;
    endpoint.finish().map_err(CoordinationLineError::Contract)?;
    outbound.close().map_err(transport)?;
    returned.close().map_err(transport)
}

pub fn run_motherbrain(
    endpoint: &mut CoordinationEndpoint,
    return_listener: NativeWebSocketListener,
    forebrain_outbound: SocketAddr,
) -> Result<(), CoordinationLineError> {
    let mut inbound = NativeWebSocketLine::connect(
        forebrain_outbound,
        &format!("ws://{forebrain_outbound}/conduit"),
        BODY_COORDINATION_MAXIMUM_FRAME_BYTES,
    )
    .map_err(peer_absent)?;
    let mut returned = return_listener.accept().map_err(peer_absent)?;
    sink_handshake(endpoint, &mut inbound)?;
    source_handshake(endpoint, &mut returned)?;
    receive_offer(endpoint, &mut inbound)?;
    send_offer(endpoint, &mut returned)?;
    close_sink(endpoint, &mut inbound)?;
    close_source(endpoint, &mut returned)?;
    endpoint.finish().map_err(CoordinationLineError::Contract)?;
    inbound.close().map_err(transport)?;
    returned.close().map_err(transport)
}

fn source_handshake(
    endpoint: &mut CoordinationEndpoint,
    line: &mut NativeWebSocketLine,
) -> Result<(), CoordinationLineError> {
    let direction = RemoteCordDirection::Egress;
    let binding = endpoint.binding(direction).clone();
    send(endpoint, direction, line, binding.hello_frame().message)?;
    if !matches!(receive(endpoint, direction, line)?, ReceivedMessage::Hello) {
        return Err(CoordinationLineError::Classified(
            CoordinationFailure::WrongBoot,
        ));
    }
    send(endpoint, direction, line, SessionMessage::Ready)?;
    if !matches!(receive(endpoint, direction, line)?, ReceivedMessage::Ready)
        || !endpoint.session_mut(direction).is_active()
    {
        return Err(CoordinationLineError::Contract(
            "source session did not reach Ready".into(),
        ));
    }
    Ok(())
}

fn sink_handshake(
    endpoint: &mut CoordinationEndpoint,
    line: &mut NativeWebSocketLine,
) -> Result<(), CoordinationLineError> {
    let direction = RemoteCordDirection::Ingress;
    let binding = endpoint.binding(direction).clone();
    if !matches!(receive(endpoint, direction, line)?, ReceivedMessage::Hello) {
        return Err(CoordinationLineError::Classified(
            CoordinationFailure::WrongBoot,
        ));
    }
    send(endpoint, direction, line, binding.hello_frame().message)?;
    if !matches!(receive(endpoint, direction, line)?, ReceivedMessage::Ready) {
        return Err(CoordinationLineError::Contract(
            "sink expected Ready".into(),
        ));
    }
    send(endpoint, direction, line, SessionMessage::Ready)?;
    if !endpoint.session_mut(direction).is_active() {
        return Err(CoordinationLineError::Contract(
            "sink session did not reach Ready".into(),
        ));
    }
    Ok(())
}

fn send_offer(
    endpoint: &mut CoordinationEndpoint,
    line: &mut NativeWebSocketLine,
) -> Result<(), CoordinationLineError> {
    let offer = endpoint
        .next_offer()
        .map_err(CoordinationLineError::Contract)?;
    send(
        endpoint,
        RemoteCordDirection::Egress,
        line,
        SessionMessage::Offered {
            sequence: offer.sequence,
            payload: &offer.bytes,
        },
    )?;
    match receive(endpoint, RemoteCordDirection::Egress, line)? {
        ReceivedMessage::Accepted { sequence } if sequence == offer.sequence => {
            endpoint
                .accept_offer(sequence)
                .map_err(CoordinationLineError::Contract)?;
        }
        ReceivedMessage::Pressure => {
            return Err(CoordinationLineError::Classified(
                CoordinationFailure::Pressure,
            ));
        }
        _ => {
            return Err(CoordinationLineError::Classified(
                CoordinationFailure::LossBeforeAcceptance,
            ));
        }
    }
    match receive(endpoint, RemoteCordDirection::Egress, line)? {
        ReceivedMessage::Delivered { sequence } if sequence == offer.sequence => endpoint
            .deliver_offer(sequence)
            .map_err(CoordinationLineError::Contract),
        _ => Err(CoordinationLineError::Classified(
            CoordinationFailure::LossAfterAcceptance,
        )),
    }
}

fn receive_offer(
    endpoint: &mut CoordinationEndpoint,
    line: &mut NativeWebSocketLine,
) -> Result<(), CoordinationLineError> {
    let direction = RemoteCordDirection::Ingress;
    let ReceivedMessage::Offered { sequence, payload } = receive(endpoint, direction, line)? else {
        return Err(CoordinationLineError::Classified(
            CoordinationFailure::Malformed,
        ));
    };
    endpoint.admit_input(sequence, &payload).map_err(|detail| {
        if detail.to_ascii_lowercase().contains("duplicate") {
            CoordinationLineError::Classified(CoordinationFailure::Duplicate)
        } else {
            CoordinationLineError::Contract(detail)
        }
    })?;
    send(
        endpoint,
        direction,
        line,
        SessionMessage::Accepted { sequence },
    )?;
    send(
        endpoint,
        direction,
        line,
        SessionMessage::Delivered { sequence },
    )
}

fn close_source(
    endpoint: &mut CoordinationEndpoint,
    line: &mut NativeWebSocketLine,
) -> Result<(), CoordinationLineError> {
    let direction = RemoteCordDirection::Egress;
    send(
        endpoint,
        direction,
        line,
        SessionMessage::InputClosed { final_sequence: 1 },
    )?;
    send(
        endpoint,
        direction,
        line,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1,
        },
    )?;
    match receive(endpoint, direction, line)? {
        ReceivedMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1,
        } if endpoint.session_mut(direction).is_terminal() => Ok(()),
        _ => Err(CoordinationLineError::Classified(
            CoordinationFailure::TerminalDisagreement,
        )),
    }
}

fn close_sink(
    endpoint: &mut CoordinationEndpoint,
    line: &mut NativeWebSocketLine,
) -> Result<(), CoordinationLineError> {
    let direction = RemoteCordDirection::Ingress;
    if !matches!(
        receive(endpoint, direction, line)?,
        ReceivedMessage::InputClosed { final_sequence: 1 }
    ) {
        return Err(CoordinationLineError::Classified(
            CoordinationFailure::TerminalDisagreement,
        ));
    }
    endpoint
        .close_input()
        .map_err(CoordinationLineError::Contract)?;
    if !matches!(
        receive(endpoint, direction, line)?,
        ReceivedMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1,
        }
    ) {
        return Err(CoordinationLineError::Classified(
            CoordinationFailure::TerminalDisagreement,
        ));
    }
    send(
        endpoint,
        direction,
        line,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1,
        },
    )?;
    if endpoint.session_mut(direction).is_terminal() {
        Ok(())
    } else {
        Err(CoordinationLineError::Classified(
            CoordinationFailure::TerminalDisagreement,
        ))
    }
}

fn send(
    endpoint: &mut CoordinationEndpoint,
    direction: RemoteCordDirection,
    line: &mut NativeWebSocketLine,
    message: SessionMessage<'_>,
) -> Result<(), CoordinationLineError> {
    let binding = endpoint.binding(direction).clone();
    let frame = binding.frame(message);
    endpoint
        .session_mut(direction)
        .admit_outbound(frame)
        .map_err(classify_wire_error)?;
    let mut output = [0_u8; BODY_COORDINATION_MAXIMUM_FRAME_BYTES as usize];
    let length = encode_session_frame_into(
        frame,
        &mut output,
        MAX_TEXT_BYTES,
        BODY_COORDINATION_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|_| CoordinationLineError::Classified(CoordinationFailure::Oversized))?;
    line.send_binary(&output[..length]).map_err(transport)
}

fn receive(
    endpoint: &mut CoordinationEndpoint,
    direction: RemoteCordDirection,
    line: &mut NativeWebSocketLine,
) -> Result<ReceivedMessage, CoordinationLineError> {
    let mut input = [0_u8; BODY_COORDINATION_MAXIMUM_FRAME_BYTES as usize];
    let length = line.receive_binary(&mut input).map_err(transport)?;
    let decoded = decode_session_frame(
        &input[..length],
        MAX_TEXT_BYTES,
        BODY_COORDINATION_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|_| CoordinationLineError::Classified(CoordinationFailure::Malformed))?;
    endpoint
        .session_mut(direction)
        .admit_inbound(decoded)
        .map_err(classify_wire_error)?;
    Ok(match decoded.message {
        SessionMessage::Hello(_) => ReceivedMessage::Hello,
        SessionMessage::Ready => ReceivedMessage::Ready,
        SessionMessage::Offered { sequence, payload } => ReceivedMessage::Offered {
            sequence,
            payload: payload.to_vec(),
        },
        SessionMessage::Accepted { sequence } => ReceivedMessage::Accepted { sequence },
        SessionMessage::Delivered { sequence } => ReceivedMessage::Delivered { sequence },
        SessionMessage::Pressure { .. } => ReceivedMessage::Pressure,
        SessionMessage::InputClosed { final_sequence } => {
            ReceivedMessage::InputClosed { final_sequence }
        }
        SessionMessage::Terminal {
            disposition,
            final_sequence,
        } => ReceivedMessage::Terminal {
            disposition,
            final_sequence,
        },
        _ => ReceivedMessage::Other,
    })
}

fn peer_absent(error: NativeWebSocketError) -> CoordinationLineError {
    match error {
        NativeWebSocketError::Protocol | NativeWebSocketError::Handshake => {
            CoordinationLineError::Classified(CoordinationFailure::WrongBoot)
        }
        _ => CoordinationLineError::Classified(CoordinationFailure::PeerAbsent),
    }
}

fn transport(error: NativeWebSocketError) -> CoordinationLineError {
    match error {
        NativeWebSocketError::OversizedMessage | NativeWebSocketError::OutputTooSmall => {
            CoordinationLineError::Classified(CoordinationFailure::Oversized)
        }
        NativeWebSocketError::TextMessageRejected | NativeWebSocketError::Protocol => {
            CoordinationLineError::Classified(CoordinationFailure::Malformed)
        }
        NativeWebSocketError::Disconnected | NativeWebSocketError::Transport(_) => {
            CoordinationLineError::Classified(CoordinationFailure::PeerAbsent)
        }
        _ => CoordinationLineError::Contract(format!("{error:?}")),
    }
}

pub(super) fn classify_wire_error(error: WireError) -> CoordinationLineError {
    let failure = match error {
        WireError::BootMismatch
        | WireError::PlanMismatch
        | WireError::ConnectionMismatch
        | WireError::SessionEpochMismatch
        | WireError::ValueContractMismatch
        | WireError::InvalidSession => CoordinationFailure::WrongBoot,
        WireError::DuplicateFrame => CoordinationFailure::Duplicate,
        WireError::OversizedFrame
        | WireError::OversizedPayload
        | WireError::OutputTooSmall
        | WireError::IdentifierTooLong => CoordinationFailure::Oversized,
        WireError::LateFrame => CoordinationFailure::TerminalDisagreement,
        WireError::InvalidMagic
        | WireError::UnsupportedWireFormat
        | WireError::WrongProtocolVersion
        | WireError::TruncatedFrame
        | WireError::InvalidIdentifierEncoding
        | WireError::TrailingGarbage
        | WireError::InvalidMessageKind
        | WireError::InvalidBase
        | WireError::InvalidLimits
        | WireError::InvalidState
        | WireError::ReorderedFrame => CoordinationFailure::Malformed,
    };
    CoordinationLineError::Classified(failure)
}
