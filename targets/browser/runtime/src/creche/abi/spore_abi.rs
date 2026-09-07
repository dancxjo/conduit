//! Bounded ABI entrances for Body-owned physical-spore preparation and admission.

use super::{
    clear_output, refuse, write_output, ERROR_ADMISSION, ERROR_INPUT, ERROR_OUTPUT, ERROR_SPORE,
    INPUT, INPUT_BYTES, STATUS_READY,
};
use crate::creche::spore;

#[no_mangle]
pub extern "C" fn conduit_creche_prepare_physical_spore(now_millis: u64) -> i32 {
    clear_output();
    let entropy = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let mut entropy = [0u8; 32];
        entropy.copy_from_slice(&input[..32]);
        input[..32].fill(0);
        entropy
    });
    finish(spore::prepare(entropy, now_millis), ERROR_SPORE)
}

#[no_mangle]
pub extern "C" fn conduit_creche_prepare_selected_physical_spore(
    digest_length: usize,
    now_millis: u64,
) -> i32 {
    clear_output();
    if digest_length == 0 || 32usize.saturating_add(digest_length) > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let mut entropy = [0u8; 32];
        entropy.copy_from_slice(&input[..32]);
        let result = core::str::from_utf8(&input[32..32 + digest_length])
            .map_err(|_| "selected IMAGE content digest is not UTF-8".to_string())
            .and_then(|digest| spore::prepare_selected(entropy, now_millis, Some(digest)));
        input[..32 + digest_length].fill(0);
        finish(result, ERROR_SPORE)
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_prepare_selected_physical_spore_for_target(
    target_length: usize,
    digest_length: usize,
    now_millis: u64,
) -> i32 {
    prepare_for_target(target_length, digest_length, 0, now_millis)
}

#[no_mangle]
pub extern "C" fn conduit_creche_prepare_selected_physical_spore_for_target_with_image(
    target_length: usize,
    digest_length: usize,
    image_length: usize,
    now_millis: u64,
) -> i32 {
    prepare_for_target(target_length, digest_length, image_length, now_millis)
}

fn prepare_for_target(
    target_length: usize,
    digest_length: usize,
    image_length: usize,
    now_millis: u64,
) -> i32 {
    clear_output();
    let total_length = 32usize
        .checked_add(target_length)
        .and_then(|length| length.checked_add(digest_length))
        .and_then(|length| length.checked_add(image_length));
    if target_length == 0
        || digest_length == 0
        || total_length.is_none_or(|length| length > INPUT_BYTES)
    {
        return ERROR_INPUT;
    }
    let total_length = total_length.expect("validated bounded input length");
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let mut entropy = [0u8; 32];
        entropy.copy_from_slice(&input[..32]);
        let target_end = 32 + target_length;
        let digest_end = target_end + digest_length;
        let result = (|| {
            let target = core::str::from_utf8(&input[32..target_end])
                .map_err(|_| "selected physical Host target is not UTF-8".to_string())?;
            let digest = core::str::from_utf8(&input[target_end..digest_end])
                .map_err(|_| "selected IMAGE content digest is not UTF-8".to_string())?;
            if image_length == 0 {
                spore::prepare_selected_for_target(entropy, now_millis, target, Some(digest))
            } else {
                spore::prepare_selected_for_target_with_image(
                    entropy,
                    now_millis,
                    target,
                    digest,
                    &input[digest_end..total_length],
                )
            }
        })();
        input[..total_length].fill(0);
        finish(result, ERROR_SPORE)
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_admit_physical_spore(length: usize) -> i32 {
    clear_output();
    if length == 0 || length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let observation = serde_json::from_slice::<spore::JoinObservation>(&input[..length]);
        input[..length].fill(0);
        match observation {
            Ok(observation) => finish(spore::admit(observation), ERROR_ADMISSION),
            Err(error) => refuse(
                format!("decode physical join request: {error}"),
                ERROR_INPUT,
            ),
        }
    })
}

fn finish<T: serde::Serialize>(result: Result<T, String>, code: i32) -> i32 {
    match result {
        Ok(receipt) => write_output(&receipt)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(message) => refuse(message, code),
    }
}
