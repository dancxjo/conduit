//! Bounded Body start input; execution and completions use the existing slot.
use super::*;
const BODY_INPUT_BYTES: usize = 256 * 1024;
thread_local! {
    static BODY_INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; BODY_INPUT_BYTES].into_boxed_slice());
}

#[no_mangle]
pub extern "C" fn conduit_browser_body_input_ptr() -> usize {
    BODY_INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_body_input_capacity() -> usize {
    BODY_INPUT_BYTES
}

/// The trusted browser Host supplies exact resource observations. This does
/// not discover or acquire devices, grant permission, or implicitly replace Play.
#[no_mangle]
pub extern "C" fn conduit_browser_body_start(length: usize) -> i32 {
    clear_output();
    if length == 0 || length > BODY_INPUT_BYTES {
        return ERROR_INPUT;
    }
    if SESSION.with(|slot| slot.borrow().is_some()) {
        let _ = write_output(&super::super::refusal(
            "browser session is already active; retire it explicitly before Body start".into(),
        ));
        return ERROR_PREPARE;
    }
    let result = BODY_INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let request =
            serde_json::from_slice::<super::super::body_start::BodyStartRequest>(&input[..length]);
        input[..length].fill(0);
        request
    });
    let request = match result {
        Ok(request) => request,
        Err(_) => return ERROR_INPUT,
    };
    match super::super::body_start::prepare(request) {
        Ok((session, started)) => {
            if write_output(&started).is_err() {
                return ERROR_OUTPUT;
            }
            if !matches!(&started.progress, super::super::TourProgress::Receipt(_)) {
                SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            }
            STATUS_READY
        }
        Err(message) => {
            let _ = write_output(&super::super::refusal(message));
            ERROR_PREPARE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_start_uses_the_existing_slot_and_completion_abi() {
        let request = super::super::super::body_start::tests::request();
        let play = conduit_body::BodyPlayIdentity::bind(&request.plan, request.play_sequence);
        let bytes = serde_json::to_vec(&request).unwrap();
        assert!(bytes.len() <= conduit_browser_body_input_capacity());
        BODY_INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
        assert_eq!(conduit_browser_body_start(bytes.len()), STATUS_READY);
        SESSION.with(|slot| {
            assert_eq!(
                slot.borrow().as_ref().unwrap().active_play_id,
                play.active_play_id
            )
        });
        assert_eq!(conduit_browser_body_start(bytes.len()), ERROR_PREPARE);
        SESSION.with(|slot| {
            assert_eq!(
                slot.borrow().as_ref().unwrap().active_play_id,
                play.active_play_id
            )
        });
        assert_eq!(conduit_tour_complete(), STATUS_READY);
        assert_eq!(conduit_tour_complete(), STATUS_READY);
        SESSION.with(|slot| assert!(slot.borrow().is_none()));
        let receipt: serde_json::Value = OUTPUT.with(|output| {
            serde_json::from_slice(&output.borrow()[..conduit_tour_output_len()]).unwrap()
        });
        assert_eq!(receipt["active_play_id"], play.active_play_id.as_str());
        assert_eq!(receipt["disposition"], "completed");
        assert_eq!(
            conduit_browser_body_start(BODY_INPUT_BYTES + 1),
            ERROR_INPUT
        );
    }
}
