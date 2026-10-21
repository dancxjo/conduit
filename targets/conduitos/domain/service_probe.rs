//! Proof-only accesses through the actual service address space.
use crate::{frame::TextFrame, gate};

pub unsafe fn run(frame: &mut TextFrame) -> ! {
    match frame.probe {
        18 if frame.input_length == 8 => {
            let mut value = [0; 8];
            value.copy_from_slice(&frame.input[..8]);
            unsafe { (frame.target as *mut u64).write_volatile(u64::from_le_bytes(value)) };
            frame.output_length = 0;
        }
        19 => {
            let value = unsafe { (frame.target as *const u64).read_volatile() };
            frame.output[..8].copy_from_slice(&value.to_le_bytes());
            frame.output_length = 8;
        }
        20 => {
            let cs: u16;
            unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack)) };
            frame.output[..8].copy_from_slice(&u64::from(cs & 3).to_le_bytes());
            frame.output_length = 8;
        }
        _ => gate::finish(3),
    }
    frame.status = 0;
    gate::finish(0)
}
