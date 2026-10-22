//! Proof-only accesses through the actual service address space.
use crate::{frame::TextFrame, gate};
#[path = "service_bulk.rs"]
mod service_bulk;

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
        22 => {
            let cs: u16;
            unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack)) };
            if cs & 3 != 3 {
                gate::finish(3);
            }

            let descriptor = usize::try_from(frame.input_length)
                .ok()
                .and_then(|length| frame.input.get(..length))
                .ok_or(service_bulk::Refusal::Length)
                .and_then(service_bulk::Descriptor::decode)
                .and_then(|descriptor| {
                    descriptor.validate()?;
                    Ok(descriptor)
                });
            let descriptor = match descriptor {
                Ok(value) => value,
                Err(reason) => {
                    frame.status = 0x100 + reason as u32;
                    frame.output_length = 0;
                    gate::finish(0);
                }
            };
            // No large guest array, allocation, or extra work/entry grant.
            // Volatile byte accesses ensure every admitted byte is transformed.
            for index in 0..descriptor.length {
                let address = (descriptor.start + descriptor.offset + index) as *mut u8;
                unsafe { address.write_volatile(address.read_volatile() ^ service_bulk::XOR) };
            }
            frame.output[..service_bulk::BYTES].copy_from_slice(&descriptor.returned().encode());
            frame.output_length = service_bulk::BYTES as u32;
        }
        _ => gate::finish(3),
    }
    frame.status = 0;
    gate::finish(0)
}
