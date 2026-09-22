//! Browser-owned finite session client for one selected remote pool member.

use conduit_core::{PlacementId, Plan, PoolMemberSessionDirection, PoolSelectionEvidence};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole,
};
use serde::Deserialize;
use std::cell::RefCell;

const CAPACITY: usize = 256 * 1024;
const MAXIMUM_SESSIONS: usize = 16;
const OK: i32 = 0;
const REFUSED: i32 = -1;

thread_local! {
    static INPUT: RefCell<Box<[u8; CAPACITY]>> = RefCell::new(Box::new([0; CAPACITY]));
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(CAPACITY));
    static STATE: RefCell<Option<PoolMemberClient>> = const { RefCell::new(None) };
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    plan: Plan,
    selection: PoolSelectionEvidence,
    consumer_placement_id: PlacementId,
    session_hellos: Vec<Vec<u8>>,
}

struct ClientSession {
    direction: PoolMemberSessionDirection,
    binding: SessionBinding,
    machine: SessionMachine,
}

struct PoolMemberClient {
    sessions: Vec<ClientSession>,
    prompt_offered: bool,
    result_received: bool,
}

fn input(length: u32) -> Result<Vec<u8>, String> {
    let length = usize::try_from(length).map_err(|_| "pool-member input length overflow")?;
    if length == 0 || length > CAPACITY {
        return Err("pool-member input exceeds its finite arena".into());
    }
    Ok(INPUT.with(|input| input.borrow()[..length].to_vec()))
}

fn write(value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > CAPACITY {
        return Err("pool-member output exceeds its finite arena".into());
    }
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        output.clear();
        output.extend_from_slice(&bytes);
    });
    Ok(())
}

fn refuse(message: impl Into<String>) -> i32 {
    let _ = write(&serde_json::json!({
        "schema": "conduit.browser/pool-member-client-refusal@1",
        "message": message.into(),
    }));
    REFUSED
}

fn with_state(action: impl FnOnce(&mut PoolMemberClient) -> Result<(), String>) -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(client) = state.as_mut() else {
            return refuse("pool-member client is not started");
        };
        action(client).map_or_else(refuse, |()| OK)
    })
}

