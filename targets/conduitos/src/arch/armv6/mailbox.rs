//! Bounded property-channel exchange over uncached, aligned Root-owned RAM.
use super::{read_counter, read32, write32};
use core::sync::atomic::{Ordering, compiler_fence};

const BASE: usize = 0x2000_b880;

pub(super) fn call(words: &mut [u32]) -> Result<(), &'static str> {
    let address = words.as_mut_ptr() as usize;
    if address & 15 != 0
        || address >= 0x4000_0000
        || words.len() < 3
        || words[0] as usize != words.len() * 4
    {
        return Err("mailbox-buffer-invalid");
    }
    let request = address as u32 | 0x4000_0008;
    let start = read_counter() as u32;
    compiler_fence(Ordering::SeqCst);
    unsafe {
        core::arch::asm!("mcr p15, 0, {0}, c7, c10, 4", in(reg) 0_u32, options(nostack));
    }
    let mut sent = false;
    for _ in 0..4_000_000 {
        if (read_counter() as u32).wrapping_sub(start) > 2_000_000 {
            break;
        }
        if !sent {
            if unsafe { read32(BASE + 0x18) } & (1 << 31) == 0 {
                unsafe {
                    write32(BASE + 0x20, request);
                }
                sent = true;
            }
        } else if unsafe { read32(BASE + 0x18) } & (1 << 30) == 0 {
            if unsafe { read32(BASE) } != request {
                continue;
            }
            unsafe {
                core::arch::asm!("mcr p15, 0, {0}, c7, c10, 4", in(reg) 0_u32, options(nostack));
            }
            compiler_fence(Ordering::SeqCst);
            return if words[1] == 0x8000_0000 {
                Ok(())
            } else {
                Err("mailbox-response-refused")
            };
        }
    }
    Err("mailbox-timeout")
}
