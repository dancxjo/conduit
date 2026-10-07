//! Separately linked pure implementation image; no Root/provider code.
#![no_std]
#![no_main]

mod frame;
mod gate;
mod memory;
#[cfg(domain_proof)]
mod probes;
use frame::{TEXT_CAPACITY, TextFrame};
#[path = "../src/text_transform.rs"]
mod text_transform;

/// The supervisor admits this exact bounded input/output page before entry.
/// This layout is mechanism ABI, never authored Plot or semantic identity.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn domain_entry(frame: *mut TextFrame) -> ! {
    let frame = unsafe { &mut *frame };
    #[cfg(domain_proof)]
    if frame.probe != 0 {
        unsafe { probes::run(frame) }
    }
    let length = frame.input_length as usize;
    frame.output_length = 0;
    frame.status = if length > frame.input.len() || frame.capacity != TEXT_CAPACITY as u32 {
        1
    } else {
        match text_transform::uppercase_into(&frame.input[..length], &mut frame.output) {
            Ok(length) => {
                frame.output_length = length as u32;
                0
            }
            Err(text_transform::UppercaseError::MalformedUtf8) => 1,
            Err(text_transform::UppercaseError::OutputOverflow) => 2,
        }
    };
    gate::finish(frame.status)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    gate::finish(3)
}
