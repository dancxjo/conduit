//! Finite production ABI for one browser-owned remote Plan fragment.

use super::*;
use crate::form_runner::engine::{BrowserHostEffect, DriveStatus};
use conduit_kernel::RemoteEndpointId;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole,
};
use serde::Deserialize;
use std::cell::RefCell;

const CAPACITY: usize = 256 * 1024;
const OK: i32 = 0;
const EFFECT: i32 = 1;
const WAITING: i32 = 2;
const QUIESCENT: i32 = 3;
const COMPLETE: i32 = 4;
const OFFER: i32 = 5;
const ACCEPTED: i32 = 6;
const PRESSURE: i32 = 7;
const TERMINAL: i32 = 8;
const REFUSED: i32 = -1;

thread_local! {
    static INPUT: RefCell<Box<[u8; CAPACITY]>> = RefCell::new(Box::new([0; CAPACITY]));
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(CAPACITY));
    static STATE: RefCell<Option<RemoteAbi>> = const { RefCell::new(None) };
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    plan: conduit_core::Plan,
    host: conduit_core::HostAdvertisement,
    session_hellos: Vec<Vec<u8>>,
    observations: Vec<conduit_core::ResourceObservation>,
}

struct RemoteAbi {
    execution: RemoteExecution,
    pending: Option<PendingHostEffect>,
    complete: bool,
    cancelled: bool,
    active_play_id: conduit_core::ActivePlayId,
    endpoints: Vec<RemoteEndpointId>,
    sessions: Vec<RemoteSession>,
}

struct RemoteSession {
    endpoint: RemoteEndpointId,
    direction: conduit_plan_lowering::lowering::RemoteCordDirection,
    binding: SessionBinding,
    machine: SessionMachine,
}

fn refuse(message: impl Into<String>) -> i32 {
    let message = message.into();
    let _ = write_json(&serde_json::json!({
        "schema": "conduit.browser/remote-fragment-refusal@1",
        "message": message,
    }));
    REFUSED
}

fn write_json(value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > CAPACITY {
        return Err("remote fragment output exceeds its finite bound".into());
    }
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        output.clear();
        output.extend_from_slice(&bytes);
    });
    Ok(())
}

fn input(length: u32) -> Result<Vec<u8>, String> {
    let length = usize::try_from(length).map_err(|_| "remote input length overflow")?;
    if length > CAPACITY {
        return Err("remote fragment input exceeds its finite bound".into());
    }
    Ok(INPUT.with(|input| input.borrow()[..length].to_vec()))
}

