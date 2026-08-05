//! WebSocket session and carrier orchestration for the S4 toggle-demo std source.
//!
//! Owns: `send`, `receive`, the main `run` loop, and `bind_listener`.
//! All methods access `DistributedToggleSource`'s `pub(super)` fields.

use super::source::{DistributedToggleSource, MAXIMUM_VALUES};
use crate::websocket::{NativeWebSocketCarrier, NativeWebSocketListener};
use conduit_kernel::{EvidenceQuery, KernelEventKind, ValueStorage};
use conduit_signal::{DISTRIBUTED_MAXIMUM_FRAME_BYTES, SIGNAL_ENCODED_LEN};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionMessage, SessionTerminalDisposition,
};
use std::io::{BufRead, Write};

pub(super) fn send(
    src: &mut DistributedToggleSource,
    carrier: &mut NativeWebSocketCarrier,
    message: SessionMessage<'_>,
    output: &mut [u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize],
) -> Result<(), String> {
    let frame = src.binding.frame(message);
    src.session
        .admit_outbound(frame)
        .map_err(|error| format!("{error:?}"))?;
    let length = encode_session_frame_into(
        frame,
        output,
        SIGNAL_ENCODED_LEN,
        DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("{error:?}"))?;
    carrier
        .send_binary(&output[..length])
        .map_err(|error| format!("{error:?}"))
}

pub(super) fn receive<'a>(
    src: &mut DistributedToggleSource,
    carrier: &mut NativeWebSocketCarrier,
    input: &'a mut [u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize],
) -> Result<SessionMessage<'a>, String> {
    let length = carrier
        .receive_binary(input)
        .map_err(|error| format!("{error:?}"))?;
    let frame = decode_session_frame(
        &input[..length],
        SIGNAL_ENCODED_LEN,
        DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("{error:?}"))?;
    src.session
        .admit_inbound(frame)
        .map_err(|error| format!("{error:?}"))?;
    Ok(frame.message)
}

/// Drive the source, reading Enter from `stdin` before each activation.
pub(super) fn run_source<R: BufRead, W: Write>(
    mut src: DistributedToggleSource,
    listener: NativeWebSocketListener,
    stdin: &mut R,
    report: &mut W,
) -> Result<(), String> {
    let mut carrier = listener.accept().map_err(|error| format!("{error:?}"))?;
    let mut outbound = [0_u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize];
    let mut inbound = [0_u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize];

    if !matches!(
        receive(&mut src, &mut carrier, &mut inbound).map_err(|detail| {
            format!("CND-TOG-S4-201 phase=before-readiness detail={detail}")
        })?,
        SessionMessage::Hello(_)
    ) {
        return Err("browser did not begin with exact Hello".to_string());
    }
    let hello_binding = src.binding.clone();
    let hello = hello_binding.hello_frame().message;
    send(&mut src, &mut carrier, hello, &mut outbound)?;
    if !matches!(
        receive(&mut src, &mut carrier, &mut inbound).map_err(|detail| {
            format!("CND-TOG-S4-201 phase=before-readiness detail={detail}")
        })?,
        SessionMessage::Ready
    ) {
        return Err("browser did not report Ready".to_string());
    }
    send(&mut src, &mut carrier, SessionMessage::Ready, &mut outbound)?;
    if !src.session.is_active() {
        return Err("std source activated before both exact readiness facts".to_string());
    }

    let mut activation_index = 0usize;
    while let Some((sequence, payload)) = src.next_offer(report, stdin, &mut activation_index)? {
        loop {
            send(
                &mut src,
                &mut carrier,
                SessionMessage::Offered {
                    sequence,
                    payload: &payload,
                },
                &mut outbound,
            )?;
            match receive(&mut src, &mut carrier, &mut inbound).map_err(|detail| {
                format!("CND-TOG-S4-202 phase=value-in-flight sequence={sequence} detail={detail}")
            })? {
                SessionMessage::Pressure {
                    sequence: pressured,
                } if pressured == sequence => {
                    src.pressure_retries += 1;
                    continue;
                }
                SessionMessage::Accepted { sequence: accepted } if accepted == sequence => {
                    let (endpoint, cord) = src.remote();
                    src.scheduler
                        .remote_egress_accept(endpoint, cord, sequence)
                        .map_err(|error| format!("{error:?}"))?;
                }
                other => return Err(format!("unexpected offer response {other:?}")),
            }
            match receive(&mut src, &mut carrier, &mut inbound).map_err(|detail| {
                format!("CND-TOG-S4-202 phase=value-in-flight sequence={sequence} detail={detail}")
            })? {
                SessionMessage::Delivered {
                    sequence: delivered,
                } if delivered == sequence => {
                    let (endpoint, cord) = src.remote();
                    src.scheduler
                        .remote_egress_delivered(endpoint, cord, sequence)
                        .map_err(|error| format!("{error:?}"))?;
                    break;
                }
                other => return Err(format!("unexpected delivery response {other:?}")),
            }
        }
    }
    let (endpoint, cord) = src.remote();
    if !src
        .scheduler
        .remote_egress_terminal(endpoint, cord)
        .map_err(|error| format!("{error:?}"))?
    {
        return Err("std remote egress was not terminal".to_string());
    }
    let final_sequence = MAXIMUM_VALUES as u64;
    send(
        &mut src,
        &mut carrier,
        SessionMessage::InputClosed { final_sequence },
        &mut outbound,
    )?;
    send(
        &mut src,
        &mut carrier,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence,
        },
        &mut outbound,
    )?;
    match receive(&mut src, &mut carrier, &mut inbound)
        .map_err(|detail| format!("CND-TOG-S4-203 phase=terminal-agreement detail={detail}"))?
    {
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: peer_final,
        } if peer_final == final_sequence => {}
        other => return Err(format!("unexpected browser terminal {other:?}")),
    }
    if !src.session.is_terminal()
        || src.scheduler.values().used_items() != 0
        || src
            .scheduler
            .cord_usage(cord)
            .map_err(|error| format!("{error:?}"))?
            != (0, 0)
        || !src
            .scheduler
            .evidence()
            .contains_kind(KernelEventKind::RemoteValueDelivered)
        || !src
            .scheduler
            .evidence()
            .contains_kind(KernelEventKind::OperationCompleted)
        || src.capacity_seal() != src.seal
    {
        return Err("distributed toggle source terminal invariants failed".to_string());
    }
    writeln!(
        report,
        "summary plan={} source_fragment={} sink_fragment={} source_play={} browser_play={} values={} pressure_retries={} retained=0 in_flight=0 source_terminal=completed browser_terminal=completed capacity_stable=true",
        src.binding.plan_id.as_str(),
        src.binding.source_fragment_id.as_str(),
        src.binding.sink_fragment_id.as_str(),
        src.binding.source_active_play_id.as_str(),
        src.binding.sink_active_play_id.as_str(),
        MAXIMUM_VALUES,
        src.pressure_retries,
    )
    .map_err(|error| error.to_string())?;
    carrier.close().map_err(|error| format!("{error:?}"))?;
    Ok(())
}

pub fn bind_listener() -> Result<NativeWebSocketListener, String> {
    NativeWebSocketListener::bind_loopback(DISTRIBUTED_MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("{error:?}"))
}
