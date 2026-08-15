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

use plan::exact_binding;

const FRAME_CAPACITY: usize = 1_024;
const PAYLOAD_CAPACITY: u32 = 16;
const STATUS_HANDSHAKE: i32 = 0;
const STATUS_ACTIVE: i32 = 1;
const STATUS_TERMINAL: i32 = 2;
const STATUS_TERMINATING: i32 = 3;
const ERROR_NOT_STARTED: i32 = -200;
const ERROR_OUTPUT_PENDING: i32 = -201;
const ERROR_STAGE: i32 = -202;

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
    output: [u8; FRAME_CAPACITY],
    output_len: usize,
    received: [u8; PAYLOAD_CAPACITY as usize],
    received_len: usize,
    received_sequence: Option<u64>,
    stage: Stage,
}

impl BrowserWebRtcSession {
    fn new(role: SessionRole, variant: u32) -> Result<Self, WireError> {
        let binding = exact_binding(variant)?;
        let machine = SessionMachine::new(binding.clone(), role)?;
        let mut session = Self {
            binding,
            machine,
            role,
            output: [0; FRAME_CAPACITY],
            output_len: 0,
            received: [0; PAYLOAD_CAPACITY as usize],
            received_len: 0,
            received_sequence: None,
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
            (Stage::Active, SessionMessage::InputClosed { .. }) => Ok(STATUS_ACTIVE),
            (Stage::Active, SessionMessage::Offered { sequence, payload }) => {
                self.received[..payload.len()].copy_from_slice(payload);
                self.received_len = payload.len();
                self.received_sequence = Some(sequence);
                let binding = self.binding.clone();
                let accepted = binding.frame(SessionMessage::Accepted { sequence });
                self.machine.admit_outbound(accepted)?;
                self.write(accepted)?;
                Ok(STATUS_ACTIVE)
            }
            (Stage::Active, SessionMessage::Pressure { .. })
            | (Stage::Active, SessionMessage::Accepted { .. })
            | (Stage::Active, SessionMessage::Delivered { .. }) => Ok(STATUS_ACTIVE),
            (Stage::Active, SessionMessage::Terminal { .. }) => {
                self.stage = Stage::PeerTerminal;
                Ok(STATUS_TERMINATING)
            }
            (Stage::LocalTerminal, SessionMessage::Terminal { .. }) => {
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
pub extern "C" fn conduit_browser_webrtc_session_start(role: u32, variant: u32) -> i32 {
    let role = match role {
        0 => SessionRole::Source,
        1 => SessionRole::Sink,
        _ => return ERROR_STAGE,
    };
    match BrowserWebRtcSession::new(role, variant) {
        Ok(endpoint) => {
            ENDPOINT.with(|slot| *slot.borrow_mut() = Some(endpoint));
            STATUS_HANDSHAKE
        }
        Err(error) => wire_error(error),
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
pub extern "C" fn conduit_browser_webrtc_session_close_input() -> i32 {
    with_endpoint(|endpoint| endpoint.close_input().unwrap_or_else(wire_error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_webrtc_binding_is_planned_and_session_eligible() {
        let binding = exact_binding(0).unwrap();
        assert_eq!(
            binding.attachment.base,
            conduit_core::ConnectionBase::WebRtcDataChannel
        );
        assert_eq!(binding.attachment.base.canonical_code(), 7);
        assert!(binding.attachment.base.supports_remote_session());
        assert!(SessionMachine::new(binding, SessionRole::Source).is_ok());
    }

    #[test]
    fn out_of_stage_failure_does_not_mutate_or_create_false_active_state() {
        let binding = exact_binding(0).unwrap();
        let mut endpoint = BrowserWebRtcSession::new(SessionRole::Source, 0).unwrap();
        endpoint.output_len = 0;

        let mut bytes = [0; FRAME_CAPACITY];
        let failed = binding.frame(SessionMessage::Failed { code: 1 });
        let failed_len =
            encode_session_frame_into(failed, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
                .unwrap();
        assert_eq!(endpoint.ingest(&bytes[..failed_len]), Ok(ERROR_STAGE));
        assert_eq!(endpoint.stage, Stage::PeerHello);

        let hello = binding.hello_frame();
        let hello_len =
            encode_session_frame_into(hello, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
                .unwrap();
        assert_eq!(endpoint.ingest(&bytes[..hello_len]), Ok(STATUS_HANDSHAKE));
        endpoint.output_len = 0;
        let ready = binding.frame(SessionMessage::Ready);
        let ready_len =
            encode_session_frame_into(ready, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
                .unwrap();
        assert_eq!(endpoint.ingest(&bytes[..ready_len]), Ok(STATUS_ACTIVE));
        assert!(endpoint.machine.is_active());
    }

    #[test]
    fn reordered_offer_refuses_without_consuming_the_expected_sequence() {
        let binding = exact_binding(0).unwrap();
        let mut endpoint = BrowserWebRtcSession::new(SessionRole::Sink, 0).unwrap();
        endpoint.output_len = 0;

        let mut bytes = [0; FRAME_CAPACITY];
        let hello = binding.hello_frame();
        let hello_len =
            encode_session_frame_into(hello, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
                .unwrap();
        assert_eq!(endpoint.ingest(&bytes[..hello_len]), Ok(STATUS_HANDSHAKE));
        endpoint.output_len = 0;
        let ready = binding.frame(SessionMessage::Ready);
        let ready_len =
            encode_session_frame_into(ready, &mut bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)
                .unwrap();
        assert_eq!(endpoint.ingest(&bytes[..ready_len]), Ok(STATUS_ACTIVE));

        let reordered = binding.frame(SessionMessage::Offered {
            sequence: 1,
            payload: &[7],
        });
        let reordered_len = encode_session_frame_into(
            reordered,
            &mut bytes,
            PAYLOAD_CAPACITY,
            FRAME_CAPACITY as u32,
        )
        .unwrap();
        assert_eq!(
            endpoint.ingest(&bytes[..reordered_len]),
            Err(WireError::ReorderedFrame)
        );
        assert_eq!(endpoint.machine.next_sequence(), 0);
        assert_eq!(endpoint.received_sequence, None);

        let expected = binding.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &[7],
        });
        let expected_len = encode_session_frame_into(
            expected,
            &mut bytes,
            PAYLOAD_CAPACITY,
            FRAME_CAPACITY as u32,
        )
        .unwrap();
        assert_eq!(endpoint.ingest(&bytes[..expected_len]), Ok(STATUS_ACTIVE));
        assert_eq!(endpoint.received_sequence, Some(0));
        assert_eq!(&endpoint.received[..endpoint.received_len], &[7]);

        endpoint.output_len = 0;
        assert_eq!(
            endpoint.ingest(&bytes[..expected_len]),
            Err(WireError::ReorderedFrame)
        );
        assert_eq!(endpoint.machine.next_sequence(), 0);
        assert_eq!(endpoint.received_sequence, Some(0));
        assert_eq!(&endpoint.received[..endpoint.received_len], &[7]);
    }
}
