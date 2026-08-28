use super::{execute_browser_pointer, NormalizedPointerSample};
use std::cell::RefCell;

const RECEIPT_BYTES: usize = 2_048;

thread_local! {
    static RECEIPT: RefCell<[u8; RECEIPT_BYTES]> = const { RefCell::new([0; RECEIPT_BYTES]) };
    static RECEIPT_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[no_mangle]
pub extern "C" fn conduit_browser_pointer_run(
    position_x: i32,
    position_y: i32,
    delta_x: i32,
    delta_y: i32,
    primary_pressed: i32,
    coalesced: u32,
    dropped: u32,
    queue_capacity: u32,
    sequence: u32,
) -> i32 {
    if !matches!(primary_pressed, 0 | 1) {
        return -2;
    }
    let sample = NormalizedPointerSample {
        position_x: i64::from(position_x),
        position_y: i64::from(position_y),
        delta_x: i64::from(delta_x),
        delta_y: i64::from(delta_y),
        primary_pressed: primary_pressed == 1,
        coalesced: u64::from(coalesced),
        dropped: u64::from(dropped),
        queue_capacity: u64::from(queue_capacity),
        sequence: u64::from(sequence),
    };
    let Ok(receipt) = execute_browser_pointer(sample) else {
        return -1;
    };
    let Ok(encoded) = serde_json::to_vec(&receipt) else {
        return -3;
    };
    if encoded.len() > RECEIPT_BYTES {
        return -4;
    }
    RECEIPT.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        buffer[..encoded.len()].copy_from_slice(&encoded);
    });
    RECEIPT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    0
}

#[no_mangle]
pub extern "C" fn conduit_browser_pointer_receipt_ptr() -> usize {
    RECEIPT.with(|receipt| receipt.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_browser_pointer_receipt_len() -> usize {
    RECEIPT_LEN.with(|length| *length.borrow())
}
