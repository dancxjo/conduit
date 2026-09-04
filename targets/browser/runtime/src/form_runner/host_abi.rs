//! Product-neutral browser Form execution ABI.
//!
//! The implementation deliberately delegates to the one bounded session and
//! one installed-browser registry also used by the Book. Applications are
//! envelopes around this Host-owned machinery; they do not define another
//! Kind catalog, planner, scheduler, or Play universe.

use super::abi;

#[no_mangle]
pub extern "C" fn conduit_browser_form_input_ptr() -> usize {
    abi::conduit_book_input_ptr()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_input_capacity() -> usize {
    abi::conduit_book_input_capacity()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_output_ptr() -> usize {
    abi::conduit_book_output_ptr()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_output_len() -> usize {
    abi::conduit_book_output_len()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_inventory() -> i32 {
    abi::conduit_book_inventory()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_human_machinery() -> i32 {
    abi::conduit_book_human_machinery()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_reviewed_gallery() -> i32 {
    abi::conduit_book_reviewed_gallery()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_admit_source_interaction(
    source_length: usize,
    sequence: u64,
) -> i32 {
    abi::conduit_book_admit_source_interaction(source_length, sequence)
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_start(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    play_sequence: u64,
) -> i32 {
    abi::conduit_book_start(host_length, boot_length, source_length, play_sequence)
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_start_recursive(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    play_sequence: u64,
) -> i32 {
    abi::conduit_book_start_recursive(host_length, boot_length, source_length, play_sequence)
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_complete() -> i32 {
    abi::conduit_book_complete()
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_complete_with_output(output_length: usize) -> i32 {
    abi::conduit_book_complete_with_output(output_length)
}

#[no_mangle]
pub extern "C" fn conduit_browser_form_cancel() -> i32 {
    abi::conduit_book_cancel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_form_abi_and_legacy_book_envelope_share_one_session_and_inventory() {
        assert_eq!(conduit_browser_form_inventory(), 0);
        assert_eq!(
            conduit_browser_form_output_ptr(),
            abi::conduit_book_output_ptr()
        );
        assert_eq!(
            conduit_browser_form_output_len(),
            abi::conduit_book_output_len()
        );
        assert_eq!(conduit_browser_form_complete(), -403);
    }
}
