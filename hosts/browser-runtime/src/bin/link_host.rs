use conduit_browser_runtime::{
    std_link_advertisement, websocket_link_binding, websocket_pair_plan, STD_LINK_HOST_ID,
    WIRE_MAXIMUM_PAYLOAD_BYTES,
};
use conduit_core::{ConnectionOutcome, HostCommand, HostEvent, ImplementationId, PlatformEffect};
use conduit_runtime::HostRuntime;
use conduit_signal::signal_registry;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use tungstenite::protocol::{Message, WebSocketConfig};

const MAXIMUM_WEBSOCKET_MESSAGE_BYTES: usize = 512;
const ACKNOWLEDGED: u8 = b'A';
const DELIVERED: u8 = b'D';
const CLOSE_CONNECTION: u8 = b'C';

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "4180".to_string())
        .parse::<u16>()?;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("ready websocket=ws://127.0.0.1:{port} items=1 bytes=64");
    let (stream, _) = listener.accept()?;
    stream.set_nodelay(true)?;
    let config = WebSocketConfig::default()
        .read_buffer_size(1_024)
        .write_buffer_size(0)
        .max_write_buffer_size(1_024)
        .max_message_size(Some(MAXIMUM_WEBSOCKET_MESSAGE_BYTES))
        .max_frame_size(Some(MAXIMUM_WEBSOCKET_MESSAGE_BYTES));
    let mut socket = tungstenite::accept_with_config(stream, Some(config))?;

    let advertisement = std_link_advertisement();
    let registry = signal_registry(
        ImplementationId::from("std/pulse-v1"),
        ImplementationId::from("std/stdout-show-signal-v1"),
    )
    .map_err(|error| format!("signal registry failed: {error:?}"))?;
    let mut runtime = HostRuntime::new(advertisement, registry, 256);
    runtime.replace_link_bindings(vec![websocket_link_binding()]);
    let plan = websocket_pair_plan()?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id.as_str() == STD_LINK_HOST_ID)
        .ok_or("std WebSocket fragment missing")?;
    let plan_id = fragment.plan_id.clone();
    let prepared = runtime.handle(HostCommand::Prepare(fragment));
    require_event(&prepared.events, |event| {
        matches!(event, HostEvent::Prepared { .. })
    })?;
    let activated = runtime.handle(HostCommand::Activate(plan_id));
    require_event(&activated.events, |event| {
        matches!(event, HostEvent::Activated { .. })
    })?;
    let mut pending = activated.effects;
    let mut transmitted = 0u32;
    let mut complete = false;

    while let Some(effect) = pending.pop() {
        let output = match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                duration_ms,
            } => {
                thread::sleep(Duration::from_millis(duration_ms));
                runtime.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                })
            }
            PlatformEffect::TransmitConnection { envelope } => {
                let frame = conduit_wire::encode_envelope(&envelope, WIRE_MAXIMUM_PAYLOAD_BYTES)
                    .map_err(|error| format!("wire encode failed: {error:?}"))?;
                if frame.len() > MAXIMUM_WEBSOCKET_MESSAGE_BYTES {
                    return Err("encoded envelope exceeds WebSocket message bound".into());
                }
                socket.send(Message::Binary(frame.into()))?;
                let accepted_sequence = read_ack(&mut socket, ACKNOWLEDGED)?;
                if accepted_sequence != envelope.sequence {
                    return Err("accepted acknowledgement sequence mismatch".into());
                }
                let accepted = runtime.handle(HostCommand::CompleteConnectionDelivery {
                    plan_id: envelope.plan_id.clone(),
                    connection_id: envelope.connection_id.clone(),
                    sequence: envelope.sequence,
                    outcome: ConnectionOutcome::Accepted,
                });
                require_connection_outcome(
                    &accepted.events,
                    envelope.sequence,
                    ConnectionOutcome::Accepted,
                )?;
                pending.extend(accepted.effects.into_iter().rev());

                let delivered_sequence = read_ack(&mut socket, DELIVERED)?;
                if delivered_sequence != envelope.sequence {
                    return Err("delivered acknowledgement sequence mismatch".into());
                }
                transmitted += 1;
                runtime.handle(HostCommand::CompleteConnectionDelivery {
                    plan_id: envelope.plan_id,
                    connection_id: envelope.connection_id,
                    sequence: envelope.sequence,
                    outcome: ConnectionOutcome::Delivered,
                })
            }
            PlatformEffect::PresentValue { .. } => {
                return Err("std source unexpectedly requested presentation".into())
            }
        };
        complete |= output
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::PlanCompleted { .. }));
        if output.events.iter().any(|event| {
            matches!(
                event,
                HostEvent::ConnectionTerminated {
                    disposition: conduit_core::ConnectionTerminalDisposition {
                        disposition: conduit_core::TerminalDisposition::Completed,
                        ..
                    },
                    ..
                }
            )
        }) {
            socket.send(Message::Binary(vec![CLOSE_CONNECTION].into()))?;
        }
        pending.extend(output.effects.into_iter().rev());
    }

    if !complete || transmitted != 16 {
        return Err(format!(
            "std WebSocket source stopped complete={complete} transmitted={transmitted}"
        )
        .into());
    }
    println!("complete transmitted={transmitted}");
    Ok(())
}

fn read_ack(
    socket: &mut tungstenite::WebSocket<std::net::TcpStream>,
    expected_kind: u8,
) -> Result<u64, Box<dyn std::error::Error>> {
    let message = socket.read()?;
    let Message::Binary(bytes) = message else {
        return Err("expected binary WebSocket acknowledgement".into());
    };
    if bytes.len() != 9 || bytes[0] != expected_kind {
        return Err("malformed WebSocket acknowledgement".into());
    }
    Ok(u64::from_le_bytes(bytes[1..9].try_into()?))
}

fn require_event(
    events: &[HostEvent],
    predicate: impl Fn(&HostEvent) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    events
        .iter()
        .any(predicate)
        .then_some(())
        .ok_or_else(|| format!("required runtime event missing: {events:?}").into())
}

fn require_connection_outcome(
    events: &[HostEvent],
    sequence: u64,
    expected: ConnectionOutcome,
) -> Result<(), Box<dyn std::error::Error>> {
    require_event(events, |event| {
        matches!(
            event,
            HostEvent::ConnectionEnvelopeOutcome {
                sequence: event_sequence,
                outcome,
                ..
            } if *event_sequence == sequence && *outcome == expected
        )
    })
}