fn with_state(action: impl FnOnce(&mut RemoteAbi) -> Result<i32, String>) -> i32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return refuse("remote fragment is not started");
        };
        action(state).unwrap_or_else(refuse)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_input_capacity() -> u32 {
    CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_output_ptr() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_output_len() -> u32 {
    OUTPUT.with(|output| output.borrow().len() as u32)
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_start(length: u32) -> i32 {
    STATE.with(|state| *state.borrow_mut() = None);
    OUTPUT.with(|output| output.borrow_mut().clear());
    let result = (|| -> Result<(), String> {
        let mut bytes = input(length)?;
        let start: Start = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode remote fragment start: {error}"))?;
        bytes.fill(0);
        if start.session_hellos.is_empty() || start.session_hellos.len() > 16 {
            return Err("remote fragment requires one to sixteen exact sessions".into());
        }
        let bindings = start
            .session_hellos
            .iter()
            .map(|hello| {
                let frame = decode_session_frame(hello, CAPACITY as u32, CAPACITY as u32)
                    .map_err(|error| format!("decode remote grant: {error:?}"))?;
                SessionBinding::from_hello_frame(frame)
                    .map_err(|error| format!("reconstruct remote grant: {error:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let active_play_id = bindings
            .iter()
            .map(|binding| {
                if binding.source.host_id == start.host.host_id
                    && binding.source.boot_id == start.host.boot_id
                {
                    Ok(binding.source_active_play_id.clone())
                } else if binding.sink.host_id == start.host.host_id
                    && binding.sink.boot_id == start.host.boot_id
                {
                    Ok(binding.sink_active_play_id.clone())
                } else {
                    Err("remote grant does not name the current browser Host and Boot".to_string())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let active_play_id = active_play_id
            .first()
            .filter(|first| active_play_id.iter().all(|current| current == *first))
            .cloned()
            .ok_or_else(|| "remote grants differ on browser Active Play identity".to_string())?;
        let execution = RemoteExecution::prepare(
            &start.plan,
            &start.host,
            &bindings,
            &active_play_id,
            &start.observations,
        )?;
        let endpoints = bindings
            .iter()
            .map(|binding| {
                execution
                    .endpoint_for(binding)
                    .ok_or_else(|| "remote grant has no lowered endpoint".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut sessions = Vec::with_capacity(bindings.len());
        let mut initial_frames = Vec::with_capacity(bindings.len() * 2);
        for ((binding, hello), endpoint) in bindings
            .into_iter()
            .zip(start.session_hellos.iter())
            .zip(endpoints.iter().copied())
        {
            let direction = execution.direction(endpoint)?;
            let role = if direction == conduit_plan_lowering::lowering::RemoteCordDirection::Egress
            {
                SessionRole::Source
            } else {
                SessionRole::Sink
            };
            let mut machine = SessionMachine::new(binding.clone(), role)
                .map_err(|error| format!("prepare browser remote session: {error:?}"))?;
            let outbound_hello = binding.hello_frame();
            machine
                .admit_outbound(outbound_hello)
                .map_err(|error| format!("admit browser remote hello: {error:?}"))?;
            let inbound_hello = decode_session_frame(hello, CAPACITY as u32, CAPACITY as u32)
                .map_err(|error| format!("decode browser remote hello: {error:?}"))?;
            machine
                .admit_inbound(inbound_hello)
                .map_err(|error| format!("admit native remote hello: {error:?}"))?;
            let ready = binding.frame(SessionMessage::Ready);
            machine
                .admit_outbound(ready)
                .map_err(|error| format!("admit browser remote readiness: {error:?}"))?;
            initial_frames.push(encode_frame(&binding, outbound_hello)?);
            initial_frames.push(encode_frame(&binding, ready)?);
            sessions.push(RemoteSession {
                endpoint,
                direction,
                binding,
                machine,
            });
        }
        write_json(&serde_json::json!({
            "schema": "conduit.browser/remote-fragment-started@1",
            "plan_id": start.plan.plan_id,
            "active_play_id": active_play_id,
            "endpoints": endpoints.iter().map(|endpoint| endpoint.0).collect::<Vec<_>>(),
            "egress_endpoints": sessions.iter().filter_map(|session| {
                (session.direction == conduit_plan_lowering::lowering::RemoteCordDirection::Egress)
                    .then_some(session.endpoint.0)
            }).collect::<Vec<_>>(),
            "initial_frames": initial_frames,
        }))?;
        STATE.with(|state| {
            *state.borrow_mut() = Some(RemoteAbi {
                execution,
                pending: None,
                complete: false,
                cancelled: false,
                active_play_id,
                endpoints,
                sessions,
            });
        });
        Ok(())
    })();
    result.map_or_else(refuse, |()| OK)
}

fn encode_frame(
    binding: &SessionBinding,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0; binding.attachment.limits.maximum_frame_bytes as usize];
    let length = encode_session_frame_into(
        frame,
        &mut bytes,
        binding.limits.maximum_payload_bytes,
        binding.attachment.limits.maximum_frame_bytes,
    )
    .map_err(|error| format!("encode browser remote frame: {error:?}"))?;
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
pub extern "C" fn conduit_browser_remote_exchange(length: u32) -> i32 {
    with_state(|state| {
        let mut bytes = input(length)?;
        let frame = decode_session_frame(&bytes, CAPACITY as u32, CAPACITY as u32)
            .map_err(|error| format!("decode browser remote frame: {error:?}"))?;
        let session = state
            .sessions
            .iter_mut()
            .find(|session| session.binding.identity() == frame.identity)
            .ok_or_else(|| "browser remote frame names no admitted session".to_string())?;
        let message = frame.message;
        let message_label = message_name(message);
        session
            .machine
            .admit_inbound(frame)
            .map_err(|error| format!("admit browser remote frame: {error:?}"))?;
        let mut responses = Vec::new();
        match message {
            SessionMessage::Ready => {}
            SessionMessage::Offered { sequence, payload } => {
                match state.execution.admit(session.endpoint, sequence, payload)? {
                    RemoteIngressOutcome::Accepted { sequence: accepted }
                        if accepted == sequence =>
                    {
                        for message in [
                            SessionMessage::Accepted { sequence },
                            SessionMessage::Delivered { sequence },
                        ] {
                            let response = session.binding.frame(message);
                            session.machine.admit_outbound(response).map_err(|error| {
                                format!("admit browser remote response: {error:?}")
                            })?;
                            responses.push(encode_frame(&session.binding, response)?);
                        }
                    }
                    RemoteIngressOutcome::Full {
                        sequence: pressured,
                    } if pressured == sequence => {
                        let response = session.binding.frame(SessionMessage::Pressure { sequence });
                        session
                            .machine
                            .admit_outbound(response)
                            .map_err(|error| format!("admit browser remote pressure: {error:?}"))?;
                        responses.push(encode_frame(&session.binding, response)?);
                    }
                    _ => return Err("browser remote ingress sequence differs".into()),
                }
            }
            SessionMessage::Accepted { sequence } => {
                state.execution.accepted(session.endpoint, sequence)?;
            }
            SessionMessage::Delivered { sequence } => {
                state.execution.delivered(session.endpoint, sequence)?;
            }
            SessionMessage::Pressure { .. } => {
                if session.direction != conduit_plan_lowering::lowering::RemoteCordDirection::Egress
                {
                    return Err("browser remote pressure names an ingress endpoint".into());
                }
            }
            SessionMessage::InputClosed { .. } => {
                if session.direction
                    != conduit_plan_lowering::lowering::RemoteCordDirection::Ingress
                {
                    return Err("browser remote input close names an egress endpoint".into());
                }
                state.execution.close_ingress(session.endpoint)?;
            }
            SessionMessage::Terminal { .. } => {}
            SessionMessage::Cancelled { .. } | SessionMessage::Failed { .. } => {
                if !state.cancelled {
                    state.execution.cancel()?;
                    state.cancelled = true;
                }
            }
            SessionMessage::Hello(_) => {
                return Err("browser remote session message is out of order".into());
            }
        }
        bytes.fill(0);
        write_json(&serde_json::json!({
            "schema": "conduit.browser/remote-session-exchange@1",
            "endpoint": session.endpoint.0,
            "message": message_label,
            "responses": responses,
            "active": session.machine.is_active(),
        }))?;
        Ok(OK)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_drive() -> i32 {
    with_state(|state| {
        if state.pending.is_some() {
            return Err("remote Host effect is already pending".into());
        }
        match state.execution.drive()? {
            DriveStatus::Effect(pending) => {
                let placement = state
                    .execution
                    .fragment
                    .placements
                    .get(usize::from(pending.request.node.0))
                    .ok_or("remote effect has no exact placement")?;
                let mut effect = serde_json::json!({
                    "schema": "conduit.browser/remote-host-effect@1",
                    "active_play_id": state.active_play_id,
                    "placement_id": placement.placement_id,
                    "host_id": placement.host_id,
                    "boot_id": placement.boot_id,
                });
                let object = effect.as_object_mut().unwrap();
                match &pending.effect {
                    BrowserHostEffect::AudioCue => {
                        object.insert("effect_kind".into(), "audio-cue".into());
                    }
                    BrowserHostEffect::AudioCapture => {
                        object.insert("effect_kind".into(), "audio-capture".into());
                    }
                    BrowserHostEffect::PcmPlayback { frame } => {
                        object.insert("effect_kind".into(), "pcm-playback".into());
                        object.insert(
                            "maximum_gain_millionths".into(),
                            crate::installed_browser::audio_io::MAXIMUM_SAFE_GAIN_MILLIONTHS.into(),
                        );
                        object.insert("frame_hex".into(), hex_bytes(frame).into());
                    }
                    BrowserHostEffect::PitchTone { hertz } => {
                        object.insert("effect_kind".into(), "pitch-tone".into());
                        object.insert("hertz".into(), (*hertz).into());
                    }
                    BrowserHostEffect::ClockObservation => {
                        object.insert("effect_kind".into(), "clock-observation".into());
                    }
                    BrowserHostEffect::Timer { duration_millis } => {
                        object.insert("effect_kind".into(), "timer".into());
                        object.insert("duration_millis".into(), (*duration_millis).into());
                    }
                    BrowserHostEffect::Snapshot { publish } => {
                        object.insert("effect_kind".into(), "snapshot".into());
                        object.insert("publish".into(), (*publish).into());
                    }
                    BrowserHostEffect::KeyEvent => {
                        object.insert("effect_kind".into(), "key-event".into());
                    }
                    BrowserHostEffect::PointerEvent => {
                        object.insert("effect_kind".into(), "pointer-event".into());
                    }
                    BrowserHostEffect::ButtonTransition => {
                        object.insert("effect_kind".into(), "button-transition".into());
                    }
                    BrowserHostEffect::ApplicationEvent => {
                        object.insert("effect_kind".into(), "application-event".into());
                    }
                    BrowserHostEffect::Manifestation(manifestation) => {
                        object.insert("effect_kind".into(), "manifestation".into());
                        object.insert("presentation_kind".into(), manifestation.kind_id.into());
                        object.insert(
                            "canonical_value".into(),
                            manifestation.canonical_value.clone().into(),
                        );
                    }
                }
                write_json(&effect)?;
                state.pending = Some(pending);
                Ok(EFFECT)
            }
            DriveStatus::Waiting { pending_effects } => {
                write_json(&serde_json::json!({
                    "schema": "conduit.browser/remote-fragment-progress@1",
                    "state": "waiting",
                    "pending_effects": pending_effects,
                }))?;
                Ok(WAITING)
            }
            DriveStatus::Quiescent => Ok(QUIESCENT),
            DriveStatus::SemanticCompleted => {
                state.complete = true;
                Ok(COMPLETE)
            }
        }
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_complete_effect(length: u32, has_output: u32) -> i32 {
    with_state(|state| {
        let pending = state
            .pending
            .take()
            .ok_or_else(|| "remote fragment has no pending Host effect".to_string())?;
        let bytes = (has_output != 0).then(|| input(length)).transpose()?;
        state
            .execution
            .complete_effect(&pending, bytes.as_deref())?;
        Ok(OK)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_offer(endpoint: u16) -> i32 {
    with_state(
        |state| match state.execution.offer(RemoteEndpointId(endpoint))? {
            Some(offer) => {
                write_json(&serde_json::json!({
                    "schema": "conduit.browser/remote-offer@1",
                    "endpoint": endpoint,
                    "sequence": offer.sequence,
                    "payload": offer.payload,
                }))?;
                Ok(OFFER)
            }
            None => Ok(WAITING),
        },
    )
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_offer_frame(endpoint: u16) -> i32 {
    with_state(|state| {
        let endpoint = RemoteEndpointId(endpoint);
        let session = state
            .sessions
            .iter_mut()
            .find(|session| session.endpoint == endpoint)
            .ok_or_else(|| "unknown remote endpoint".to_string())?;
        if session.direction != conduit_plan_lowering::lowering::RemoteCordDirection::Egress {
            return Err("browser remote offer names an ingress endpoint".into());
        }
        if !session.machine.is_active() {
            return Err("browser remote session is not active".into());
        }
        match state.execution.offer(endpoint)? {
            Some(offer) => {
                let frame = session.binding.frame(SessionMessage::Offered {
                    sequence: offer.sequence,
                    payload: &offer.payload,
                });
                session
                    .machine
                    .admit_outbound(frame)
                    .map_err(|error| format!("admit browser remote offer: {error:?}"))?;
                write_json(&serde_json::json!({
                    "schema": "conduit.browser/remote-session-frame@1",
                    "frame": encode_frame(&session.binding, frame)?,
                }))?;
                Ok(OFFER)
            }
            None => Ok(WAITING),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_admit(endpoint: u16, sequence: u64, length: u32) -> i32 {
    with_state(|state| {
        let mut bytes = input(length)?;
        let outcome = state
            .execution
            .admit(RemoteEndpointId(endpoint), sequence, &bytes)?;
        bytes.fill(0);
        Ok(match outcome {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => ACCEPTED,
            conduit_kernel::scheduler::RemoteIngressOutcome::Full { .. } => PRESSURE,
        })
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_accepted(endpoint: u16, sequence: u64) -> i32 {
    with_state(|state| {
        state
            .execution
            .accepted(RemoteEndpointId(endpoint), sequence)?;
        Ok(OK)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_delivered(endpoint: u16, sequence: u64) -> i32 {
    with_state(|state| {
        state
            .execution
            .delivered(RemoteEndpointId(endpoint), sequence)?;
        Ok(OK)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_terminal(endpoint: u16) -> i32 {
    with_state(|state| {
        Ok(if state.execution.terminal(RemoteEndpointId(endpoint))? {
            TERMINAL
        } else {
            WAITING
        })
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_finish() -> i32 {
    with_state(|state| {
        if !state.complete || state.pending.is_some() {
            return Err("browser remote fragment has not completed".into());
        }
        for session in &state.sessions {
            if session.direction == conduit_plan_lowering::lowering::RemoteCordDirection::Egress {
                if !state.execution.terminal(session.endpoint)? {
                    return Err("browser remote egress is not terminal".into());
                }
            } else if !session.machine.checkpoint().input_closed {
                return Err("browser remote ingress has not observed input close".into());
            }
        }
        let mut frames = Vec::with_capacity(state.sessions.len() * 2);
        for session in &mut state.sessions {
            let final_sequence = session.machine.next_sequence();
            if session.direction == conduit_plan_lowering::lowering::RemoteCordDirection::Egress {
                let closed = session
                    .binding
                    .frame(SessionMessage::InputClosed { final_sequence });
                session
                    .machine
                    .admit_outbound(closed)
                    .map_err(|error| format!("admit browser remote input close: {error:?}"))?;
                frames.push(encode_frame(&session.binding, closed)?);
            }
            let terminal = session.binding.frame(SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Completed,
                final_sequence,
            });
            session
                .machine
                .admit_outbound(terminal)
                .map_err(|error| format!("admit browser remote terminal: {error:?}"))?;
            frames.push(encode_frame(&session.binding, terminal)?);
        }
        write_json(&serde_json::json!({
            "schema": "conduit.browser/remote-session-finished@1",
            "frames": frames,
        }))?;
        Ok(TERMINAL)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_cancel() -> i32 {
    with_state(|state| {
        if !state.cancelled {
            state.execution.cancel()?;
            state.cancelled = true;
        }
        state.pending = None;
        Ok(OK)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_cancel_frames(code: u32) -> i32 {
    with_state(|state| {
        let code = u16::try_from(code)
            .ok()
            .filter(|code| *code != 0)
            .ok_or_else(|| "browser remote cancellation code is invalid".to_string())?;
        if !state.cancelled {
            state.execution.cancel()?;
            state.cancelled = true;
        }
        state.pending = None;
        let mut frames = Vec::with_capacity(state.sessions.len() * 2);
        for session in &mut state.sessions {
            let cancelled = session.binding.frame(SessionMessage::Cancelled { code });
            session
                .machine
                .admit_outbound(cancelled)
                .map_err(|error| format!("admit browser remote cancellation: {error:?}"))?;
            frames.push(encode_frame(&session.binding, cancelled)?);
            let terminal = session.binding.frame(SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Cancelled,
                final_sequence: session.machine.next_sequence(),
            });
            session
                .machine
                .admit_outbound(terminal)
                .map_err(|error| format!("admit browser remote cancelled terminal: {error:?}"))?;
            frames.push(encode_frame(&session.binding, terminal)?);
        }
        write_json(&serde_json::json!({
            "schema": "conduit.browser/remote-session-cancelled@1",
            "frames": frames,
        }))?;
        Ok(TERMINAL)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_endpoint_count() -> u16 {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.endpoints.len() as u16)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_wire::encode_session_frame_into;

    #[test]
    fn exact_grant_starts_production_abi_and_projects_first_effect() {
        let (plan, source, _, binding) = crate::form_runner::remote_execution::tests::fixture();
        let mut hello = vec![0; binding.attachment.limits.maximum_frame_bytes as usize];
        let length = encode_session_frame_into(
            binding.hello_frame(),
            &mut hello,
            binding.limits.maximum_payload_bytes,
            binding.attachment.limits.maximum_frame_bytes,
        )
        .unwrap();
        hello.truncate(length);
        let observations = crate::form_runner::remote_execution::tests::observations(&source);
        let start = serde_json::to_vec(&serde_json::json!({
            "plan": plan,
            "host": source,
            "session_hellos": [hello],
            "observations": observations,
        }))
        .unwrap();
        INPUT.with(|input| input.borrow_mut()[..start.len()].copy_from_slice(&start));

        assert_eq!(conduit_browser_remote_start(start.len() as u32), OK);
        assert_eq!(conduit_browser_remote_endpoint_count(), 1);
        let started = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(
            started["schema"],
            "conduit.browser/remote-fragment-started@1"
        );
        let initial_frames =
            serde_json::from_value::<Vec<Vec<u8>>>(started["initial_frames"].clone()).unwrap();
        assert_eq!(initial_frames.len(), 2);

        let mut peer = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
        peer.admit_outbound(binding.hello_frame()).unwrap();
        for bytes in &initial_frames {
            let frame = decode_session_frame(bytes, CAPACITY as u32, CAPACITY as u32).unwrap();
            peer.admit_inbound(frame).unwrap();
        }
        let ready = binding.frame(SessionMessage::Ready);
        peer.admit_outbound(ready).unwrap();
        assert!(peer.is_active());
        let ready = encode_frame(&binding, ready).unwrap();
        INPUT.with(|input| input.borrow_mut()[..ready.len()].copy_from_slice(&ready));
        assert_eq!(conduit_browser_remote_exchange(ready.len() as u32), OK);
        let exchanged = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(
            exchanged["schema"],
            "conduit.browser/remote-session-exchange@1"
        );
        assert_eq!(exchanged["active"], true);
        assert_eq!(exchanged["responses"], serde_json::json!([]));

        assert_eq!(conduit_browser_remote_drive(), EFFECT);
        let output = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(output["schema"], "conduit.browser/remote-host-effect@1");
        assert_eq!(output["effect_kind"], "button-transition");

        let button = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        INPUT.with(|input| input.borrow_mut()[..button.len()].copy_from_slice(&button));
        assert_eq!(
            conduit_browser_remote_complete_effect(button.len() as u32, 1),
            OK
        );
        assert_eq!(conduit_browser_remote_drive(), WAITING);
        assert_eq!(conduit_browser_remote_offer_frame(0), OFFER);
        let offered = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(offered["schema"], "conduit.browser/remote-session-frame@1");
        let offered = serde_json::from_value::<Vec<u8>>(offered["frame"].clone()).unwrap();
        let frame = decode_session_frame(&offered, CAPACITY as u32, CAPACITY as u32).unwrap();
        assert!(matches!(
            frame.message,
            SessionMessage::Offered { sequence: 0, payload } if payload == button
        ));
        peer.admit_inbound(frame).unwrap();
        for message in [
            SessionMessage::Accepted { sequence: 0 },
            SessionMessage::Delivered { sequence: 0 },
        ] {
            let frame = binding.frame(message);
            peer.admit_outbound(frame).unwrap();
            let bytes = encode_frame(&binding, frame).unwrap();
            INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
            assert_eq!(conduit_browser_remote_exchange(bytes.len() as u32), OK);
        }
        assert_eq!(peer.next_sequence(), 1);
        assert_eq!(conduit_browser_remote_cancel_frames(7), TERMINAL);
        let cancelled = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(
            cancelled["schema"],
            "conduit.browser/remote-session-cancelled@1"
        );
        let frames = serde_json::from_value::<Vec<Vec<u8>>>(cancelled["frames"].clone()).unwrap();
        assert_eq!(frames.len(), 2);
        for bytes in frames {
            peer.admit_inbound(
                decode_session_frame(&bytes, CAPACITY as u32, CAPACITY as u32).unwrap(),
            )
            .unwrap();
        }
        for message in [
            SessionMessage::Cancelled { code: 7 },
            SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Cancelled,
                final_sequence: 1,
            },
        ] {
            let frame = binding.frame(message);
            peer.admit_outbound(frame).unwrap();
            let bytes = encode_frame(&binding, frame).unwrap();
            INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
            assert_eq!(conduit_browser_remote_exchange(bytes.len() as u32), OK);
        }
    }

    #[test]
    fn completed_remote_fragment_closes_input_and_exchanges_terminal_truth() {
        let (plan, source, _, binding) =
            crate::form_runner::remote_execution::tests::finite_fixture();
        let hello = encode_frame(&binding, binding.hello_frame()).unwrap();
        let observations = crate::form_runner::remote_execution::tests::observations(&source);
        let start = serde_json::to_vec(&serde_json::json!({
            "plan": plan,
            "host": source,
            "session_hellos": [hello],
            "observations": observations,
        }))
        .unwrap();
        INPUT.with(|input| input.borrow_mut()[..start.len()].copy_from_slice(&start));
        assert_eq!(conduit_browser_remote_start(start.len() as u32), OK);
        let started = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        let initial_frames =
            serde_json::from_value::<Vec<Vec<u8>>>(started["initial_frames"].clone()).unwrap();
        let mut peer = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
        peer.admit_outbound(binding.hello_frame()).unwrap();
        for bytes in initial_frames {
            peer.admit_inbound(
                decode_session_frame(&bytes, CAPACITY as u32, CAPACITY as u32).unwrap(),
            )
            .unwrap();
        }
        let ready = binding.frame(SessionMessage::Ready);
        peer.admit_outbound(ready).unwrap();
        let ready = encode_frame(&binding, ready).unwrap();
        INPUT.with(|input| input.borrow_mut()[..ready.len()].copy_from_slice(&ready));
        assert_eq!(conduit_browser_remote_exchange(ready.len() as u32), OK);

        assert_eq!(conduit_browser_remote_drive(), WAITING);
        assert_eq!(conduit_browser_remote_offer_frame(0), OFFER);
        let offered = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        let offered = serde_json::from_value::<Vec<u8>>(offered["frame"].clone()).unwrap();
        let offered = decode_session_frame(&offered, CAPACITY as u32, CAPACITY as u32).unwrap();
        assert!(matches!(
            offered.message,
            SessionMessage::Offered {
                sequence: 0,
                payload: b"hello"
            }
        ));
        peer.admit_inbound(offered).unwrap();
        for message in [
            SessionMessage::Accepted { sequence: 0 },
            SessionMessage::Delivered { sequence: 0 },
        ] {
            let frame = binding.frame(message);
            peer.admit_outbound(frame).unwrap();
            let bytes = encode_frame(&binding, frame).unwrap();
            INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
            assert_eq!(conduit_browser_remote_exchange(bytes.len() as u32), OK);
        }
        assert_eq!(conduit_browser_remote_drive(), COMPLETE);
        assert_eq!(conduit_browser_remote_finish(), TERMINAL);
        let finished = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(
            finished["schema"],
            "conduit.browser/remote-session-finished@1"
        );
        let frames = serde_json::from_value::<Vec<Vec<u8>>>(finished["frames"].clone()).unwrap();
        assert_eq!(frames.len(), 2);
        for bytes in frames {
            peer.admit_inbound(
                decode_session_frame(&bytes, CAPACITY as u32, CAPACITY as u32).unwrap(),
            )
            .unwrap();
        }
        assert_eq!(peer.checkpoint().next_sequence, 1);
        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: conduit_wire::SessionTerminalDisposition::Completed,
            final_sequence: 1,
        });
        peer.admit_outbound(terminal).unwrap();
        let terminal = encode_frame(&binding, terminal).unwrap();
        INPUT.with(|input| input.borrow_mut()[..terminal.len()].copy_from_slice(&terminal));
        assert_eq!(conduit_browser_remote_exchange(terminal.len() as u32), OK);
        let exchanged = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(exchanged["active"], false);
    }
}
