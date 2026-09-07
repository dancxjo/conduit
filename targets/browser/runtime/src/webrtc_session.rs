//! Browser-owned endpoint for one exact WebRTC DataChannel session.
//!
//! JavaScript owns transport readiness and pressure. This module owns only the
//! shared, plan-bound `conduit-wire` admission state; every page instantiates a
//! separate WASM module and therefore a separate endpoint.

use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition, WireError,
};
use std::cell::RefCell;

#[path = "webrtc_session/plan.rs"]
mod plan;
#[path = "webrtc_session/start.rs"]
mod start;

use plan::exact_binding;

const FRAME_CAPACITY: usize = 128 * 1_024;
const PAYLOAD_CAPACITY: u32 = 64 * 1_024;
const STATUS_HANDSHAKE: i32 = 0;
const STATUS_ACTIVE: i32 = 1;
const STATUS_TERMINAL: i32 = 2;
const STATUS_TERMINATING: i32 = 3;
const ERROR_NOT_STARTED: i32 = -200;
const ERROR_OUTPUT_PENDING: i32 = -201;
const ERROR_STAGE: i32 = -202;
const EVENT_NONE: i32 = 0;
const EVENT_OFFERED: i32 = 1;
const EVENT_PRESSURE: i32 = 2;
const EVENT_ACCEPTED: i32 = 3;
const EVENT_DELIVERED: i32 = 4;
const EVENT_INPUT_CLOSED: i32 = 5;
const EVENT_TERMINAL: i32 = 6;

thread_local! {
    static ENDPOINT: RefCell<Option<BrowserWebRtcSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; FRAME_CAPACITY]> = const { RefCell::new([0; FRAME_CAPACITY]) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    PeerHello,
    PeerReady,
    Active,
    LocalTerminal,
    PeerTerminal,
    Terminal,
}

struct BrowserWebRtcSession {
    binding: SessionBinding,
    machine: SessionMachine,
    role: SessionRole,
    output: Vec<u8>,
    output_len: usize,
    received: Vec<u8>,
    received_len: usize,
    received_sequence: Option<u64>,
    last_event: i32,
    stage: Stage,
}

impl BrowserWebRtcSession {
    fn new(role: SessionRole, binding: SessionBinding) -> Result<Self, WireError> {
        if binding.limits.maximum_payload_bytes > PAYLOAD_CAPACITY
            || binding.limits.maximum_buffered_bytes > PAYLOAD_CAPACITY
        {
            return Err(WireError::OversizedPayload);
        }
        if binding.attachment.limits.maximum_frame_bytes > FRAME_CAPACITY as u32 {
            return Err(WireError::OversizedFrame);
        }
        if binding.limits.maximum_in_flight_items > 1 {
            return Err(WireError::InvalidSession);
        }
        let machine = SessionMachine::new(binding.clone(), role)?;
        let mut session = Self {
            binding,
            machine,
            role,
            output: vec![0; FRAME_CAPACITY],
            output_len: 0,
            received: vec![0; PAYLOAD_CAPACITY as usize],
            received_len: 0,
            received_sequence: None,
            last_event: EVENT_NONE,
            stage: Stage::PeerHello,
        };
        let binding = session.binding.clone();
        let hello = binding.hello_frame();
        session.machine.admit_outbound(hello)?;
        session.write(hello)?;
        Ok(session)
    }

    fn write(&mut self, frame: conduit_wire::SessionFrame<'_>) -> Result<(), WireError> {
        self.output_len = encode_session_frame_into(
            frame,
            &mut self.output,
            PAYLOAD_CAPACITY,
            FRAME_CAPACITY as u32,
        )?;
        Ok(())
    }

