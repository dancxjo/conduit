use super::session;
use crate::book_runner::interaction::SourceInteractionEvidence;
use std::cell::RefCell;

const INPUT_BYTES: usize = 8 * 1_024;
const OUTPUT_BYTES: usize = 32 * 1_024;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -451;
const ERROR_BIRTH: i32 = -452;
const ERROR_OUTPUT: i32 = -453;
const ERROR_INTERACTION: i32 = -454;
const STATUS_ABSENT: i32 = 1;

thread_local! {
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
    static OUTPUT: RefCell<[u8; OUTPUT_BYTES]> = const { RefCell::new([0; OUTPUT_BYTES]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
    static SOURCE_INTERACTION: RefCell<Option<SourceInteractionEvidence>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn conduit_book_body_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_book_body_input_capacity() -> usize {
    INPUT_BYTES
}

#[no_mangle]
pub extern "C" fn conduit_book_body_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_book_body_output_len() -> usize {
    OUTPUT_LEN.with(|length| *length.borrow())
}

#[no_mangle]
pub extern "C" fn conduit_book_body_admit_source_interaction(
    source_length: usize,
    sequence: u64,
) -> i32 {
    clear_output();
    SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = None);
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result =
            crate::book_runner::interaction::admit_source(&input[..source_length], sequence);
        input[..source_length].fill(0);
        match result {
            Ok(evidence) => {
                if write_output(&evidence).is_err() {
                    return ERROR_OUTPUT;
                }
                SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = Some(evidence));
                STATUS_READY
            }
            Err(message) => refuse(message, ERROR_INTERACTION),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_book_body_birth(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    birth_sequence: u64,
) -> i32 {
    clear_output();
    let interaction = SOURCE_INTERACTION.with(|slot| slot.borrow_mut().take());
    let Some(identity_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    let Some(total_length) = identity_length.checked_add(source_length) else {
        return ERROR_INPUT;
    };
    if host_length == 0 || boot_length == 0 || source_length == 0 || total_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    let Some(interaction) = interaction else {
        return refuse(
            "source interaction was not admitted before BIRTH".into(),
            ERROR_INTERACTION,
        );
    };
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length])
                .map_err(|_| "Host identity is not UTF-8".to_string())?;
            let boot = core::str::from_utf8(&input[host_length..identity_length])
                .map_err(|_| "Boot identity is not UTF-8".to_string())?;
            let source = core::str::from_utf8(&input[identity_length..total_length])
                .map_err(|_| "Body Seed source is not UTF-8".to_string())?;
            session::birth(host, boot, source, birth_sequence, interaction)
        })();
        input[..total_length].fill(0);
        match result {
            Ok(receipt) => write_output(&receipt)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => refuse(message, ERROR_BIRTH),
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_book_body_current() -> i32 {
    clear_output();
    match session::current() {
        Some(receipt) => write_output(&receipt)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        None => STATUS_ABSENT,
    }
}

fn refuse(message: String, code: i32) -> i32 {
    if write_output(&crate::book_runner::refusal(message)).is_err() {
        ERROR_OUTPUT
    } else {
        code
    }
}

fn write_output(value: &impl serde::Serialize) -> Result<(), ()> {
    let encoded = serde_json::to_vec(value).map_err(|_| ())?;
    if encoded.len() > OUTPUT_BYTES {
        return Err(());
    }
    OUTPUT.with(|output| output.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    Ok(())
}

fn clear_output() {
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
}
