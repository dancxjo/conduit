//! One bounded ABI slot. A canonical hello proves identity, not membership or
//! transport readiness. The caller separately owns admitted current transport.
use super::{
    session::{Command, Session},
    *,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};

const INPUT_BYTES: usize = 256 * 1024;
const OUTPUT_BYTES: usize = 128 * 1024;
const HELLO_BYTES: usize = 16 * 1024;
const READY: i32 = 1;
const ERROR: i32 = -1;
thread_local! {
    static INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; INPUT_BYTES].into_boxed_slice());
    static OUTPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; OUTPUT_BYTES].into_boxed_slice());
    static LENGTH: Cell<usize> = const { Cell::new(0) };
    static SLOT: RefCell<Option<Session>> = const { RefCell::new(None) };
}

pub(in crate::form_runner) fn is_active() -> bool {
    SLOT.with(|slot| slot.borrow().is_some())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    plan: Plan,
    host: HostAdvertisement,
    hello: Vec<u8>,
    active_play_id: ActivePlayId,
    observations: Vec<ResourceObservation>,
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_remote_input_capacity() -> usize {
    INPUT_BYTES
}
#[no_mangle]
pub extern "C" fn conduit_browser_remote_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_remote_output_capacity() -> usize {
    OUTPUT_BYTES
}
#[no_mangle]
pub extern "C" fn conduit_browser_remote_output_len() -> usize {
    LENGTH.with(Cell::get)
}

fn read<T: serde::de::DeserializeOwned>(length: usize) -> Result<T, String> {
    if length == 0 || length > INPUT_BYTES {
        return Err("remote input length refused".into());
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let request = serde_json::from_slice(&input[..length])
            .map_err(|_| "remote input shape refused".into());
        input[..length].fill(0);
        request
    })
}

fn write(value: &Value) -> Result<(), String> {
    LENGTH.with(|length| length.set(0));
    let length = OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        let mut cursor = std::io::Cursor::new(&mut output[..]);
        serde_json::to_writer(&mut cursor, value)
            .map_err(|_| "remote output capacity exceeded".to_string())?;
        Ok::<_, String>(cursor.position() as usize)
    })?;
    LENGTH.with(|slot| slot.set(length));
    Ok(())
}

fn report(result: Result<Value, String>) -> i32 {
    match result {
        Ok(value) => {
            if write(&value).is_ok() {
                READY
            } else {
                ERROR
            }
        }
        Err(message) => {
            let _ = write(&json!({"schema":"conduit.browser/remote-refusal@1", "message":message}));
            ERROR
        }
    }
}

fn binding(hello: &[u8]) -> Result<SessionBinding, String> {
    if hello.is_empty() || hello.len() > HELLO_BYTES {
        return Err("remote hello exceeds bound".into());
    }
    let frame = conduit_wire::decode_session_frame(
        hello,
        crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        HELLO_BYTES as u32,
    )
    .map_err(debug)?;
    let binding = SessionBinding::from_hello_frame(frame).map_err(debug)?;
    let mut canonical = vec![0; HELLO_BYTES];
    let length = conduit_wire::encode_session_frame_into(
        binding.hello_frame(),
        &mut canonical,
        binding.limits.maximum_payload_bytes,
        HELLO_BYTES as u32,
    )
    .map_err(debug)?;
    if &canonical[..length] != hello {
        return Err("noncanonical remote hello".into());
    }
    Ok(binding)
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_start(length: usize) -> i32 {
    LENGTH.with(|length| length.set(0));
    publish((|| {
        if is_active()
            || crate::form_runner::abi::has_local_session()
            || crate::form_runner::multihost::has_session()
        {
            return Err("another browser execution slot is active".into());
        }
        let request: Start = read(length)?;
        let binding = binding(&request.hello)?;
        let execution = RemoteExecution::prepare(
            &request.plan,
            &request.host,
            &binding,
            &request.active_play_id,
            &request.observations,
        )?;
        // Install before driving: a failure must not lose dispatched Host work.
        SLOT.with(|slot| {
            *slot.borrow_mut() = Some(Session::new(execution, request.active_play_id))
        });
        SLOT.with(|slot| slot.borrow_mut().as_mut().unwrap().progress())
    })())
}

#[no_mangle]
pub extern "C" fn conduit_browser_remote_command(length: usize) -> i32 {
    LENGTH.with(|length| length.set(0));
    let result = (|| {
        let command: Command = read(length)?;
        SLOT.with(|slot| {
            slot.borrow_mut()
                .as_mut()
                .ok_or("no active remote execution".to_string())?
                .command(command)
        })
    })();
    publish(result)
}

fn publish(result: Result<Value, String>) -> i32 {
    let status = report(result);
    // Terminal evidence must fit before the slot is released. Failed commands
    // never take/drop a live session, including cancellation awaiting ack.
    if status == READY {
        SLOT.with(|slot| {
            if slot
                .borrow()
                .as_ref()
                .is_some_and(|session| session.retired)
            {
                *slot.borrow_mut() = None;
            }
        });
    }
    status
}

#[cfg(test)]
#[path = "abi_tests.rs"]
mod tests;
