const COM1: u16 = 0x3f8;
const DEBUG_EXIT: u16 = 0xf4;
const SERIAL_SPIN_LIMIT: u32 = 100_000;
const RDRAND_ATTEMPTS: usize = 32;

/// Initialize and write COM1 synchronously during the bounded bootstrap proof.
pub fn early_write(bytes: &[u8]) {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xc7);
        outb(COM1 + 4, 0x0b);

        for &byte in bytes {
            let mut spins = 0;
            while inb(COM1 + 5) & 0x20 == 0 && spins < SERIAL_SPIN_LIMIT {
                spins += 1;
                core::hint::spin_loop();
            }
            outb(COM1, byte);
        }
    }
}

/// Gather a bounded boot nonce. RDRAND is preferred; the fallback still binds
/// the exact boot timestamp, TSC, and loaded image address supplied by Limine.
pub fn boot_entropy(timestamp: u64, image_address: u64) -> [u64; 4] {
    let mut words = [0; 4];
    let rdrand_available = core::arch::x86_64::__cpuid(1).ecx & (1 << 30) != 0;
    for (index, word) in words.iter_mut().enumerate() {
        let mut accepted = false;
        for _ in 0..if rdrand_available { RDRAND_ATTEMPTS } else { 0 } {
            let candidate: u64;
            let success: u8;
            unsafe {
                core::arch::asm!(
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
            let low: u32;
            let high: u32;
            unsafe {
                core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nostack, nomem));
            }
            *word = ((high as u64) << 32)
                ^ low as u64
                ^ timestamp.rotate_left(index as u32 * 11)
                ^ image_address.rotate_right(index as u32 * 7)
                ^ index as u64;
        }
    }
    words
}

/// Exit QEMU through its explicitly configured isa-debug-exit device.
pub fn deterministic_exit(success: bool) -> ! {
    unsafe { outb(DEBUG_EXIT, if success { 0x10 } else { 0x11 }) };
    loop {
        core::hint::spin_loop();
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem));
    }
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nostack, nomem));
    }
    value
}