    fn ingest(&mut self, bytes: &[u8]) -> Result<i32, WireError> {
        if self.output_len != 0 {
            return Ok(ERROR_OUTPUT_PENDING);
        }
        let frame = decode_session_frame(bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)?;
        let message = frame.message;
        let supported = matches!(
            (self.stage, message),
            (Stage::PeerHello, SessionMessage::Hello(_))
                | (Stage::PeerReady, SessionMessage::Ready)
                | (Stage::Active, SessionMessage::InputClosed { .. })
                | (Stage::Active, SessionMessage::Offered { .. })
                | (Stage::Active, SessionMessage::Pressure { .. })
                | (Stage::Active, SessionMessage::Accepted { .. })
                | (Stage::Active, SessionMessage::Delivered { .. })
                | (Stage::Active, SessionMessage::Terminal { .. })
                | (Stage::LocalTerminal, SessionMessage::Terminal { .. })
                | (Stage::PeerTerminal, _)
                | (Stage::Terminal, _)
        );
        if !supported {
            return Ok(ERROR_STAGE);
        }
        if matches!(message, SessionMessage::Offered { .. }) && self.role != SessionRole::Sink {
            return Ok(ERROR_STAGE);
        }
        self.machine.admit_inbound(frame)?;
        match (self.stage, message) {
            (Stage::PeerHello, SessionMessage::Hello(_)) => {
                let binding = self.binding.clone();
                let ready = binding.frame(SessionMessage::Ready);
                self.machine.admit_outbound(ready)?;
                self.write(ready)?;
                self.stage = Stage::PeerReady;
                Ok(STATUS_HANDSHAKE)
            }
            (Stage::PeerReady, SessionMessage::Ready) => {
                self.stage = Stage::Active;
                Ok(STATUS_ACTIVE)
            }
            (Stage::Active, SessionMessage::InputClosed { .. }) => {
                self.last_event = EVENT_INPUT_CLOSED;
                Ok(STATUS_ACTIVE)
            }
            (Stage::Active, SessionMessage::Offered { sequence, payload }) => {
                self.received[..payload.len()].copy_from_slice(payload);
                self.received_len = payload.len();
                self.received_sequence = Some(sequence);
                let binding = self.binding.clone();
                let accepted = binding.frame(SessionMessage::Accepted { sequence });
                self.machine.admit_outbound(accepted)?;
                self.write(accepted)?;
                self.last_event = EVENT_OFFERED;
                Ok(STATUS_ACTIVE)
            }
            (Stage::Active, SessionMessage::Pressure { .. }) => {
                self.last_event = EVENT_PRESSURE;
                Ok(STATUS_ACTIVE)
            }
            (Stage::Active, SessionMessage::Accepted { .. }) => {
                self.last_event = EVENT_ACCEPTED;
                Ok(STATUS_ACTIVE)
            }
            (Stage::Active, SessionMessage::Delivered { .. }) => {
                self.last_event = EVENT_DELIVERED;
                Ok(STATUS_ACTIVE)
            }
            (Stage::Active, SessionMessage::Terminal { .. }) => {
                self.last_event = EVENT_TERMINAL;
                self.stage = Stage::PeerTerminal;
                Ok(STATUS_TERMINATING)
            }
            (Stage::LocalTerminal, SessionMessage::Terminal { .. }) => {
                self.last_event = EVENT_TERMINAL;
                self.stage = Stage::Terminal;
                Ok(STATUS_TERMINAL)
            }
            _ => Ok(ERROR_STAGE),
        }
    }

    fn close_input(&mut self) -> Result<i32, WireError> {
        if self.output_len != 0 || self.stage != Stage::Active {
            return Ok(ERROR_STAGE);
        }
        let binding = self.binding.clone();
        let closed = binding.frame(SessionMessage::InputClosed {
            final_sequence: self.machine.next_sequence(),
        });
        self.machine.admit_outbound(closed)?;
        self.write(closed)?;
        Ok(STATUS_ACTIVE)
    }

    fn offer(&mut self, payload: &[u8]) -> Result<i32, WireError> {
        if self.output_len != 0 || self.stage != Stage::Active || self.role != SessionRole::Source {
            return Ok(ERROR_STAGE);
        }
        let binding = self.binding.clone();
        let offered = binding.frame(SessionMessage::Offered {
            sequence: self.machine.next_sequence(),
            payload,
        });
        self.machine.admit_outbound(offered)?;
        self.write(offered)?;
        Ok(STATUS_ACTIVE)
    }

    fn deliver(&mut self) -> Result<i32, WireError> {
        if self.output_len != 0 || self.stage != Stage::Active || self.role != SessionRole::Sink {
            return Ok(ERROR_STAGE);
        }
        let Some(sequence) = self.received_sequence else {
            return Ok(ERROR_STAGE);
        };
        let binding = self.binding.clone();
        let delivered = binding.frame(SessionMessage::Delivered { sequence });
        self.machine.admit_outbound(delivered)?;
        self.write(delivered)?;
        self.received.fill(0);
        self.received_len = 0;
        self.received_sequence = None;
        Ok(STATUS_ACTIVE)
    }

