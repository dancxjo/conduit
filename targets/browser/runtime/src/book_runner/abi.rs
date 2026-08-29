//! Bounded WASM boundary for the single executable-book Play.

use super::BookSession;
use std::cell::RefCell;

const INPUT_BYTES: usize = 8 * 1_024;
const OUTPUT_BYTES: usize = 16 * 1_024;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -401;
const ERROR_PREPARE: i32 = -402;
const ERROR_NOT_RUNNING: i32 = -403;
const ERROR_OUTPUT: i32 = -404;
const ERROR_COMPLETE: i32 = -405;
const ERROR_CANCEL: i32 = -406;

thread_local! {
    static SESSION: RefCell<Option<BookSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
    static OUTPUT: RefCell<[u8; OUTPUT_BYTES]> = const { RefCell::new([0; OUTPUT_BYTES]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[no_mangle]
pub extern "C" fn conduit_book_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_book_input_capacity() -> usize {
    INPUT_BYTES
}

#[no_mangle]
pub extern "C" fn conduit_book_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_book_output_len() -> usize {
    OUTPUT_LEN.with(|length| *length.borrow())
}

/// Starts one exact Play from adjacent UTF-8 Host, Boot, and Form source bytes.
/// Replacing an unfinished session explicitly cancels its kernel scheduler.
#[no_mangle]
pub extern "C" fn conduit_book_start(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    play_sequence: u64,
) -> i32 {
    clear_output();
    let Some(identity_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    let Some(total_length) = identity_length.checked_add(source_length) else {
        return ERROR_INPUT;
    };
    if host_length == 0 || boot_length == 0 || source_length == 0 || total_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    SESSION.with(|slot| {
        if let Some(previous) = slot.borrow_mut().take() {
            let _ = previous.cancel();
        }
    });
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length]).map_err(|_| ERROR_INPUT)?;
            let boot = core::str::from_utf8(&input[host_length..identity_length])
                .map_err(|_| ERROR_INPUT)?;
            let source = core::str::from_utf8(&input[identity_length..total_length])
                .map_err(|_| ERROR_INPUT)?;
            let (session, effect) = BookSession::prepare(host, boot, source, play_sequence)
                .map_err(|_| ERROR_PREPARE)?;
            write_output(&effect).map_err(|_| ERROR_OUTPUT)?;
            SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            Ok(STATUS_READY)
        })();
        input[..total_length].fill(0);
        result.unwrap_or_else(|error| error)
    })
}

#[no_mangle]
pub extern "C" fn conduit_book_complete() -> i32 {
    finish(false)
}

#[no_mangle]
pub extern "C" fn conduit_book_cancel() -> i32 {
    finish(true)
}

fn finish(cancel: bool) -> i32 {
    clear_output();
    SESSION.with(|slot| {
        let Some(session) = slot.borrow_mut().take() else {
            return ERROR_NOT_RUNNING;
        };
        let receipt = if cancel {
            session.cancel().map_err(|_| ERROR_CANCEL)
        } else {
            session.complete().map_err(|_| ERROR_COMPLETE)
        };
        match receipt.and_then(|receipt| write_output(&receipt).map_err(|_| ERROR_OUTPUT)) {
            Ok(()) => STATUS_READY,
            Err(error) => error,
        }
    })
}

fn write_output(value: &impl serde::Serialize) -> Result<(), ()> {
    let encoded = serde_json::to_vec(value).map_err(|_| ())?;
    if encoded.len() > OUTPUT_BYTES {
        return Err(());
    }
    OUTPUT.with(|output| {
        output.borrow_mut()[..encoded.len()].copy_from_slice(&encoded);
    });
    OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    Ok(())
}

fn clear_output() {
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_book_start_is_refused_without_a_session() {
        assert_eq!(conduit_book_start(0, 0, 0, 0), ERROR_INPUT);
        assert_eq!(conduit_book_complete(), ERROR_NOT_RUNNING);
    }
}
