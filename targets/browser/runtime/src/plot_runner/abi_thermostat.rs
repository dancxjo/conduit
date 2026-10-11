//! Bounded local Face/Show interaction boundary, sharing the current Body Play slot.
use super::*;
use serde::de::DeserializeOwned;

fn read<T: DeserializeOwned>(length: usize) -> Result<T, String> {
    if length == 0 || length > INPUT_BYTES {
        return Err("Thermostat request exceeds the bounded input capacity".into());
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = serde_json::from_slice(&input[..length])
            .map_err(|error| format!("Thermostat request refused: {error}"));
        input[..length].fill(0);
        result
    })
}
fn run(action: impl FnOnce(&mut TourSession) -> Result<serde_json::Value, String>) -> i32 {
    clear_output();
    let result = SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        action(slot.as_mut().ok_or("no current browser-owned Body Play")?)
    });
    match result {
        Ok(snapshot) => write_output(&snapshot)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(message) => {
            let _ = write_output(&super::super::refusal(message));
            ERROR_INTERACTION
        }
    }
}
#[no_mangle]
pub extern "C" fn conduit_browser_thermostat_face() -> i32 {
    run(TourSession::thermostat_snapshot)
}
#[no_mangle]
pub extern "C" fn conduit_browser_thermostat_acknowledge(length: usize) -> i32 {
    run(|session| session.acknowledge_thermostat_show(read(length)?))
}
#[no_mangle]
pub extern "C" fn conduit_browser_thermostat_interact(length: usize) -> i32 {
    run(|session| session.interact_thermostat(read(length)?))
}