    fn pressure(&mut self, bytes: &[u8]) -> Result<i32, WireError> {
        if self.output_len != 0 || self.stage != Stage::Active || self.role != SessionRole::Sink {
            return Ok(ERROR_STAGE);
        }
        let frame = decode_session_frame(bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)?;
        let SessionMessage::Offered { sequence, .. } = frame.message else {
            return Ok(ERROR_STAGE);
        };
        self.machine.admit_inbound(frame)?;
        let binding = self.binding.clone();
        let pressure = binding.frame(SessionMessage::Pressure { sequence });
        self.machine.admit_outbound(pressure)?;
        self.write(pressure)?;
        Ok(STATUS_ACTIVE)
    }

    fn finish(&mut self) -> Result<i32, WireError> {
        if self.output_len != 0 || !matches!(self.stage, Stage::Active | Stage::PeerTerminal) {
            return Ok(ERROR_STAGE);
        }
        let binding = self.binding.clone();
        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: self.machine.next_sequence(),
        });
        self.machine.admit_outbound(terminal)?;
        self.write(terminal)?;
        if self.stage == Stage::PeerTerminal {
            self.stage = Stage::Terminal;
            Ok(STATUS_TERMINAL)
        } else {
            self.stage = Stage::LocalTerminal;
            Ok(STATUS_TERMINATING)
        }
    }
}

fn with_endpoint(action: impl FnOnce(&mut BrowserWebRtcSession) -> i32) -> i32 {
    ENDPOINT.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.as_mut().map(action).unwrap_or(ERROR_NOT_STARTED)
    })
}

fn wire_error(error: WireError) -> i32 {
    match error {
        WireError::PlanMismatch => -210,
        WireError::BootMismatch => -211,
        WireError::ConnectionMismatch => -212,
        WireError::ValueContractMismatch => -213,
        WireError::SessionEpochMismatch => -214,
        WireError::OversizedFrame | WireError::OversizedPayload => -215,
        WireError::LateFrame => -216,
        WireError::DuplicateFrame => -217,
        WireError::ReorderedFrame => -218,
        WireError::InvalidState => -220,
        _ => -219,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_input_capacity() -> u32 {
    FRAME_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_maximum_frame_bytes() -> u32 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.binding.attachment.limits.maximum_frame_bytes)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_maximum_payload_bytes() -> u32 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.binding.limits.maximum_payload_bytes)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_maximum_in_flight_items() -> u32 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| u32::from(endpoint.binding.limits.maximum_in_flight_items))
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_output_ptr() -> *const u8 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.output.as_ptr())
            .unwrap_or(core::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_output_len() -> u32 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.output_len as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_clear_output() -> i32 {
    with_endpoint(|endpoint| {
        endpoint.output_len = 0;
        match endpoint.stage {
            Stage::Active => STATUS_ACTIVE,
            Stage::Terminal => STATUS_TERMINAL,
            Stage::LocalTerminal | Stage::PeerTerminal => STATUS_TERMINATING,
            _ => STATUS_HANDSHAKE,
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_ingest(length: u32) -> i32 {
    let length = length as usize;
    if length > FRAME_CAPACITY {
        return wire_error(WireError::OversizedFrame);
    }
    INPUT.with(|input| {
        let input = input.borrow();
        with_endpoint(|endpoint| endpoint.ingest(&input[..length]).unwrap_or_else(wire_error))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_finish() -> i32 {
    with_endpoint(|endpoint| endpoint.finish().unwrap_or_else(wire_error))
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_offer(length: u32) -> i32 {
    let length = length as usize;
    if length > PAYLOAD_CAPACITY as usize {
        return wire_error(WireError::OversizedPayload);
    }
    INPUT.with(|input| {
        let input = input.borrow();
        with_endpoint(|endpoint| endpoint.offer(&input[..length]).unwrap_or_else(wire_error))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_pressure(length: u32) -> i32 {
    let length = length as usize;
    if length > FRAME_CAPACITY {
        return wire_error(WireError::OversizedFrame);
    }
    INPUT.with(|input| {
        let input = input.borrow();
        with_endpoint(|endpoint| {
            endpoint
                .pressure(&input[..length])
                .unwrap_or_else(wire_error)
        })
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_deliver() -> i32 {
    with_endpoint(|endpoint| endpoint.deliver().unwrap_or_else(wire_error))
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_value_ptr() -> *const u8 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.received.as_ptr())
            .unwrap_or(core::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_value_len() -> u32 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.received_len as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_next_sequence() -> u64 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.machine.next_sequence())
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_last_event() -> i32 {
    ENDPOINT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|endpoint| endpoint.last_event)
            .unwrap_or(EVENT_NONE)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_close_input() -> i32 {
    with_endpoint(|endpoint| endpoint.close_input().unwrap_or_else(wire_error))
}

#[cfg(test)]
#[path = "webrtc_session/tests.rs"]
mod tests;
