use super::*;

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_start_fixture(role: u32, variant: u32) -> i32 {
    ENDPOINT.with(|slot| *slot.borrow_mut() = None);
    let role = match role {
        0 => SessionRole::Source,
        1 => SessionRole::Sink,
        _ => return ERROR_STAGE,
    };
    let binding = match exact_binding(variant) {
        Ok(binding) => binding,
        Err(error) => return wire_error(error),
    };
    install_session(role, binding)
}

#[no_mangle]
pub extern "C" fn conduit_browser_webrtc_session_start_granted(role: u32, length: u32) -> i32 {
    ENDPOINT.with(|slot| *slot.borrow_mut() = None);
    let role = match role {
        0 => SessionRole::Source,
        1 => SessionRole::Sink,
        _ => return ERROR_STAGE,
    };
    let length = length as usize;
    if length == 0 || length > FRAME_CAPACITY {
        return wire_error(WireError::OversizedFrame);
    }
    let binding = match INPUT.with(|input| {
        let input = input.borrow();
        let bytes = &input[..length];
        let frame = decode_session_frame(bytes, PAYLOAD_CAPACITY, FRAME_CAPACITY as u32)?;
        let binding = SessionBinding::from_hello_frame(frame)?;
        if binding.attachment.base
            != conduit_core::BaseImplementationId::from("conduit.base/webrtc-data-channel@1")
        {
            return Err(WireError::InvalidBase);
        }
        let mut canonical = vec![0; FRAME_CAPACITY];
        let canonical_length = encode_session_frame_into(
            binding.hello_frame(),
            &mut canonical,
            binding.limits.maximum_payload_bytes,
            FRAME_CAPACITY as u32,
        )?;
        if canonical[..canonical_length] != *bytes {
            return Err(WireError::InvalidSession);
        }
        Ok(binding)
    }) {
        Ok(binding) => binding,
        Err(error) => return wire_error(error),
    };
    install_session(role, binding)
}

fn install_session(role: SessionRole, binding: SessionBinding) -> i32 {
    match BrowserWebRtcSession::new(role, binding) {
        Ok(endpoint) => {
            ENDPOINT.with(|slot| *slot.borrow_mut() = Some(endpoint));
            STATUS_HANDSHAKE
        }
        Err(error) => wire_error(error),
    }
}