fn encode(
    binding: &SessionBinding,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<Vec<u8>, String> {
    let bound = usize::try_from(binding.attachment.limits.maximum_frame_bytes)
        .map_err(|_| "pool-member frame bound overflow".to_string())?;
    if bound == 0 || bound > CAPACITY {
        return Err("pool-member frame exceeds its finite arena".into());
    }
    let mut bytes = vec![0; bound];
    let length = encode_session_frame_into(
        frame,
        &mut bytes,
        binding.limits.maximum_payload_bytes,
        binding.attachment.limits.maximum_frame_bytes,
    )
    .map_err(|error| format!("encode pool-member frame: {error:?}"))?;
    bytes.truncate(length);
    Ok(bytes)
}

fn message_name(message: SessionMessage<'_>) -> &'static str {
    match message {
        SessionMessage::Hello(_) => "hello",
        SessionMessage::Ready => "ready",
        SessionMessage::Offered { .. } => "offered",
        SessionMessage::Pressure { .. } => "pressure",
        SessionMessage::Accepted { .. } => "accepted",
        SessionMessage::Delivered { .. } => "delivered",
        SessionMessage::InputClosed { .. } => "input-closed",
        SessionMessage::Cancelled { .. } => "cancelled",
        SessionMessage::Failed { .. } => "failed",
        SessionMessage::Terminal { .. } => "terminal",
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_input_capacity() -> u32 {
    CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_output_ptr() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_output_len() -> u32 {
    OUTPUT.with(|output| output.borrow().len() as u32)
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_start(length: u32) -> i32 {
    STATE.with(|state| *state.borrow_mut() = None);
    OUTPUT.with(|output| output.borrow_mut().clear());
    let result = (|| -> Result<(), String> {
        let mut bytes = input(length)?;
        let start: Start = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode pool-member client start: {error}"))?;
        bytes.fill(0);
        start
            .selection
            .validate(&start.plan)
            .map_err(|error| format!("validate pool-member selection: {error:?}"))?;
        let pool = start
            .plan
            .fragments
            .first()
            .and_then(|fragment| {
                fragment
                    .shared_pools
                    .iter()
                    .find(|pool| pool.pool_id == start.selection.pool_id)
            })
            .ok_or_else(|| "selected shared pool is absent from the plan".to_string())?;
        let count = pool
            .member_front
            .inputs()
            .len()
            .checked_add(pool.member_front.outputs().len())
            .ok_or_else(|| "pool-member session count overflow".to_string())?;
        if count == 0 || count > MAXIMUM_SESSIONS || start.session_hellos.len() != count {
            return Err("pool-member session grants differ from the exact finite front".into());
        }

        let mut sessions = Vec::with_capacity(count);
        let mut initial_frames = Vec::with_capacity(count * 2);
        for (direction, ports, role) in [
            (
                PoolMemberSessionDirection::Input,
                pool.member_front.inputs(),
                SessionRole::Source,
            ),
            (
                PoolMemberSessionDirection::Output,
                pool.member_front.outputs(),
                SessionRole::Sink,
            ),
        ] {
            for port in ports {
                let binding = SessionBinding::from_selected_pool_operation(
                    &start.plan,
                    &start.selection,
                    &start.consumer_placement_id,
                    direction,
                    &port.port_id,
                )
                .map_err(|error| format!("bind browser pool-member session: {error:?}"))?;
                let matching = start
                    .session_hellos
                    .iter()
                    .filter_map(|bytes| {
                        decode_session_frame(bytes, CAPACITY as u32, CAPACITY as u32).ok()
                    })
                    .filter(|frame| frame.identity == binding.identity())
                    .collect::<Vec<_>>();
                if matching.len() != 1 {
                    return Err("pool-member grant is missing or ambiguous".into());
                }
                let hello = matching[0];
                if SessionBinding::from_hello_frame(hello)
                    .map_err(|error| format!("reconstruct pool-member grant: {error:?}"))?
                    != binding
                {
                    return Err("pool-member grant differs from the immutable Plan".into());
                }
                let mut machine = SessionMachine::new(binding.clone(), role)
                    .map_err(|error| format!("prepare browser pool-member session: {error:?}"))?;
                machine
                    .admit_inbound(hello)
                    .map_err(|error| format!("admit worker pool-member hello: {error:?}"))?;
                let local_hello = binding.hello_frame();
                machine
                    .admit_outbound(local_hello)
                    .map_err(|error| format!("admit browser pool-member hello: {error:?}"))?;
                let ready = binding.frame(SessionMessage::Ready);
                machine
                    .admit_outbound(ready)
                    .map_err(|error| format!("admit browser pool-member readiness: {error:?}"))?;
                initial_frames.push(encode(&binding, local_hello)?);
                initial_frames.push(encode(&binding, ready)?);
                sessions.push(ClientSession {
                    direction,
                    binding,
                    machine,
                });
            }
        }
        write(&serde_json::json!({
            "schema": "conduit.browser/pool-member-client-started@1",
            "plan_id": start.plan.plan_id,
            "operation_id": start.selection.operation_id,
            "initial_frames": initial_frames,
        }))?;
        STATE.with(|state| {
            *state.borrow_mut() = Some(PoolMemberClient {
                sessions,
                prompt_offered: false,
                result_received: false,
            });
        });
        Ok(())
    })();
    result.map_or_else(refuse, |()| OK)
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_offer(length: u32) -> i32 {
    let mut payload = match input(length) {
        Ok(payload) => payload,
        Err(error) => return refuse(error),
    };
    let status = with_state(|client| {
        if client.prompt_offered {
            return Err("pool-member prompt was already offered".into());
        }
        let session = client
            .sessions
            .iter_mut()
            .find(|session| session.direction == PoolMemberSessionDirection::Input)
            .ok_or_else(|| "pool-member input session is absent".to_string())?;
        let frame = session.binding.frame(SessionMessage::Offered {
            sequence: session.machine.next_sequence(),
            payload: &payload,
        });
        session
            .machine
            .admit_outbound(frame)
            .map_err(|error| format!("admit browser pool-member prompt: {error:?}"))?;
        client.prompt_offered = true;
        write(&serde_json::json!({
            "schema": "conduit.browser/pool-member-client-offer@1",
            "frame": encode(&session.binding, frame)?,
        }))
    });
    payload.fill(0);
    status
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_exchange(length: u32) -> i32 {
    let mut bytes = match input(length) {
        Ok(bytes) => bytes,
        Err(error) => return refuse(error),
    };
    let status = with_state(|client| {
        let frame = decode_session_frame(&bytes, CAPACITY as u32, CAPACITY as u32)
            .map_err(|error| format!("decode worker pool-member frame: {error:?}"))?;
        let session = client
            .sessions
            .iter_mut()
            .find(|session| session.binding.identity() == frame.identity)
            .ok_or_else(|| "worker frame names no admitted pool-member session".to_string())?;
        let message = frame.message;
        let label = message_name(message);
        session
            .machine
            .admit_inbound(frame)
            .map_err(|error| format!("admit worker pool-member frame: {error:?}"))?;
        let mut responses = Vec::new();
        let mut result = None;
        if let SessionMessage::Offered { sequence, payload } = message {
            if session.direction != PoolMemberSessionDirection::Output || client.result_received {
                return Err("worker offered an unexpected pool-member result".into());
            }
            result = Some(payload.to_vec());
            client.result_received = true;
            for response in [
                SessionMessage::Accepted { sequence },
                SessionMessage::Delivered { sequence },
            ] {
                let frame = session.binding.frame(response);
                session.machine.admit_outbound(frame).map_err(|error| {
                    format!("admit browser pool-member acknowledgement: {error:?}")
                })?;
                responses.push(encode(&session.binding, frame)?);
            }
        }
        write(&serde_json::json!({
            "schema": "conduit.browser/pool-member-client-exchange@1",
            "message": label,
            "responses": responses,
            "result": result,
            "active": session.machine.is_active(),
        }))
    });
    bytes.fill(0);
    status
}

#[no_mangle]
pub extern "C" fn conduit_browser_pool_member_close() {
    STATE.with(|state| *state.borrow_mut() = None);
    OUTPUT.with(|output| output.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_start_and_unstarted_offer_fail_closed() {
        conduit_browser_pool_member_close();
        INPUT.with(|input| input.borrow_mut()[..2].copy_from_slice(b"{}"));
        assert_eq!(conduit_browser_pool_member_start(2), REFUSED);
        assert!(STATE.with(|state| state.borrow().is_none()));

        INPUT.with(|input| input.borrow_mut()[0] = b'x');
        assert_eq!(conduit_browser_pool_member_offer(1), REFUSED);
        let refusal: serde_json::Value = OUTPUT.with(|output| {
            serde_json::from_slice(&output.borrow()).expect("refusal is bounded JSON")
        });
        assert_eq!(
            refusal["schema"],
            "conduit.browser/pool-member-client-refusal@1"
        );
    }
}
