use super::{BrowserChatEffect, BrowserChatSession};
use std::cell::RefCell;

const INPUT_CAPACITY: usize = 4_096;
const ERROR_NOT_STARTED: i32 = -240;
const ERROR_INPUT: i32 = -241;

thread_local! {
    static SESSION: RefCell<Option<BrowserChatSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_CAPACITY]> = const { RefCell::new([0; INPUT_CAPACITY]) };
}

fn with_session<T>(
    action: impl FnOnce(&mut BrowserChatSession) -> Result<T, i32>,
) -> Result<T, i32> {
    SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        action(slot.as_mut().ok_or(ERROR_NOT_STARTED)?)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_input_capacity() -> u32 {
    INPUT_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_start(url_len: u32) -> i32 {
    let frame_len = url_len as usize;
    if frame_len == 0 || frame_len > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        let Ok(frame) = std::str::from_utf8(&input[..frame_len]) else {
            return ERROR_INPUT;
        };
        let mut fields = frame.split('\n');
        let (Some(url), Some(host_id), Some(boot_id), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return ERROR_INPUT;
        };
        match BrowserChatSession::prepare(
            url,
            conduit_core::HostId::from(host_id),
            conduit_core::BootId::from(boot_id),
        ) {
            Ok(session) => {
                SESSION.with(|slot| *slot.borrow_mut() = Some(session));
                0
            }
            Err(error) => error,
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_status() -> i32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(BrowserChatSession::status)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_effect_kind() -> i32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.effect() as i32)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_effect_ptr() -> *const u8 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.effect_bytes().as_ptr())
            .unwrap_or(core::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_effect_len() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.effect_bytes().len() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_complete_effect() -> i32 {
    with_session(|session| {
        let effect = session.effect();
        if !matches!(
            effect,
            BrowserChatEffect::SocketOpen
                | BrowserChatEffect::SocketSend
                | BrowserChatEffect::SocketClose
                | BrowserChatEffect::ListAppend
        ) {
            return Err(ERROR_INPUT);
        }
        session.complete_simple(effect)
    })
    .map(|_| conduit_browser_webchat_status())
    .unwrap_or_else(|error| error)
}

fn input_action(
    length: u32,
    action: impl FnOnce(&mut BrowserChatSession, &[u8]) -> Result<(), i32>,
) -> i32 {
    let length = length as usize;
    if length == 0 || length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        with_session(|session| action(session, &input[..length]))
            .map(|_| conduit_browser_webchat_status())
            .unwrap_or_else(|error| error)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_receive(length: u32) -> i32 {
    input_action(length, BrowserChatSession::receive)
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_submit(length: u32) -> i32 {
    input_action(length, BrowserChatSession::submit)
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_disconnect() -> i32 {
    with_session(BrowserChatSession::disconnect)
        .map(|_| conduit_browser_webchat_status())
        .unwrap_or_else(|error| error)
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_identity_ptr() -> *const u8 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.identity_text().as_ptr())
            .unwrap_or(core::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_identity_len() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.identity_text().len() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_disconnected() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| u32::from(session.disconnected()))
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_capacity_stable() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| u32::from(session.capacity_stable()))
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_webchat_request_count() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.request_count() as u32)
            .unwrap_or(0)
    })
}
