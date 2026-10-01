//! Product-neutral checked-Form projection ABI beside legacy Tour callers.

use super::*;

#[no_mangle]
pub extern "C" fn conduit_browser_projection_input_ptr() -> usize {
    conduit_tour_input_ptr()
}

#[no_mangle]
pub extern "C" fn conduit_browser_projection_input_capacity() -> usize {
    conduit_tour_input_capacity()
}

#[no_mangle]
pub extern "C" fn conduit_browser_projection_output_ptr() -> usize {
    conduit_tour_output_ptr()
}

#[no_mangle]
pub extern "C" fn conduit_browser_projection_output_len() -> usize {
    conduit_tour_output_len()
}

/// Projects the exact checked Form beside its Tour source without planning or
/// starting a Play. Retained temporarily for the legacy Tour consumer.
#[no_mangle]
pub extern "C" fn conduit_tour_project_patchbay(source_length: usize, sequence: u64) -> i32 {
    project_patchbay(source_length, sequence, false, false)
}

/// Projects one exact checked Form through the product-neutral browser ABI.
/// External callers never receive the Tour-owned contract or parse source in
/// JavaScript.
#[no_mangle]
pub extern "C" fn conduit_browser_project_patchbay(source_length: usize, sequence: u64) -> i32 {
    project_patchbay(source_length, sequence, false, true)
}

/// Projects the same visible checked Form while retaining distinct recursive
/// expansion evidence for the comparison lesson.
#[no_mangle]
pub extern "C" fn conduit_tour_project_patchbay_recursive(
    source_length: usize,
    sequence: u64,
) -> i32 {
    project_patchbay(source_length, sequence, true, false)
}

fn project_patchbay(
    source_length: usize,
    sequence: u64,
    recursive: bool,
    product_neutral: bool,
) -> i32 {
    clear_output();
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = core::str::from_utf8(&input[..source_length])
            .map_err(|_| "checked-Form Patchbay source is not UTF-8".to_owned())
            .and_then(|source| {
                if product_neutral {
                    super::super::compact_patchbay::project_form(source, sequence)
                } else {
                    super::super::compact_patchbay::project(source, sequence, recursive)
                }
            });
        input[..source_length].fill(0);
        match result {
            Ok(projection) => write_output(&projection)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => {
                let _ = write_output(&super::super::refusal(message));
                ERROR_PROJECTION
            }
        }
    })
}
