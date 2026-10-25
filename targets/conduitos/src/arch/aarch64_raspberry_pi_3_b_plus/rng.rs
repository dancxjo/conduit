//! Bounded BCM2837B0 boot RNG acquisition; not yet wired into a Pi product.
//!
//! Board/address provenance:
//! - https://www.raspberrypi.com/documentation/computers/processors.html#bcm2837b0
//! - https://github.com/torvalds/linux/blob/master/arch/arm/boot/dts/broadcom/bcm2837-rpi-3-b-plus.dts
//! - https://github.com/torvalds/linux/blob/master/arch/arm/boot/dts/broadcom/bcm2837.dtsi
//! - https://github.com/torvalds/linux/blob/master/arch/arm/boot/dts/broadcom/bcm2835-common.dtsi
//! Register protocol reference (independently implemented here):
//! https://github.com/torvalds/linux/blob/master/drivers/char/hw_random/bcm2835-rng.c
//!
//! The DTS maps bus 0x7e104000 to CPU physical 0x3f104000, extent 0x10.
//! STATUS's high byte counts available words; writing 0x40000 establishes
//! hardware warmup before CTRL bit 0 enables generation. INT_MASK at 0x10 is
//! used for other compatibles by Linux; it is outside this device's extent.
//! These conservative stuck/repeated-word checks are boot refusal policy,
//! not entropy estimation, conditioning, certification or physical proof.

/// CPU physical address, never an automatically valid Rust/MMIO pointer.
pub const PHYSICAL_BASE: usize = 0x3f10_4000;
pub const REGISTER_BYTES: usize = 0x10;
/// Maximum STATUS reads across warmup and the complete acquisition combined.
pub const MAXIMUM_STATUS_POLLS: u32 = 1_000_000;
pub const SEED_BYTES: usize = 32;
const CTRL: usize = 0;
const STATUS: usize = 4;
const DATA: usize = 8;
const ENABLE: u32 = 1;
const WARMUP: u32 = 0x40000;
const SAMPLE_WORDS: usize = 1 + SEED_BYTES / 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RngRefusal {
    InvalidMapping,
    InvalidBudget,
    Unavailable,
    PollBudgetExhausted,
    Unhealthy,
}

/// Acquire one 32-byte boot seed, with no clock-derived or deterministic fallback.
/// The first available word is withheld and participates in repetition checking.
/// Any refusal disables the generator after initialization; no partial seed is
/// returned. A successful acquisition also disables it before returning.
///
/// `status_poll_budget` is a hard work bound, not a calibrated time deadline:
/// at most that many STATUS reads, nine DATA reads, and five CTRL accesses.
/// It cannot bound a hardware bus transaction that never completes.
///
/// # Safety
/// The caller must establish the exact Pi 3B+ BCM2837B0 device and exclusively
/// own its RNG for this entire call, including against firmware/other cores.
/// `mapped_base..mapped_base+REGISTER_BYTES` must remain a live writable mapping
/// of PHYSICAL_BASE with Device-nGnRnE attributes, 32-bit aligned, little-endian
/// AArch64 accesses, and no conflicting aliases. Clocks/reset/power must permit
/// register access. The caller owns fault containment; inaccessible MMIO can
/// fault rather than return a refusal. This function does not configure MMU,
/// clocks, interrupts or infer ownership from an address. No RNG interrupt may
/// be routed/enabled by another owner during this polling acquisition.
#[cfg(target_arch = "aarch64")]
pub unsafe fn read_boot_seed(
    mapped_base: usize,
    status_poll_budget: u32,
) -> Result<[u8; SEED_BYTES], RngRefusal> {
    validate_mapping(mapped_base)?;
    let mut registers = Mmio { base: mapped_base };
    collect(&mut registers, status_poll_budget)
}

fn validate_mapping(base: usize) -> Result<(), RngRefusal> {
    if base == 0 || base & 3 != 0 || base.checked_add(REGISTER_BYTES).is_none() {
        return Err(RngRefusal::InvalidMapping);
    }
    Ok(())
}

trait Registers {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
}

#[cfg(target_arch = "aarch64")]
struct Mmio {
    base: usize,
}

#[cfg(target_arch = "aarch64")]
impl Registers for Mmio {
    fn read(&mut self, offset: usize) -> u32 {
        // SAFETY: read_boot_seed's caller supplies the exclusive Device mapping;
        // offsets are private fixed aligned registers inside the checked extent.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let value = ((self.base + offset) as *const u32).read_volatile();
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            u32::from_le(value)
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        // SAFETY: same mapping contract as read; barriers order device accesses.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            ((self.base + offset) as *mut u32).write_volatile(value.to_le());
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
}

fn collect(registers: &mut impl Registers, mut polls: u32) -> Result<[u8; SEED_BYTES], RngRefusal> {
    if polls == 0 || polls > MAXIMUM_STATUS_POLLS {
        return Err(RngRefusal::InvalidBudget);
    }
    // Reestablish the driver's hardware warmup protocol under exclusive custody.
    registers.write(CTRL, 0);
    registers.write(STATUS, WARMUP);
    registers.write(CTRL, ENABLE);
    let result = (|| {
        let control = registers.read(CTRL);
        if control == u32::MAX || control & ENABLE == 0 {
            return Err(RngRefusal::Unavailable);
        }
        let mut words = [0_u32; SAMPLE_WORDS];
        for index in 0..SAMPLE_WORDS {
            loop {
                if polls == 0 {
                    return Err(RngRefusal::PollBudgetExhausted);
                }
                polls -= 1;
                let status = registers.read(STATUS);
                if status == u32::MAX {
                    return Err(RngRefusal::Unavailable);
                }
                if status >> 24 != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            let word = registers.read(DATA);
            if word == 0 || word == u32::MAX || words[..index].contains(&word) {
                return Err(RngRefusal::Unhealthy);
            }
            words[index] = word;
        }
        let control = registers.read(CTRL);
        if control == u32::MAX || control & ENABLE == 0 {
            return Err(RngRefusal::Unavailable);
        }
        let mut seed = [0_u8; SEED_BYTES];
        for (bytes, word) in seed.chunks_exact_mut(4).zip(&words[1..]) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        Ok(seed)
    })();
    registers.write(CTRL, 0);
    result
}

#[cfg(test)]
#[path = "rng/tests.rs"]
mod tests;
