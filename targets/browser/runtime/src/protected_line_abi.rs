//! Fixed-buffer WASM ABI for the portable protected-Line profile.

use conduit_protected_line::{
    EndpointBinding, ProtectedHandshake, ProtectedLineError, ProtectedSession, Role,
    SessionBinding, SessionLimits,
};
use serde::Deserialize;
use std::cell::RefCell;

const INPUT_CAPACITY: usize = 65_600;
const OUTPUT_CAPACITY: usize = 65_600;
const KEY_BYTES: usize = 32;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -300;
const ERROR_STATE: i32 = -301;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AbiEndpointBinding {
    host_id: String,
    boot_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AbiSessionBinding {
    initiator: AbiEndpointBinding,
    responder: AbiEndpointBinding,
    negotiation_id: String,
    line_session_id: String,
    candidate_binding: String,
    transport_binding: String,
}

impl From<AbiSessionBinding> for SessionBinding {
    fn from(binding: AbiSessionBinding) -> Self {
        Self {
            initiator: EndpointBinding {
                host_id: binding.initiator.host_id,
                boot_id: binding.initiator.boot_id,
            },
            responder: EndpointBinding {
                host_id: binding.responder.host_id,
                boot_id: binding.responder.boot_id,
            },
            negotiation_id: binding.negotiation_id,
            line_session_id: binding.line_session_id,
            candidate_binding: binding.candidate_binding,
            transport_binding: binding.transport_binding,
        }
    }
}

thread_local! {
    static HANDSHAKE: RefCell<Option<ProtectedHandshake>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<ProtectedSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_CAPACITY]> = const { RefCell::new([0; INPUT_CAPACITY]) };
    static OUTPUT: RefCell<[u8; OUTPUT_CAPACITY]> = const { RefCell::new([0; OUTPUT_CAPACITY]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_input_capacity() -> u32 {
    INPUT_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_output_ptr() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_output_len() -> u32 {
    OUTPUT_LEN.with(|length| *length.borrow() as u32)
}

/// Input is canonical binding JSON followed by the 32-byte rendezvous secret
/// and a fresh 32-byte browser-owned ephemeral private key. Both keys are
/// erased from the ABI input before this function returns.
#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_initialize(
    role: u32,
    binding_bytes: u32,
    maximum_payload_bytes: u32,
    maximum_frames_low: u32,
    maximum_frames_high: u32,
    maximum_bytes_low: u32,
    maximum_bytes_high: u32,
) -> i32 {
    reset_state();
    let binding_bytes = binding_bytes as usize;
    let Some(key_end) = binding_bytes.checked_add(KEY_BYTES * 2) else {
        return ERROR_INPUT;
    };
    if binding_bytes == 0 || key_end > INPUT_CAPACITY {
        INPUT.with(|input| input.borrow_mut().fill(0));
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let role = match role {
                0 => Role::Initiator,
                1 => Role::Responder,
                _ => return Err(ERROR_INPUT),
            };
            let binding: AbiSessionBinding =
                serde_json::from_slice(&input[..binding_bytes]).map_err(|_| ERROR_INPUT)?;
            let mut psk = [0; KEY_BYTES];
            psk.copy_from_slice(&input[binding_bytes..binding_bytes + KEY_BYTES]);
            let mut ephemeral = [0; KEY_BYTES];
            ephemeral.copy_from_slice(&input[binding_bytes + KEY_BYTES..key_end]);
            let limits = SessionLimits {
                maximum_payload_bytes,
                maximum_frames_per_direction: join_u64(maximum_frames_low, maximum_frames_high),
                maximum_bytes_per_direction: join_u64(maximum_bytes_low, maximum_bytes_high),
            };
            ProtectedHandshake::new(role, &binding.into(), limits, psk, ephemeral)
                .map_err(map_protected)
        })();
        input[binding_bytes..key_end].fill(0);
        match result {
            Ok(handshake) => {
                HANDSHAKE.with(|state| *state.borrow_mut() = Some(handshake));
                STATUS_READY
            }
            Err(error) => error,
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_write_handshake() -> i32 {
    clear_output();
    let result = with_handshake(|handshake| {
        let length = handshake.next_message_bytes().map_err(map_protected)?;
        if length > OUTPUT_CAPACITY {
            return Err(ERROR_INPUT);
        }
        OUTPUT.with(|output| {
            handshake
                .write_message(&mut output.borrow_mut()[..length])
                .map_err(map_protected)
        })?;
        OUTPUT_LEN.with(|output| *output.borrow_mut() = length);
        Ok(STATUS_READY)
    });
    match result.and_then(|status| promote_completed_handshake().map(|()| status)) {
        Ok(status) => status,
        Err(error) => error,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_read_handshake(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length == 0 || length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let result = INPUT.with(|input| {
        with_handshake(|handshake| {
            handshake
                .read_message(&input.borrow()[..length])
                .map_err(map_protected)?;
            Ok(STATUS_READY)
        })
    });
    match result.and_then(|status| promote_completed_handshake().map(|()| status)) {
        Ok(status) => status,
        Err(error) => error,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_seal(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let result = INPUT.with(|input| {
        OUTPUT.with(|output| {
            with_session(|session| {
                let sealed = session
                    .seal(&input.borrow()[..length], &mut output.borrow_mut()[..])
                    .map_err(map_protected)?;
                OUTPUT_LEN.with(|output| *output.borrow_mut() = sealed);
                Ok(STATUS_READY)
            })
        })
    });
    result.unwrap_or_else(|error| error)
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_open(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length == 0 || length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let result = INPUT.with(|input| {
        OUTPUT.with(|output| {
            with_session(|session| {
                let opened = session
                    .open(&input.borrow()[..length], &mut output.borrow_mut()[..])
                    .map_err(map_protected)?;
                OUTPUT_LEN.with(|output| *output.borrow_mut() = opened);
                Ok(STATUS_READY)
            })
        })
    });
    result.unwrap_or_else(|error| error)
}

#[no_mangle]
pub extern "C" fn conduit_browser_protected_line_close() -> i32 {
    let result = with_session(|session| {
        session.close();
        Ok(STATUS_READY)
    });
    clear_output();
    result.unwrap_or_else(|error| error)
}

fn with_handshake<T>(
    action: impl FnOnce(&mut ProtectedHandshake) -> Result<T, i32>,
) -> Result<T, i32> {
    HANDSHAKE.with(|state| {
        let mut state = state.borrow_mut();
        action(state.as_mut().ok_or(ERROR_STATE)?)
    })
}

fn with_session<T>(action: impl FnOnce(&mut ProtectedSession) -> Result<T, i32>) -> Result<T, i32> {
    SESSION.with(|state| {
        let mut state = state.borrow_mut();
        action(state.as_mut().ok_or(ERROR_STATE)?)
    })
}

fn promote_completed_handshake() -> Result<(), i32> {
    HANDSHAKE.with(|state| {
        let mut handshake = state.borrow_mut();
        let Some(current) = handshake.as_ref() else {
            return Err(ERROR_STATE);
        };
        if current.next_message_bytes().is_ok() {
            return Ok(());
        }
        let session = handshake
            .take()
            .ok_or(ERROR_STATE)?
            .finish()
            .map_err(map_protected)?;
        SESSION.with(|state| *state.borrow_mut() = Some(session));
        Ok(())
    })
}

fn join_u64(low: u32, high: u32) -> u64 {
    u64::from(low) | (u64::from(high) << 32)
}

fn map_protected(error: ProtectedLineError) -> i32 {
    match error {
        ProtectedLineError::EmptyEphemeralKey => -310,
        ProtectedLineError::EmptyPresharedKey => -311,
        ProtectedLineError::InvalidBinding => -312,
        ProtectedLineError::BindingTooLarge => -313,
        ProtectedLineError::BindingMismatch(_) => -314,
        ProtectedLineError::InvalidLimits => -315,
        ProtectedLineError::WrongHandshakeTurn => -316,
        ProtectedLineError::HandshakeFrameLength => -317,
        ProtectedLineError::AuthenticationFailed => -318,
        ProtectedLineError::HandshakeIncomplete => -319,
        ProtectedLineError::FrameTooLarge => -320,
        ProtectedLineError::OutputTooSmall => -321,
        ProtectedLineError::MalformedFrame => -322,
        ProtectedLineError::TruncatedFrame => -323,
        ProtectedLineError::WrongDirection => -324,
        ProtectedLineError::Replay => -325,
        ProtectedLineError::Reordered => -326,
        ProtectedLineError::FrameLimitExhausted => -327,
        ProtectedLineError::ByteLimitExhausted => -328,
        ProtectedLineError::InvalidPolicy => -329,
        ProtectedLineError::SessionCapacity => -330,
        ProtectedLineError::Pressure => -331,
        ProtectedLineError::HandshakeTimedOut => -332,
        ProtectedLineError::SessionTimedOut => -333,
        ProtectedLineError::OuterCarrierLost => -334,
        ProtectedLineError::Cancelled => -335,
        ProtectedLineError::Closed => -336,
    }
}

fn reset_state() {
    HANDSHAKE.with(|state| state.borrow_mut().take());
    SESSION.with(|state| state.borrow_mut().take());
    clear_output();
}

fn clear_output() {
    OUTPUT.with(|output| output.borrow_mut().fill(0));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
}
