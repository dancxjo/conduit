//! Compatibility exports for consumers compiled against the former Book-owned ABI.

use super::abi;

#[no_mangle]
pub extern "C" fn conduit_book_body_input_ptr() -> usize {
    abi::conduit_creche_input_ptr()
}
#[no_mangle]
pub extern "C" fn conduit_book_body_input_capacity() -> usize {
    abi::conduit_creche_input_capacity()
}
#[no_mangle]
pub extern "C" fn conduit_book_body_output_ptr() -> usize {
    abi::conduit_creche_output_ptr()
}
#[no_mangle]
pub extern "C" fn conduit_book_body_output_len() -> usize {
    abi::conduit_creche_output_len()
}
#[no_mangle]
pub extern "C" fn conduit_book_body_admit_source_interaction(
    source_length: usize,
    sequence: u64,
) -> i32 {
    abi::conduit_creche_admit_source_interaction(source_length, sequence)
}
#[no_mangle]
pub extern "C" fn conduit_book_body_birth(
    host_length: usize,
    boot_length: usize,
    friendly_name_length: usize,
    initial_program_length: usize,
    source_length: usize,
    birth_sequence: u64,
) -> i32 {
    abi::conduit_creche_birth(
        host_length,
        boot_length,
        friendly_name_length,
        initial_program_length,
        source_length,
        birth_sequence,
    )
}
#[no_mangle]
pub extern "C" fn conduit_book_body_attach_here(
    host_length: usize,
    boot_length: usize,
    sequence: u64,
) -> i32 {
    abi::conduit_creche_attach_here(host_length, boot_length, sequence)
}
#[no_mangle]
pub extern "C" fn conduit_book_body_current() -> i32 {
    abi::conduit_creche_current()
}
#[no_mangle]
pub extern "C" fn conduit_book_body_biography() -> i32 {
    abi::conduit_creche_biography()
}
#[no_mangle]
pub extern "C" fn conduit_book_body_graduation_readiness() -> i32 {
    abi::conduit_creche_graduation_readiness()
}
#[no_mangle]
pub extern "C" fn conduit_book_body_graduate(choice: u32, sequence: u64) -> i32 {
    abi::conduit_creche_graduate(choice, sequence)
}
#[no_mangle]
pub extern "C" fn conduit_book_body_prepare_physical_spore(now_millis: u64) -> i32 {
    abi::conduit_creche_prepare_physical_spore(now_millis)
}
#[no_mangle]
pub extern "C" fn conduit_book_body_prepare_selected_physical_spore(
    digest_length: usize,
    now_millis: u64,
) -> i32 {
    abi::conduit_creche_prepare_selected_physical_spore(digest_length, now_millis)
}
#[no_mangle]
pub extern "C" fn conduit_book_body_admit_physical_spore(length: usize) -> i32 {
    abi::conduit_creche_admit_physical_spore(length)
}
