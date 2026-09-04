use super::*;

pub(super) fn send_native(
    line: &mut NativeWebSocketLine,
    session: &mut SessionMachine,
    binding: &SessionBinding,
    message: SessionMessage<'_>,
    output: &mut [u8],
) -> Result<(), CrossHostRendererError> {
    let frame = binding.frame(message);
    session
        .admit_outbound(frame)
        .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
    let length = encode_session_frame_into(
        frame,
        output,
        MAX_RENDERER_VALUE_BYTES,
        CROSS_HOST_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
    line.send_binary(&output[..length])
        .map_err(|error| CrossHostRendererError::Line(format!("{error:?}")))
}

pub(super) fn receive_native<'a>(
    line: &mut NativeWebSocketLine,
    session: &mut SessionMachine,
    input: &'a mut [u8],
) -> Result<SessionMessage<'a>, CrossHostRendererError> {
    let length = line
        .receive_binary(input)
        .map_err(|error| CrossHostRendererError::Line(format!("{error:?}")))?;
    decode_and_admit(session, &input[..length])
}

pub(super) fn send_client(
    line: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    session: &mut SessionMachine,
    binding: &SessionBinding,
    message: SessionMessage<'_>,
    output: &mut [u8],
) -> Result<(), CrossHostRendererError> {
    let frame = binding.frame(message);
    session
        .admit_outbound(frame)
        .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
    let length = encode_session_frame_into(
        frame,
        output,
        MAX_RENDERER_VALUE_BYTES,
        CROSS_HOST_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
    line.send(Message::Binary(output[..length].to_vec().into()))
        .map_err(|error| CrossHostRendererError::Line(error.to_string()))
}

pub(super) fn receive_client<'a>(
    line: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    session: &mut SessionMachine,
    input: &'a mut [u8],
) -> Result<SessionMessage<'a>, CrossHostRendererError> {
    let message = line
        .read()
        .map_err(|error| CrossHostRendererError::Line(error.to_string()))?;
    let bytes = match message {
        Message::Binary(bytes) if bytes.len() <= input.len() => bytes,
        _ => {
            return Err(CrossHostRendererError::Line(
                "WebSocket Line yielded a non-binary or oversized frame".into(),
            ))
        }
    };
    input[..bytes.len()].copy_from_slice(&bytes);
    decode_and_admit(session, &input[..bytes.len()])
}

fn decode_and_admit<'a>(
    session: &mut SessionMachine,
    bytes: &'a [u8],
) -> Result<SessionMessage<'a>, CrossHostRendererError> {
    let frame = decode_session_frame(
        bytes,
        MAX_RENDERER_VALUE_BYTES,
        CROSS_HOST_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
    session
        .admit_inbound(frame)
        .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
    Ok(frame.message)
}

pub(super) fn expect_message(
    message: SessionMessage<'_>,
    expected: impl FnOnce(SessionMessage<'_>) -> bool,
    label: &str,
) -> Result<(), CrossHostRendererError> {
    if expected(message) {
        Ok(())
    } else {
        Err(CrossHostRendererError::Session(format!("expected {label}")))
    }
}

pub(super) fn expect_sequence(
    message: SessionMessage<'_>,
    expected: u64,
    accepted: bool,
) -> Result<(), CrossHostRendererError> {
    let matches = if accepted {
        matches!(message, SessionMessage::Accepted { sequence } if sequence == expected)
    } else {
        matches!(message, SessionMessage::Delivered { sequence } if sequence == expected)
    };
    if matches {
        Ok(())
    } else {
        Err(CrossHostRendererError::Session(
            "unexpected Presentation disposition".into(),
        ))
    }
}
