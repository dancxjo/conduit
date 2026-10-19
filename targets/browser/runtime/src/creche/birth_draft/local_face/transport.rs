//! Separate finite canonical-Face transport; legacy application ABI is unchanged.
use super::*;
const OUTPUT_BYTES: usize = 512 * 1024;
// Lifecycle acknowledgement adds bounded Sign/lifecycle metadata to this snapshot.
const PREPARED_HEADROOM: usize = 6 * conduit_face::MAX_FACE_INTERACTION_BYTES + 4096;
thread_local! {
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(OUTPUT_BYTES));
}
pub(super) fn admit(value: &impl Serialize) -> Result<(), String> {
    let length = serde_json::to_vec(value)
        .map_err(|error| error.to_string())?
        .len();
    if length.saturating_add(PREPARED_HEADROOM) > OUTPUT_BYTES {
        return Err("Crèche canonical transport preparation capacity".into());
    }
    Ok(())
}
pub(super) fn write(value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > OUTPUT_BYTES {
        return Err("Crèche canonical transport capacity".into());
    }
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        output.clear();
        output.extend_from_slice(&bytes);
    });
    Ok(())
}
fn request<T: serde::de::DeserializeOwned>(
    length: usize,
    action: impl FnOnce(&mut CurrentDraft, T) -> Result<(), String>,
) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    let result = abi::take_input(length)
        .map_err(|_| "Crèche local input capacity".to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
        .and_then(|request| CURRENT.with(|current| action(&mut current.borrow_mut(), request)));
    match result {
        Ok(()) => 0,
        Err(message) => {
            let _ = write(
                &serde_json::json!({"schema":"conduit.browser/creche-local-refusal@1", "message":message}),
            );
            ERROR_DRAFT
        }
    }
}
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_output_len() -> usize {
    OUTPUT.with(|output| output.borrow().len())
}
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_output_capacity() -> usize {
    OUTPUT_BYTES
}
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_prepare(length: usize) -> i32 {
    request(length, CurrentDraft::prepare_local_face)
}
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_ack(length: usize) -> i32 {
    request(length, CurrentDraft::acknowledge_local_face)
}
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_interact(length: usize) -> i32 {
    request(length, CurrentDraft::submit_local_face)
}
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_close(generation: u32) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    let result = CURRENT.with(|current| {
        let mut current = current.borrow_mut();
        current.require_generation(generation)?;
        current.close_local_face()
    });
    match result {
        Ok(()) => 0,
        Err(message) => {
            let _ = write(
                &serde_json::json!({"schema":"conduit.browser/creche-local-refusal@1", "message":message}),
            );
            ERROR_DRAFT
        }
    }
}

/// Host observation descriptor only; this does not register or acknowledge DOM.
#[no_mangle]
pub extern "C" fn conduit_creche_local_face_component_descriptor() -> i32 {
    if write(&crate::owner_face_mask::local_component::descriptor()).is_ok() {
        0
    } else {
        ERROR_DRAFT
    }
}
