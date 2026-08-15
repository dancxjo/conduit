//! Browser-owned endpoint for one exact WebRTC DataChannel session.
//!
//! JavaScript owns transport readiness and pressure. This module owns only the
//! shared, plan-bound `conduit-wire` admission state; every page instantiates a
//! separate WASM module and therefore a separate endpoint.

use conduit_core::{
    AdmittedLine, BootId, BoundLink, ConnectionBase, ConnectionBaseInstanceId, ConnectionId,
    FragmentId, HostId, KindId, LineContinuation, LineContract, LineDuplex, LineId, LineOrdering,
    LineReliability, LineScope, LineSecurity, LineTrafficShape, LinkAuthorityReference,
    LinkBindingId, LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, PlacementId,
    PlannedConnection, PortId, PortTemporal,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition, WireError,
};
use std::cell::RefCell;

const FRAME_CAPACITY: usize = 1_024;
const PAYLOAD_CAPACITY: u32 = 16;
const STATUS_HANDSHAKE: i32 = 0;
const STATUS_ACTIVE: i32 = 1;
const STATUS_TERMINAL: i32 = 2;
const ERROR_NOT_STARTED: i32 = -200;
const ERROR_OUTPUT_PENDING: i32 = -201;
const ERROR_STAGE: i32 = -202;

thread_local! {
    static ENDPOINT: RefCell<Option<BrowserWebRtcSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; FRAME_CAPACITY]> = const { RefCell::new([0; FRAME_CAPACITY]) };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    PeerHello,
    PeerReady,
    Active,
    Terminal,
}

struct BrowserWebRtcSession {
    binding: SessionBinding,
    machine: SessionMachine,
    output: [u8; FRAME_CAPACITY],
    output_len: usize,
    stage: Stage,
}

impl BrowserWebRtcSession {
    fn new(role: SessionRole, variant: u32) -> Result<Self, WireError> {
        let binding = exact_binding(variant)?;
        let machine = SessionMachine::new(binding.clone(), role)?;
        let mut session = Self {
            binding,
            machine,
            output: [0; FRAME_CAPACITY],
            output_len: 0,
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
            (Stage::Terminal, SessionMessage::Terminal { .. }) => Ok(STATUS_TERMINAL),
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

    fn finish(&mut self) -> Result<i32, WireError> {
        if self.output_len != 0 || self.stage != Stage::Active {
            return Ok(ERROR_STAGE);
        }
        let binding = self.binding.clone();
        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: self.machine.next_sequence(),
        });
        self.machine.admit_outbound(terminal)?;
        self.write(terminal)?;
        self.stage = Stage::Terminal;
        Ok(STATUS_TERMINAL)
    }
}

fn exact_binding(variant: u32) -> Result<SessionBinding, WireError> {
    let mut plan_id = conduit_core::PlanId::from("browser-webrtc/plan/1");
    let source = LinkEndpoint {
        host_id: HostId::from("browser-webrtc/source"),
        boot_id: BootId::from("browser-webrtc/source-boot/1"),
        endpoint_id: LinkEndpointId::from("browser-webrtc/source-egress"),
    };
    let sink = LinkEndpoint {
        host_id: HostId::from("browser-webrtc/sink"),
        boot_id: BootId::from("browser-webrtc/sink-boot/1"),
        endpoint_id: LinkEndpointId::from("browser-webrtc/sink-ingress"),
    };
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: PAYLOAD_CAPACITY,
        maximum_buffered_bytes: PAYLOAD_CAPACITY,
        maximum_frame_bytes: FRAME_CAPACITY as u32,
    };
    let line = AdmittedLine {
        line_id: LineId::from("browser-webrtc/line/1"),
        binding: BoundLink {
            binding_id: LinkBindingId::from("browser-webrtc/binding/1"),
            source: source.clone(),
            sink: sink.clone(),
            base: ConnectionBase::WebRtcDataChannel,
            base_instance_id: ConnectionBaseInstanceId::from("browser-webrtc/base-instance/1"),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits,
        },
        contract: LineContract {
            scope: LineScope::PointToPoint,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::AuthenticatedEncrypted,
        },
    };
    let mut connection = PlannedConnection {
        connection_id: ConnectionId::from("browser-webrtc/connection/1"),
        source_placement_id: PlacementId::from("browser-webrtc/source-placement"),
        source_port_id: PortId::from("out"),
        sink_placement_id: PlacementId::from("browser-webrtc/sink-placement"),
        sink_port_id: PortId::from("in"),
        value_kind: KindId::from("conduit.test/bounded-bytes@1"),
        temporal: PortTemporal::Value,
        selected_line: Some(line.clone()),
        admitted_lines: vec![line],
        item_capacity: 1,
        byte_capacity: PAYLOAD_CAPACITY,
    };
    match variant {
        0 => {}
        1 => connection.connection_id = ConnectionId::from("browser-webrtc/wrong-connection"),
        2 => connection.value_kind = KindId::from("conduit.test/wrong-value@1"),
        3 => {
            let line = connection.selected_line.as_mut().expect("selected Line");
            line.binding.source.boot_id = BootId::from("browser-webrtc/stale-source-boot");
            connection.admitted_lines[0] = line.clone();
        }
        4 => {
            let line = connection.selected_line.as_mut().expect("selected Line");
            line.binding.base_instance_id =
                ConnectionBaseInstanceId::from("browser-webrtc/wrong-base-instance");
            connection.admitted_lines[0] = line.clone();
        }
        5 => plan_id = conduit_core::PlanId::from("browser-webrtc/wrong-plan"),
        _ => return Err(WireError::InvalidSession),
    }
    SessionBinding::from_planned_connection(
        plan_id,
        FragmentId::from("browser-webrtc/source-fragment"),
        FragmentId::from("browser-webrtc/sink-fragment"),
        &connection,
    )
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
pub extern "C" fn conduit_browser_webrtc_session_close_input() -> i32 {
    with_endpoint(|endpoint| endpoint.close_input().unwrap_or_else(wire_error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_webrtc_binding_is_planned_and_session_eligible() {
        let binding = exact_binding(0).unwrap();
        assert_eq!(binding.attachment.base, ConnectionBase::WebRtcDataChannel);
        assert_eq!(binding.attachment.base.canonical_code(), 7);
        assert!(binding.attachment.base.supports_remote_session());
        assert!(SessionMachine::new(binding, SessionRole::Source).is_ok());
    }
}
