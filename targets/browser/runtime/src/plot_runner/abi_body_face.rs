//! Whole-Body Mask commands reuse the existing bounded Body/control buffers.
use super::*;

fn respond(result: Result<serde_json::Value, String>) -> i32 {
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
pub extern "C" fn conduit_browser_body_face_prepare(length: usize) -> i32 {
    clear_output();
    if length == 0 || length > super::body::BODY_INPUT_BYTES {
        return ERROR_INPUT;
    }
    let request =
        super::body::BODY_INPUT.with(|input| {
            let mut input = input.borrow_mut();
            let request = serde_json::from_slice::<
                super::super::body_face_publication::BodyFaceRequest,
            >(&input[..length]);
            input[..length].fill(0);
            request.map_err(|error| format!("whole Body publication input: {error}"))
        });
    respond(request.and_then(|request| {
        SESSION.with(|slot| {
            slot.borrow_mut()
                .as_mut()
                .ok_or("no current browser-owned Body Play")?
                .body_face_prepare(request)
        })
    }))
}

fn command<T: serde::de::DeserializeOwned>(
    length: usize,
    apply: impl FnOnce(&mut TourSession, T) -> Result<serde_json::Value, String>,
) -> i32 {
    clear_output();
    if length == 0 || length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    let request = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let request = serde_json::from_slice::<T>(&input[..length]);
        input[..length].fill(0);
        request.map_err(|error| format!("whole Body Mask command: {error}"))
    });
    respond(request.and_then(|request| {
        SESSION.with(|slot| {
            apply(
                slot.borrow_mut()
                    .as_mut()
                    .ok_or("no current browser-owned Body Play")?,
                request,
            )
        })
    }))
}

#[no_mangle]
pub extern "C" fn conduit_browser_body_face_acknowledge(length: usize) -> i32 {
    command(length, |session, request| {
        session.body_face_acknowledge(request)
    })
}
#[no_mangle]
pub extern "C" fn conduit_browser_body_face_interact(length: usize) -> i32 {
    command(length, |session, request| {
        session.body_face_interact(request)
    })
}
#[no_mangle]
pub extern "C" fn conduit_browser_body_face_observation() -> i32 {
    clear_output();
    respond(SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .ok_or("no current browser-owned Body Play")?
            .body_face_observation()
    }))
}
