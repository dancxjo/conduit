//! Bounded WebSocket transport and session lifecycle for the hosted two-Host Tour.

use std::{io::Write, thread};

use conduit_kernel::{
    scheduler::{HostCallRequest, RemoteIngressOutcome, SchedulerStatus},
    HostCallDisposition, HostCallOutcome, RemoteEndpointId,
};
use conduit_plan_lowering::lowering::RemoteCordDirection;
use conduit_std_host::{
    websocket::{NativeWebSocketLine, NativeWebSocketListener},
    InstalledRemoteFragment, RemoteValueTransfer,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionMessage, SessionTerminalDisposition,
};

const MAXIMUM_FRAME_BYTES: u32 = 2_048;
pub(super) fn exact_endpoint(
    runtime: &InstalledRemoteFragment,
    direction: RemoteCordDirection,
) -> Result<RemoteEndpointId, String> {
    let mut matches = runtime
        .sessions()
        .iter()
        .filter(|session| session.direction == direction);
    let endpoint = matches
        .next()
        .ok_or("two-Host remote endpoint missing")?
        .endpoint;
    if matches.next().is_some() {
        return Err("two-Host remote endpoint is ambiguous".into());
    }
    Ok(endpoint)
}

fn next_transfer(
    runtime: &mut InstalledRemoteFragment,
) -> Result<conduit_std_host::RemoteValueTransfer, String> {
    let endpoint = exact_endpoint(runtime, RemoteCordDirection::Egress)?;
    for _ in 0..32 {
        if let Some(transfer) = runtime.next_egress(endpoint)? {
            return Ok(transfer);
        }
        match runtime.step()? {
            SchedulerStatus::Progress { .. } => {}
            state => return Err(format!("source reached {state:?} before its Line value")),
        }
    }
    Err("source exceeded bounded steps before its Line value".into())
}

fn drive_sink(
    runtime: &mut InstalledRemoteFragment,
    output: &mut impl Write,
) -> Result<(), String> {
    for _ in 0..32 {
        if let Some(request) = runtime.next_host_request() {
            return complete_text_presentation(runtime, request, output);
        }
        match runtime.step()? {
            SchedulerStatus::Progress { .. } => {}
            state => return Err(format!("sink reached {state:?} before presentation")),
        }
    }
    Err("sink exceeded bounded steps before presentation".into())
}

fn complete_text_presentation(
    runtime: &mut InstalledRemoteFragment,
    request: HostCallRequest,
    output: &mut impl Write,
) -> Result<(), String> {
    let work = runtime.describe_host_request(request)?;
    if work.contract_id.as_str() != conduit_core::PRESENT_HOST_CALL_CONTRACT {
        return Err(format!(
            "unexpected two-Host Call {}",
            work.contract_id.as_str()
        ));
    }
    let text =
        core::str::from_utf8(&work.input).map_err(|_| "presented two-Host text is not UTF-8")?;
    writeln!(output, "{text}").map_err(|error| error.to_string())?;
    runtime.complete_host_call(
        request,
        HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        },
    )
}

fn drain_source(runtime: &mut InstalledRemoteFragment) -> Result<(), String> {
    for _ in 0..32 {
        if matches!(runtime.step()?, SchedulerStatus::Drained) {
            return Ok(());
        }
    }
    Err("source did not drain within its admitted steps".into())
}

fn drain_sink(
    runtime: &mut InstalledRemoteFragment,
    output: &mut impl Write,
) -> Result<(), String> {
    for _ in 0..32 {
        if let Some(request) = runtime.next_host_request() {
            complete_text_presentation(runtime, request, output)?;
        }
        if matches!(runtime.step()?, SchedulerStatus::Drained) {
            return Ok(());
        }
    }
    Err("sink did not drain within its admitted steps".into())
}

