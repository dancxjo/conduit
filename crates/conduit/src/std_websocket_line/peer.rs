use super::{kernel, Driver, MAXIMUM_FRAME_BYTES, MAXIMUM_VALUES, SIGNAL_ENCODED_LEN_USIZE};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::FixedValueStore;
use conduit_std_host::websocket::NativeWebSocketLine;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};

pub(super) fn run_sink(
    lowered: conduit_plan_lowering::lowering::LoweredPlanFragment,
    binding: SessionBinding,
    address: std::net::SocketAddr,
    url: &str,
) -> Result<usize, String> {
    let values =
        FixedValueStore::<MAXIMUM_VALUES, { MAXIMUM_VALUES * SIGNAL_ENCODED_LEN_USIZE }>::new(
            (MAXIMUM_VALUES * SIGNAL_ENCODED_LEN_USIZE) as u32,
        )
        .map_err(|error| format!("{error:?}"))?;
    let mut kernel = kernel(&lowered, Driver::Sink { received: 0 }, values)?;
    let remote = &lowered.remote_endpoints[0];
    let mut line = NativeWebSocketLine::connect(address, url, MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("{error:?}"))?;
    let mut session = SessionMachine::new(binding.clone(), SessionRole::Sink)
        .map_err(|error| format!("{error:?}"))?;
    activate_sink(&mut session, &binding, &mut line)?;
    let mut frame = [0; MAXIMUM_FRAME_BYTES as usize];
    let mut pressured = false;
    loop {
        match receive(&mut session, &mut line, &mut frame)? {
            SessionMessage::Offered { sequence, payload } => {
                if !pressured {
                    pressured = true;
                    send(
                        &mut session,
                        &binding,
                        &mut line,
                        SessionMessage::Pressure { sequence },
                        &mut frame,
                    )?;
                    continue;
                }
                match kernel
                    .admit_remote_input(remote.endpoint, remote.cord, sequence, payload)
                    .map_err(|error| format!("{error:?}"))?
                {
                    RemoteIngressOutcome::Accepted { .. } => {}
                    RemoteIngressOutcome::Full { .. } => {
                        return Err("sink remained pressured after retry".into())
                    }
                }
                send(
                    &mut session,
                    &binding,
                    &mut line,
                    SessionMessage::Accepted { sequence },
                    &mut frame,
                )?;
                kernel.step().map_err(|error| format!("{error:?}"))?;
                send(
                    &mut session,
                    &binding,
                    &mut line,
                    SessionMessage::Delivered { sequence },
                    &mut frame,
                )?;
            }
            SessionMessage::InputClosed { final_sequence }
                if final_sequence == MAXIMUM_VALUES as u64 =>
            {
                kernel
                    .close_remote_input(remote.endpoint, remote.cord)
                    .map_err(|error| format!("{error:?}"))?
            }
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            } if final_sequence == MAXIMUM_VALUES as u64 => {
                while !matches!(
                    kernel.step().map_err(|error| format!("{error:?}"))?,
                    SchedulerStatus::Complete
                ) {}
                send(
                    &mut session,
                    &binding,
                    &mut line,
                    SessionMessage::Terminal {
                        disposition: SessionTerminalDisposition::Completed,
                        final_sequence,
                    },
                    &mut frame,
                )?;
                let Driver::Sink { received } = kernel.drivers()[0] else {
                    return Err("sink kernel changed driver identity".into());
                };
                return Ok(received);
            }
            other => return Err(format!("unexpected sink message {other:?}")),
        }
    }
}

pub(super) fn activate_source(
    session: &mut SessionMachine,
    binding: &SessionBinding,
    line: &mut NativeWebSocketLine,
) -> Result<(), String> {
    let mut frame = [0; MAXIMUM_FRAME_BYTES as usize];
    if !matches!(
        receive(session, line, &mut frame)?,
        SessionMessage::Hello(_)
    ) {
        return Err("sink omitted Hello".into());
    }
    send(
        session,
        binding,
        line,
        binding.hello_frame().message,
        &mut frame,
    )?;
    if !matches!(receive(session, line, &mut frame)?, SessionMessage::Ready) {
        return Err("sink omitted Ready".into());
    }
    send(session, binding, line, SessionMessage::Ready, &mut frame)
}

fn activate_sink(
    session: &mut SessionMachine,
    binding: &SessionBinding,
    line: &mut NativeWebSocketLine,
) -> Result<(), String> {
    let mut frame = [0; MAXIMUM_FRAME_BYTES as usize];
    send(
        session,
        binding,
        line,
        binding.hello_frame().message,
        &mut frame,
    )?;
    if !matches!(
        receive(session, line, &mut frame)?,
        SessionMessage::Hello(_)
    ) {
        return Err("source omitted Hello".into());
    }
    send(session, binding, line, SessionMessage::Ready, &mut frame)?;
    if !matches!(receive(session, line, &mut frame)?, SessionMessage::Ready) {
        return Err("source omitted Ready".into());
    }
    Ok(())
}

pub(super) fn send(
    session: &mut SessionMachine,
    binding: &SessionBinding,
    line: &mut NativeWebSocketLine,
    message: SessionMessage<'_>,
    output: &mut [u8; MAXIMUM_FRAME_BYTES as usize],
) -> Result<(), String> {
    let frame = binding.frame(message);
    session
        .admit_outbound(frame)
        .map_err(|error| format!("{error:?}"))?;
    let len = encode_session_frame_into(
        frame,
        output,
        conduit_signal::SIGNAL_ENCODED_LEN,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("{error:?}"))?;
    line.send_binary(&output[..len])
        .map_err(|error| format!("{error:?}"))
}

pub(super) fn receive<'a>(
    session: &mut SessionMachine,
    line: &mut NativeWebSocketLine,
    input: &'a mut [u8; MAXIMUM_FRAME_BYTES as usize],
) -> Result<SessionMessage<'a>, String> {
    let len = line
        .receive_binary(input)
        .map_err(|error| format!("{error:?}"))?;
    let frame = decode_session_frame(
        &input[..len],
        conduit_signal::SIGNAL_ENCODED_LEN,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("{error:?}"))?;
    session
        .admit_inbound(frame)
        .map_err(|error| format!("{error:?}"))?;
    Ok(frame.message)
}
