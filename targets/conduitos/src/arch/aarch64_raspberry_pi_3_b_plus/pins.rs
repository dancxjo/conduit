//! BCM2837B0 GPIO14/15 ALT0 (UART0); unwired board integration primitive.
//!
//! Protocol: https://datasheets.raspberrypi.com/bcm2835/bcm2835-peripherals.pdf
//! sections 6.1/6.2: GPFSEL1 fields 12/15 select ALT0=100; GPPUD's
//! setup and hold each require 150 cycles. Pull state itself is NOT readable.
//! BCM2837B0 identity/architecture:
//! https://www.raspberrypi.com/documentation/computers/processors.html#bcm2837b0
//! Address translation: https://github.com/raspberrypi/linux/blob/rpi-6.12.y/arch/arm/boot/dts/broadcom/bcm2837.dtsi
//! The Pi3 peripheral CPU window is 0x3f000000 (not BCM2835's 0x20000000).

pub const PHYSICAL_BASE: usize = 0x3f20_0000;
pub const REGISTER_BYTES: usize = 0xa0;
pub const MAXIMUM_DELAY_POLLS: u32 = 1_000_000;
const FSEL1: usize = 0x04;
const PUD: usize = 0x94;
const CLK0: usize = 0x98;
const CLK1: usize = 0x9c;
const PINS: u32 = (1 << 14) | (1 << 15);
const FIELDS: u32 = (7 << 12) | (7 << 15);
const ALT0: u32 = (4 << 12) | (4 << 15);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinRefusal {
    InvalidMapping,
    InvalidTiming,
    PullControllerBusy,
    Unavailable,
    DelayBudgetExhausted,
    ReadbackMismatch,
}

/// Route GPIO14/15 to UART0 and disable their internal pulls.
/// Success reads back function selection and idle pull-control registers; it
/// cannot verify the latched pulls, UART clock, external wiring or a receiver.
/// `settle_ticks` is a caller-calibrated physical-counter interval satisfying
/// BOTH datasheet 150-cycle delays. No CPU-loop/firmware clock default is used.
/// Each delay permits one start read plus `polls_per_delay` counter reads; work is finite even if
/// the counter stops. MMIO faults/stalls are outside this work bound.
///
/// # Safety
/// Caller establishes Pi 3 Model B+ / BCM2837B0, an aligned little-endian live
/// Device-nGnRnE mapping of PHYSICAL_BASE covering REGISTER_BYTES without aliases,
/// and exclusive GPIO14/15, GPFSEL1 and ENTIRE shared GPPUD/CLK0/CLK1 custody
/// against firmware, other cores, DMA and interrupts for the call. Caller owns
/// bus-fault containment and keeps peripheral power/clocks live. The physical
/// counter must be accessible, monotonic and stable, and settle_ticks must be
/// calibrated to the documented hardware setup/hold interval (at least 150
/// cycles of the relevant peripheral clock); this API cannot validate calibration.
/// UART0 must be quiescent and its alternate routes (including Bluetooth pins)
/// released/disabled by their owner before routing. External circuitry must
/// tolerate reconfiguration. This grants no UART, clock or Bluetooth ownership.
/// A refusal after writes can leave pulls disabled or pin functions changed;
/// caller must retain custody and must not enable UART on refusal.
#[cfg(target_arch = "aarch64")]
pub unsafe fn route_uart0(
    mapped_base: usize,
    settle_ticks: u32,
    polls_per_delay: u32,
) -> Result<(), PinRefusal> {
    validate_mapping(mapped_base)?;
    route(
        &mut Mmio { base: mapped_base },
        settle_ticks,
        polls_per_delay,
    )
}

fn validate_mapping(base: usize) -> Result<(), PinRefusal> {
    if base == 0 || base & 3 != 0 || base.checked_add(REGISTER_BYTES).is_none() {
        return Err(PinRefusal::InvalidMapping);
    }
    Ok(())
}

trait Registers {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
    fn counter(&mut self) -> u64;
}

fn idle(r: &mut impl Registers) -> Result<(), PinRefusal> {
    let pud = r.read(PUD);
    let clk0 = r.read(CLK0);
    let clk1 = r.read(CLK1);
    if [pud, clk0, clk1].contains(&u32::MAX) {
        return Err(PinRefusal::Unavailable);
    }
    if pud & 3 != 0 || clk0 != 0 || clk1 != 0 {
        return Err(PinRefusal::PullControllerBusy);
    }
    Ok(())
}

fn delay(r: &mut impl Registers, ticks: u32, polls: u32) -> Result<(), PinRefusal> {
    let start = r.counter();
    for _ in 0..polls {
        if r.counter().wrapping_sub(start) >= u64::from(ticks) {
            return Ok(());
        }
    }
    Err(PinRefusal::DelayBudgetExhausted)
}

fn route(r: &mut impl Registers, ticks: u32, polls: u32) -> Result<(), PinRefusal> {
    if ticks == 0 || polls == 0 || polls > MAXIMUM_DELAY_POLLS {
        return Err(PinRefusal::InvalidTiming);
    }
    idle(r)?;
    let original = r.read(FSEL1);
    if original == u32::MAX {
        return Err(PinRefusal::Unavailable);
    }
    // Idle precondition ensures no unrelated pad can receive the shared signal.
    r.write(PUD, 0);
    let result = (|| {
        delay(r, ticks, polls)?;
        r.write(CLK0, PINS);
        delay(r, ticks, polls)
    })();
    // Always remove shared control/clock, including on either delay timeout.
    r.write(PUD, 0);
    r.write(CLK0, 0);
    result?;
    idle(r)?;
    let selected = (original & !FIELDS) | ALT0;
    r.write(FSEL1, selected);
    if r.read(FSEL1) != selected {
        return Err(PinRefusal::ReadbackMismatch);
    }
    Ok(())
}

#[cfg(target_arch = "aarch64")]
struct Mmio {
    base: usize,
}
#[cfg(target_arch = "aarch64")]
impl Registers for Mmio {
    fn read(&mut self, offset: usize) -> u32 {
        // SAFETY: route_uart0 requires exclusive Device mapping; fixed offsets
        // are aligned and inside its validated extent.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let value = ((self.base + offset) as *const u32).read_volatile();
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            u32::from_le(value)
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        // SAFETY: same mapping contract, barriers complete device writes.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            ((self.base + offset) as *mut u32).write_volatile(value.to_le());
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
    fn counter(&mut self) -> u64 {
        let value;
        // SAFETY: caller establishes accessible physical counter.
        unsafe {
            core::arch::asm!("isb", "mrs {}, cntpct_el0", out(reg) value,
                options(nostack, preserves_flags));
        }
        value
    }
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