pub(super) fn execute_line(
    mut source: InstalledRemoteFragment,
    source_endpoint: RemoteEndpointId,
    sink: InstalledRemoteFragment,
    sink_endpoint: RemoteEndpointId,
) -> Result<(RemoteValueTransfer, Vec<u8>), String> {
    let listener = NativeWebSocketListener::bind_loopback(MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("bind Tour Line: {error:?}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("locate Tour Line: {error:?}"))?;
    let url = listener
        .url()
        .map_err(|error| format!("name Tour Line: {error:?}"))?;
    let sink_handle = thread::spawn(move || run_sink(sink, sink_endpoint, address, &url));
    let mut line = listener
        .accept()
        .map_err(|error| format!("accept Tour Line: {error:?}"))?;
    activate_source(&mut source, source_endpoint, &mut line)?;
    let transfer = next_transfer(&mut source)?;
    send_message(
        &mut source,
        source_endpoint,
        &mut line,
        SessionMessage::Offered {
            sequence: transfer.sequence,
            payload: &transfer.bytes,
        },
    )?;
    match receive_message(&mut source, source_endpoint, &mut line)? {
        ReceivedMessage::Accepted { sequence } if sequence == transfer.sequence => {
            source.accept_egress(&transfer)?;
        }
        message => return Err(format!("unexpected Tour Line acceptance {message:?}")),
    }
    match receive_message(&mut source, source_endpoint, &mut line)? {
        ReceivedMessage::Delivered { sequence } if sequence == transfer.sequence => {
            source.deliver_egress(&transfer)?;
        }
        message => return Err(format!("unexpected Tour Line delivery {message:?}")),
    }
    drain_source(&mut source)?;
    send_message(
        &mut source,
        source_endpoint,
        &mut line,
        SessionMessage::InputClosed { final_sequence: 1 },
    )?;
    send_message(
        &mut source,
        source_endpoint,
        &mut line,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1,
        },
    )?;
    if !matches!(
        receive_message(&mut source, source_endpoint, &mut line)?,
        ReceivedMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1
        }
    ) || !source
        .sessions()
        .get(source_endpoint)
        .ok_or("source Line session missing")?
        .machine()
        .is_terminal()
    {
        return Err("source Tour Line did not terminate exactly".into());
    }
    line.close()
        .map_err(|error| format!("close Tour Line: {error:?}"))?;
    let sink_output = sink_handle
        .join()
        .map_err(|_| "sink Tour Host panicked".to_string())??;
    Ok((transfer, sink_output))
}

fn run_sink(
    mut sink: InstalledRemoteFragment,
    sink_endpoint: RemoteEndpointId,
    address: std::net::SocketAddr,
    url: &str,
) -> Result<Vec<u8>, String> {
    let mut line = NativeWebSocketLine::connect(address, url, MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("connect Tour Line: {error:?}"))?;
    activate_sink(&mut sink, sink_endpoint, &mut line)?;
    let mut output = Vec::with_capacity(conduit_text::MAX_TEXT_BYTES as usize + 1);
    let sequence = match receive_message(&mut sink, sink_endpoint, &mut line)? {
        ReceivedMessage::Offered { sequence, payload } => {
            match sink.admit_ingress(sink_endpoint, sequence, &payload)? {
                RemoteIngressOutcome::Accepted { .. } => sequence,
                RemoteIngressOutcome::Full { .. } => return Err("Tour sink Line pressure".into()),
            }
        }
        message => return Err(format!("unexpected Tour Line offer {message:?}")),
    };
    send_message(
        &mut sink,
        sink_endpoint,
        &mut line,
        SessionMessage::Accepted { sequence },
    )?;
    drive_sink(&mut sink, &mut output)?;
    send_message(
        &mut sink,
        sink_endpoint,
        &mut line,
        SessionMessage::Delivered { sequence },
    )?;
    match receive_message(&mut sink, sink_endpoint, &mut line)? {
        ReceivedMessage::InputClosed { final_sequence: 1 } => sink.close_ingress(sink_endpoint)?,
        message => return Err(format!("unexpected Tour Line close {message:?}")),
    }
    if !matches!(
        receive_message(&mut sink, sink_endpoint, &mut line)?,
        ReceivedMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1
        }
    ) {
        return Err("sink Tour Line received wrong terminal".into());
    }
    drain_sink(&mut sink, &mut output)?;
    send_message(
        &mut sink,
        sink_endpoint,
        &mut line,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 1,
        },
    )?;
    if !sink
        .sessions()
        .get(sink_endpoint)
        .ok_or("sink Line session missing")?
        .machine()
        .is_terminal()
    {
        return Err("sink Tour Line did not terminate exactly".into());
    }
    Ok(output)
}

