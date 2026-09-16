//! Finite production ABI for one browser-owned remote Plan fragment.

use super::*;
use crate::form_runner::engine::{BrowserHostEffect, DriveStatus};
use conduit_kernel::RemoteEndpointId;
use conduit_wire::{decode_session_frame, SessionBinding};
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
    active_play_id: conduit_core::ActivePlayId,
    observations: Vec<conduit_core::ResourceObservation>,
}

struct RemoteAbi {
    execution: RemoteExecution,
    pending: Option<PendingHostEffect>,
    active_play_id: conduit_core::ActivePlayId,
    endpoints: Vec<RemoteEndpointId>,
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
        let execution = RemoteExecution::prepare(
            &start.plan,
            &start.host,
            &bindings,
            &start.active_play_id,
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
        write_json(&serde_json::json!({
            "schema": "conduit.browser/remote-fragment-started@1",
            "plan_id": start.plan.plan_id,
            "active_play_id": start.active_play_id,
            "endpoints": endpoints.iter().map(|endpoint| endpoint.0).collect::<Vec<_>>(),
        }))?;
        STATE.with(|state| {
            *state.borrow_mut() = Some(RemoteAbi {
                execution,
                pending: None,
                active_play_id: start.active_play_id,
                endpoints,
            });
        });
        Ok(())
    })();
    result.map_or_else(refuse, |()| OK)
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
            DriveStatus::SemanticCompleted => Ok(COMPLETE),
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
pub extern "C" fn conduit_browser_remote_cancel() -> i32 {
    with_state(|state| {
        state.execution.cancel()?;
        state.pending = None;
        Ok(OK)
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
            "active_play_id": binding.source_active_play_id,
            "observations": observations,
        }))
        .unwrap();
        INPUT.with(|input| input.borrow_mut()[..start.len()].copy_from_slice(&start));

        assert_eq!(conduit_browser_remote_start(start.len() as u32), OK);
        assert_eq!(conduit_browser_remote_endpoint_count(), 1);
        assert_eq!(conduit_browser_remote_drive(), EFFECT);
        let output = OUTPUT
            .with(|output| serde_json::from_slice::<serde_json::Value>(&output.borrow()).unwrap());
        assert_eq!(output["schema"], "conduit.browser/remote-host-effect@1");
        assert_eq!(output["effect_kind"], "button-transition");
        assert_eq!(conduit_browser_remote_cancel(), OK);
    }
}
