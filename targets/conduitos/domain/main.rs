//! Separately linked pure implementation image; no Root/provider code.
#![no_std]
#![no_main]

mod allocation;
mod frame;
mod gate;
mod keymap;
mod layout;
mod memory;
mod morse;
#[cfg(feature = "proof")]
mod probe_gate;
#[cfg(all(feature = "proof", target_arch = "x86_64"))]
mod probes;
#[cfg(all(feature = "proof", target_arch = "x86"))]
#[path = "probes_ia32.rs"]
mod probes;
#[cfg(all(feature = "proof", target_arch = "aarch64"))]
#[path = "probes_aarch64.rs"]
mod probes;
#[cfg(all(feature = "proof", target_arch = "riscv64"))]
#[path = "probes_riscv64.rs"]
mod probes;
#[cfg(all(feature = "proof", target_arch = "loongarch64"))]
#[path = "probes_loongarch64.rs"]
mod probes;
mod timer;
#[path = "../src/tour_timer_runtime.rs"]
mod timer_runtime;
use frame::{TEXT_CAPACITY, TextFrame};
#[path = "../src/text_transform.rs"]
mod text_transform;

/// The supervisor admits this exact bounded input/output page before entry.
/// This layout is mechanism ABI, never authored Plot or semantic identity.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn domain_entry(frame: *mut TextFrame) -> ! {
    let frame = unsafe { &mut *frame };
    #[cfg(feature = "proof")]
    if frame.probe != 0 && frame.command == 0 {
        unsafe { probes::run(frame) }
    }
    let length = frame.input_length as usize;
    match frame.command {
        0 => {}
        3 => unsafe { keymap::initialize(frame) },
        4 => unsafe { keymap::apply(frame) },
        5 => unsafe { keymap::chain(frame) },
        6 => unsafe { keymap::initialize_editor(frame) },
        7 => unsafe { keymap::edit_chain(frame) },
        8 => morse::chain(frame),
        10..=13 => unsafe { timer::execute(frame) },
        9 => {
            if frame.morse_length as usize > frame::MORSE_CAPACITY
                || frame.capacity != frame::MORSE_CAPACITY as u32
            {
                gate::finish(3);
            }
            gate::finish(0x200);
        }
        1 => {
            if length > TEXT_CAPACITY || frame.capacity != TEXT_CAPACITY as u32 {
                gate::finish(3);
            }
            #[cfg(feature = "proof")]
            probes::mutate_gate(frame);
            // Whole-buffer effect request. The opaque handle grants nothing here;
            // only Root can admit it and operate its selected serial Base.
            gate::finish(0x200);
        }
        2 => {
            #[cfg(feature = "proof")]
            if frame.probe == 14 {
                gate::finish(0x200);
            }
            gate::finish(frame.status)
        }
        _ => gate::finish(3),
    }
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