fn activate_source(
    runtime: &mut InstalledRemoteFragment,
    endpoint: RemoteEndpointId,
    line: &mut NativeWebSocketLine,
) -> Result<(), String> {
    if !matches!(
        receive_message(runtime, endpoint, line)?,
        ReceivedMessage::Hello
    ) {
        return Err("Tour sink omitted Hello".into());
    }
    let binding = runtime
        .sessions()
        .get(endpoint)
        .ok_or("source Line binding missing")?
        .binding()
        .clone();
    send_message(runtime, endpoint, line, binding.hello_frame().message)?;
    if !matches!(
        receive_message(runtime, endpoint, line)?,
        ReceivedMessage::Ready
    ) {
        return Err("Tour sink omitted Ready".into());
    }
    send_message(runtime, endpoint, line, SessionMessage::Ready)
}

fn activate_sink(
    runtime: &mut InstalledRemoteFragment,
    endpoint: RemoteEndpointId,
    line: &mut NativeWebSocketLine,
) -> Result<(), String> {
    let binding = runtime
        .sessions()
        .get(endpoint)
        .ok_or("sink Line binding missing")?
        .binding()
        .clone();
    send_message(runtime, endpoint, line, binding.hello_frame().message)?;
    if !matches!(
        receive_message(runtime, endpoint, line)?,
        ReceivedMessage::Hello
    ) {
        return Err("Tour source omitted Hello".into());
    }
    send_message(runtime, endpoint, line, SessionMessage::Ready)?;
    if !matches!(
        receive_message(runtime, endpoint, line)?,
        ReceivedMessage::Ready
    ) {
        return Err("Tour source omitted Ready".into());
    }
    Ok(())
}

fn send_message(
    runtime: &mut InstalledRemoteFragment,
    endpoint: RemoteEndpointId,
    line: &mut NativeWebSocketLine,
    message: SessionMessage<'_>,
) -> Result<(), String> {
    let binding = runtime
        .sessions()
        .get(endpoint)
        .ok_or("Tour Line binding missing")?
        .binding()
        .clone();
    let frame = binding.frame(message);
    runtime
        .sessions_mut()
        .get_mut(endpoint)
        .unwrap()
        .machine_mut()
        .admit_outbound(frame)
        .map_err(|error| format!("admit outbound Tour Line frame: {error:?}"))?;
    let mut bytes = [0; MAXIMUM_FRAME_BYTES as usize];
    let length = encode_session_frame_into(
        frame,
        &mut bytes,
        conduit_text::MAX_TEXT_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("encode Tour Line frame: {error:?}"))?;
    line.send_binary(&bytes[..length])
        .map_err(|error| format!("send Tour Line frame: {error:?}"))
}

#[derive(Debug)]
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
    InputClosed {
        final_sequence: u64,
    },
    Terminal {
        disposition: SessionTerminalDisposition,
        final_sequence: u64,
    },
}

fn receive_message(
    runtime: &mut InstalledRemoteFragment,
    endpoint: RemoteEndpointId,
    line: &mut NativeWebSocketLine,
) -> Result<ReceivedMessage, String> {
    let mut bytes = [0; MAXIMUM_FRAME_BYTES as usize];
    let length = line
        .receive_binary(&mut bytes)
        .map_err(|error| format!("receive Tour Line frame: {error:?}"))?;
    let frame = decode_session_frame(
        &bytes[..length],
        conduit_text::MAX_TEXT_BYTES,
        MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("decode Tour Line frame: {error:?}"))?;
    runtime
        .sessions_mut()
        .get_mut(endpoint)
        .ok_or("Tour Line session missing")?
        .machine_mut()
        .admit_inbound(frame)
        .map_err(|error| format!("admit inbound Tour Line frame: {error:?}"))?;
    match frame.message {
        SessionMessage::Hello(_) => Ok(ReceivedMessage::Hello),
        SessionMessage::Ready => Ok(ReceivedMessage::Ready),
        SessionMessage::Offered { sequence, payload } => Ok(ReceivedMessage::Offered {
            sequence,
            payload: payload.to_vec(),
        }),
        SessionMessage::Accepted { sequence } => Ok(ReceivedMessage::Accepted { sequence }),
        SessionMessage::Delivered { sequence } => Ok(ReceivedMessage::Delivered { sequence }),
        SessionMessage::InputClosed { final_sequence } => {
            Ok(ReceivedMessage::InputClosed { final_sequence })
        }
        SessionMessage::Terminal {
            disposition,
            final_sequence,
        } => Ok(ReceivedMessage::Terminal {
            disposition,
            final_sequence,
        }),
        message => Err(format!("unexpected Tour Line message {message:?}")),
    }
}
