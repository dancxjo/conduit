use core::arch::{asm, x86_64::__cpuid};

use crate::offer::CpuFeatures;

use super::io::outl;

const DEBUG_EXIT: u16 = 0xf4;
const RDRAND_ATTEMPTS: usize = 32;

pub fn feature_basis() -> CpuFeatures {
    let basic = __cpuid(1);
    let maximum_extended = __cpuid(0x8000_0000).eax;
    let invariant_tsc = maximum_extended >= 0x8000_0007 && __cpuid(0x8000_0007).edx & (1 << 8) != 0;
    CpuFeatures {
        sse2: basic.edx & (1 << 26) != 0,
        rdrand: basic.ecx & (1 << 30) != 0,
        invariant_tsc,
    }
}

pub fn boot_entropy(timestamp: u64, image_address: u64) -> [u64; 4] {
    let mut words = [0; 4];
    let rdrand_available = feature_basis().rdrand;
    for (index, word) in words.iter_mut().enumerate() {
        let mut accepted = false;
        for _ in 0..if rdrand_available { RDRAND_ATTEMPTS } else { 0 } {
            let candidate: u64;
            let success: u8;
            unsafe {
                asm!(
                    "rdrand {candidate}",
                    "setc {success}",
                    candidate = out(reg) candidate,
                    success = out(reg_byte) success,
                    options(nostack, nomem)
                );
            }
            if success != 0 {
                *word = candidate;
                accepted = true;
                break;
            }
        }
        if !accepted {
            *word = read_tsc()
                ^ timestamp.rotate_left(index as u32 * 11)
                ^ image_address.rotate_right(index as u32 * 7)
                ^ index as u64;
        }
    }
    words
}

pub(super) fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { asm!("rdtsc", out("eax") low, out("edx") high, options(nostack, nomem)) };
    (u64::from(high) << 32) | u64::from(low)
}

pub(super) fn enable_interrupts() {
    unsafe { asm!("sti", options(nostack, nomem)) };
}

pub(super) fn disable_interrupts() {
    unsafe { asm!("cli", options(nostack, nomem)) };
}

pub(super) fn interrupts_enabled() -> bool {
    let flags: u64;
    unsafe { asm!("pushfq", "pop {}", out(reg) flags, options(nomem)) };
    flags & (1 << 9) != 0
}

pub(super) fn interruptible_idle() {
    unsafe { asm!("sti", "hlt", "cli", options(nostack, nomem)) };
}

pub fn deterministic_exit(success: bool) -> ! {
    unsafe { outl(DEBUG_EXIT, if success { 0x10 } else { 0x11 }) };
    loop {
        unsafe { asm!("cli", "hlt", options(nostack, nomem)) };
    }
}
