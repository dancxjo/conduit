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
