//! Internal observation ABI for the current browser-owned Body Play.
use super::*;

#[no_mangle]
pub extern "C" fn conduit_browser_patchbay_observation() -> i32 {
    clear_output();
    let result = SESSION.with(|slot| {
        let slot = slot.borrow();
        slot.as_ref()
            .ok_or("no current browser-owned Body Play")?
            .patchbay_observation()
    });
    match result {
        Ok(observation) => write_output(&observation)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(message) => {
            let _ = write_output(&super::super::refusal(message));
            ERROR_INTERACTION
        }
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_patchbay_request_disposition(length: usize) -> i32 {
    clear_output();
    if length == 0 || length > INPUT_BYTES.min(4096) {
        return ERROR_INPUT;
    }
    let input =
        INPUT.with(|input| {
            let mut input = input.borrow_mut();
            let value = serde_json::from_slice::<
                super::super::patchbay_disposition::RequestDisposition,
            >(&input[..length]);
            input[..length].fill(0);
            value.map_err(|error| format!("Patchbay disposition input: {error}"))
        });
    let result = input.and_then(|disposition| {
        SESSION.with(|slot| {
            slot.borrow_mut()
                .as_mut()
                .ok_or("no current browser-owned Body Play")?
                .patchbay_request_disposition(disposition)
        })
    });
    match result {
        Ok(observation) => write_output(&observation)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(message) => {
            let _ = write_output(&super::super::refusal(message));
            ERROR_INTERACTION
        }
    }
}

// The canonical JSON envelope is separately bounded and is never decoded as a
// legacy application event. The emitted Fore frame retains its own 8 KiB bound.
fn canonical_command<T: serde::de::DeserializeOwned>(
    length: usize,
    apply: impl FnOnce(&mut TourSession, T) -> Result<serde_json::Value, String>,
) -> i32 {
    clear_output();
    if length == 0 || length > 8192 || length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    let decoded = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let value = serde_json::from_slice::<T>(&input[..length]);
        input[..length].fill(0);
        value.map_err(|error| format!("canonical Patchbay input: {error}"))
    });
    let result = decoded.and_then(|value| {
        SESSION.with(|slot| {
            let mut slot = slot.borrow_mut();
            apply(
                slot.as_mut().ok_or("no current browser-owned Body Play")?,
                value,
            )
        })
    });
    match result {
        Ok(value) => write_output(&value)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(message) => {
            let _ = write_output(&super::super::refusal(message));
            ERROR_INTERACTION
        }
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_patchbay_acknowledge(length: usize) -> i32 {
    canonical_command(length, |session, input| session.patchbay_acknowledge(input))
}

#[no_mangle]
pub extern "C" fn conduit_browser_patchbay_interact(length: usize) -> i32 {
    canonical_command(length, |session, input| session.patchbay_interact(input))
}
