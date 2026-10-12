//! Finite coordinator-side machines for the exact selected operation's Lines.
//! Provider choice is external common-pool evidence, never made by this codec.
use conduit_core::{PlacementId, Plan, PoolMemberSessionDirection, PoolSelectionEvidence};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use std::io::{Read, Write};

const DOCUMENT: usize = 1024 * 1024;
const FRAME: usize = 128 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Initialize {
    plan: Plan,
    selection: PoolSelectionEvidence,
    consumer: PlacementId,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Hello {
        session: usize,
    },
    Ready {
        session: usize,
    },
    Offer {
        session: usize,
        sequence: u64,
        payload: Vec<u8>,
    },
    Accepted {
        session: usize,
        sequence: u64,
    },
    Delivered {
        session: usize,
        sequence: u64,
    },
    Close {
        session: usize,
        final_sequence: u64,
    },
    Receive {
        session: usize,
        frame: Vec<u8>,
    },
}
struct Session {
    binding: SessionBinding,
    machine: SessionMachine,
}
fn read<T: DeserializeOwned>(input: &mut impl Read) -> Result<T, String> {
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "session control header unavailable")?;
    let size = u32::from_be_bytes(prefix) as usize;
    if size == 0 || size > DOCUMENT {
        return Err("session control extent refused".into());
    }
    let mut bytes = vec![0; size];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "session control incomplete")?;
    serde_json::from_slice(&bytes).map_err(|_| "session control invalid".into())
}
fn reply(output: &mut impl Write, value: serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(&value).map_err(|_| "session receipt invalid")?;
    if bytes.len() > DOCUMENT {
        return Err("session receipt extent refused".into());
    }
    output
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .map_err(|_| "session receipt header failed")?;
    output
        .write_all(&bytes)
        .map_err(|_| "session receipt failed")?;
    output
        .flush()
        .map_err(|_| "session receipt flush failed".into())
}
fn send(
    session: &mut Session,
    output: &mut [u8],
    message: SessionMessage<'_>,
) -> Result<usize, String> {
    let frame = session.binding.frame(message);
    let length = encode_session_frame_into(
        frame,
        output,
        session.binding.limits.maximum_payload_bytes,
        session.binding.attachment.limits.maximum_frame_bytes,
    )
    .map_err(|e| format!("encode selected session: {e:?}"))?;
    session
        .machine
        .admit_outbound(frame)
        .map_err(|e| format!("selected outbound session: {e:?}"))?;
    Ok(length)
}
pub(super) fn run() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let initial: Initialize = read(&mut input)?;
    initial
        .selection
        .validate(&initial.plan)
        .map_err(|e| format!("selected session evidence: {e:?}"))?;
    let pool = initial
        .plan
        .fragments
        .first()
        .and_then(|f| {
            f.shared_pools
                .iter()
                .find(|p| p.pool_id == initial.selection.pool_id)
        })
        .ok_or("selected session pool absent")?;
    if pool.member_front.inputs().len() != 1 || pool.member_front.outputs().len() != 1 {
        return Err("native proof requires one input and one output session".into());
    }
    let mut sessions = Vec::with_capacity(2);
    for (direction, port, role) in [
        (
            PoolMemberSessionDirection::Input,
            &pool.member_front.inputs()[0],
            SessionRole::Source,
        ),
        (
            PoolMemberSessionDirection::Output,
            &pool.member_front.outputs()[0],
            SessionRole::Sink,
        ),
    ] {
        let binding = SessionBinding::from_selected_pool_operation(
            &initial.plan,
            &initial.selection,
            &initial.consumer,
            direction,
            &port.port_id,
        )
        .map_err(|e| format!("selected session binding: {e:?}"))?;
        if binding.attachment.limits.maximum_frame_bytes as usize > FRAME {
            return Err("selected session exceeds prepared frame bound".into());
        }
        let machine = SessionMachine::new(binding.clone(), role)
            .map_err(|e| format!("selected session preparation: {e:?}"))?;
        sessions.push(Session { binding, machine });
    }
    let mut buffer = vec![0; FRAME];
    reply(
        &mut output,
        json!({"phase":"selected-sessions-ready","sessions":2}),
    )?;
    for _ in 0..64 {
        let command: Option<Command> = read(&mut input)?;
        let Some(command) = command else {
            return Ok(());
        };
        let (index, message) = match &command {
            Command::Hello { session } => {
                let current = sessions.get_mut(*session).ok_or("session index refused")?;
                let binding = current.binding.clone();
                let length = send(current, &mut buffer, binding.hello_frame().message)?;
                reply(
                    &mut output,
                    json!({"phase":"session-frame","frame":&buffer[..length]}),
                )?;
                continue;
            }
            Command::Ready { session } => (*session, SessionMessage::Ready),
            Command::Offer {
                session,
                sequence,
                payload,
            } => (
                *session,
                SessionMessage::Offered {
                    sequence: *sequence,
                    payload,
                },
            ),
            Command::Accepted { session, sequence } => (
                *session,
                SessionMessage::Accepted {
                    sequence: *sequence,
                },
            ),
            Command::Delivered { session, sequence } => (
                *session,
                SessionMessage::Delivered {
                    sequence: *sequence,
                },
            ),
            Command::Close {
                session,
                final_sequence,
            } => (
                *session,
                SessionMessage::InputClosed {
                    final_sequence: *final_sequence,
                },
            ),
            Command::Receive { session, frame } => {
                let current = sessions.get_mut(*session).ok_or("session index refused")?;
                let frame = decode_session_frame(
                    frame,
                    current.binding.limits.maximum_payload_bytes,
                    current.binding.attachment.limits.maximum_frame_bytes,
                )
                .map_err(|e| format!("decode selected session: {e:?}"))?;
                current
                    .machine
                    .admit_inbound(frame)
                    .map_err(|e| format!("selected inbound session: {e:?}"))?;
                let value = match frame.message {
                    SessionMessage::Hello(_) => json!({"message":"hello"}),
                    SessionMessage::Ready => json!({"message":"ready"}),
                    SessionMessage::Offered { sequence, payload } => {
                        json!({"message":"offered","sequence":sequence,"payload":payload})
                    }
                    SessionMessage::Accepted { sequence } => {
                        json!({"message":"accepted","sequence":sequence})
                    }
                    SessionMessage::Delivered { sequence } => {
                        json!({"message":"delivered","sequence":sequence})
                    }
                    SessionMessage::InputClosed { final_sequence } => {
                        json!({"message":"closed","final_sequence":final_sequence})
                    }
                    _ => return Err("unsupported selected session message".into()),
                };
                reply(
                    &mut output,
                    json!({"phase":"session-received","value":value}),
                )?;
                continue;
            }
        };
        let session = sessions.get_mut(index).ok_or("session index refused")?;
        let length = send(session, &mut buffer, message)?;
        reply(
            &mut output,
            json!({"phase":"session-frame","frame":&buffer[..length]}),
        )?;
    }
    Err("selected session control budget exhausted".into())
}
