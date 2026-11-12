//! Shared session message bodies; scope headers remain distinct.
use super::*;
pub(super) fn encode(
    writer: &mut Writer<'_>,
    message: SessionMessage<'_>,
    maximum_payload_bytes: u32,
) -> Result<(), WireError> {
    match message {
        SessionMessage::Hello(hello) => {
            writer.text(hello.line_id)?;
            writer.text(hello.link_binding_id)?;
            writer.text(hello.base)?;
            writer.text(hello.base_instance_id)?;
            encode_line_contract(writer, hello.contract)?;
            writer.text(hello.source_endpoint_id)?;
            writer.text(hello.sink_endpoint_id)?;
            writer.u16(hello.limits.maximum_in_flight_items)?;
            writer.u32(hello.limits.maximum_payload_bytes)?;
            writer.u32(hello.limits.maximum_buffered_bytes)?;
            writer.u32(hello.limits.maximum_frame_bytes)?;
        }
        SessionMessage::Ready => {}
        SessionMessage::Offered { sequence, payload } => {
            if payload.len()
                > usize::try_from(maximum_payload_bytes).map_err(|_| WireError::OversizedPayload)?
            {
                return Err(WireError::OversizedPayload);
            }
            writer.u64(sequence)?;
            writer.byte_field(payload)?;
        }
        SessionMessage::Pressure { sequence } => writer.u64(sequence)?,
        SessionMessage::Accepted { sequence } | SessionMessage::Delivered { sequence } => {
            writer.u64(sequence)?;
        }
        SessionMessage::InputClosed { final_sequence } => writer.u64(final_sequence)?,
        SessionMessage::InputAbnormal {
            final_sequence,
            terminal,
        } => {
            if terminal.len()
                > usize::try_from(maximum_payload_bytes).map_err(|_| WireError::OversizedPayload)?
            {
                return Err(WireError::OversizedPayload);
            }
            writer.u64(final_sequence)?;
            writer.byte_field(terminal)?;
        }
        SessionMessage::Cancelled { code } | SessionMessage::Failed { code } => writer.u16(code)?,
        SessionMessage::Terminal {
            disposition,
            final_sequence,
        } => {
            writer.u8(terminal_code(disposition))?;
            writer.u64(final_sequence)?;
        }
    }
    Ok(())
}
pub(super) fn decode<'a>(
    cursor: &mut Cursor<'a>,
    kind: u8,
    maximum_payload_bytes: u32,
) -> Result<SessionMessage<'a>, WireError> {
    Ok(match kind {
        1 => SessionMessage::Hello(SessionHello {
            line_id: cursor.text()?,
            link_binding_id: cursor.text()?,
            base: cursor.text()?,
            base_instance_id: cursor.text()?,
            contract: decode_line_contract(cursor)?,
            source_endpoint_id: cursor.text()?,
            sink_endpoint_id: cursor.text()?,
            limits: LinkLimits {
                maximum_in_flight_items: cursor.u16()?,
                maximum_payload_bytes: cursor.u32()?,
                maximum_buffered_bytes: cursor.u32()?,
                maximum_frame_bytes: cursor.u32()?,
            },
        }),
        2 => SessionMessage::Ready,
        3 => {
            let sequence = cursor.u64()?;
            let payload = cursor.byte_field()?;
            if payload.len()
                > usize::try_from(maximum_payload_bytes).map_err(|_| WireError::OversizedPayload)?
            {
                return Err(WireError::OversizedPayload);
            }
            SessionMessage::Offered { sequence, payload }
        }
        10 => SessionMessage::Pressure {
            sequence: cursor.u64()?,
        },
        4 => SessionMessage::Accepted {
            sequence: cursor.u64()?,
        },
        5 => SessionMessage::Delivered {
            sequence: cursor.u64()?,
        },
        6 => SessionMessage::InputClosed {
            final_sequence: cursor.u64()?,
        },
        11 => {
            let final_sequence = cursor.u64()?;
            let terminal = cursor.byte_field()?;
            if terminal.len()
                > usize::try_from(maximum_payload_bytes).map_err(|_| WireError::OversizedPayload)?
            {
                return Err(WireError::OversizedPayload);
            }
            SessionMessage::InputAbnormal {
                final_sequence,
                terminal,
            }
        }
        7 => SessionMessage::Cancelled {
            code: cursor.u16()?,
        },
        8 => SessionMessage::Failed {
            code: cursor.u16()?,
        },
        9 => SessionMessage::Terminal {
            disposition: decode_terminal(cursor.u8()?)?,
            final_sequence: cursor.u64()?,
        },
        _ => return Err(WireError::InvalidMessageKind),
    })
}
