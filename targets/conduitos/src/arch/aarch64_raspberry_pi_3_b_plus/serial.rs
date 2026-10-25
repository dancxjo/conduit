//! Bounded early Pi 3B+ PL011 transmit, requiring an already configured device.
//!
//! Register protocol: BCM2835 ARM Peripherals, chapter 13:
//! https://datasheets.raspberrypi.com/bcm2835/bcm2835-peripherals.pdf
//! Address translation and UART/pin ownership:
//! https://github.com/torvalds/linux/blob/master/arch/arm/boot/dts/broadcom/bcm2837.dtsi
//! https://github.com/torvalds/linux/blob/master/arch/arm/boot/dts/broadcom/bcm283x.dtsi
//! https://github.com/torvalds/linux/blob/master/arch/arm/boot/dts/broadcom/bcm2837-rpi-3-b-plus.dts
//! https://www.raspberrypi.com/documentation/computers/configuration.html#configuring-uarts
//!
//! UART0 bus 0x7e201000 maps to CPU physical 0x3f201000. Pi 3B+ defaults
//! UART0 to Bluetooth, NOT the header. Entry/firmware integration must explicitly
//! release Bluetooth and route UART0 TXD0/RXD0 to GPIO14/15 ALT0 for header use,
//! establish pulls and a stable measured/configured UART reference clock, enable
//! UARTEN/TXE with FIFO-enabled 8N1 and disable its interrupts. An overlay declaration alone
//! is not proof these effects occurred. This module changes none of those states.
//! It never assumes a 48MHz clock and is not a provider-readiness claim.

pub const PHYSICAL_BASE: usize = 0x3f20_1000;
pub const REGISTER_BYTES: usize = 0x200;
pub const MAXIMUM_STATUS_POLLS: u32 = 1_000_000;
pub const MAXIMUM_WRITE_BYTES: usize = 4096;
const DR: usize = 0;
const FR: usize = 0x18;
const IBRD: usize = 0x24;
const FBRD: usize = 0x28;
const LCRH: usize = 0x2c;
const CR: usize = 0x30;
const IMSC: usize = 0x38;
const BUSY: u32 = 1 << 3;
const TXFF: u32 = 1 << 5;
const TXFE: u32 = 1 << 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerialRefusal {
    InvalidMapping,
    InvalidClockOrBaud,
    InvalidBudget,
    MessageTooLong,
    ConfigurationMismatch,
    Unavailable,
    PollBudgetExhausted,
}

/// Bytes accepted into the UART before refusal; they cannot be rolled back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransmitRefusal {
    pub accepted_bytes: usize,
    pub reason: SerialRefusal,
}

/// Exclusive, non-cloneable custody of an already initialized PL011.
#[cfg(target_arch = "aarch64")]
pub struct EarlySerial {
    registers: Mmio,
    divisor: u32,
}

#[cfg(target_arch = "aarch64")]
impl EarlySerial {
    /// Adopt and check an existing configuration without writing control/GPIO.
    /// The expected divisor is nearest-rounded UARTCLK * 4 / baud (six fractional
    /// bits); UARTCLK must remain exact and stable throughout this object's life.
    ///
    /// # Safety
    /// Caller establishes this exact BCM2837B0 device and exclusive UART ownership
    /// against other cores, firmware, DMA and interrupt handlers for the returned
    /// object's lifetime. The aligned mapping must cover PHYSICAL_BASE's complete
    /// REGISTER_BYTES extent as live Device-nGnRnE, little-endian 32-bit MMIO, with
    /// no conflicting aliases; clocks/reset/power and pin routing must already be
    /// established. No other agent may change them. Fault containment is caller's
    /// responsibility: inaccessible MMIO may fault or stall instead of refusing.
    /// Caller supplies the actual UART reference clock, not APB/core clock or an
    /// assumed firmware default. This API cannot verify pin routing or clock truth.
    pub unsafe fn from_preconfigured(
        mapped_base: usize,
        uart_clock_hz: u32,
        baud: u32,
    ) -> Result<Self, SerialRefusal> {
        validate_mapping(mapped_base)?;
        let divisor = expected_divisor(uart_clock_hz, baud)?;
        let mut registers = Mmio { base: mapped_base };
        check_configuration(&mut registers, divisor)?;
        Ok(Self { registers, divisor })
    }

    /// Exact bytes, no newline translation. Success means FIFO acceptance only;
    /// call flush for UART-empty evidence (neither proves a receiver observed it).
    /// Each call allows at most budget FR reads plus six configuration reads and
    /// bytes.len() DR writes. A stalled bus transaction is outside this work bound.
    pub fn write(&mut self, bytes: &[u8], budget: u32) -> Result<usize, TransmitRefusal> {
        transmit(&mut self.registers, self.divisor, bytes, budget)
    }

    /// Wait for FIFO empty AND transmitter not busy, with a finite total budget.
    pub fn flush(&mut self, budget: u32) -> Result<(), SerialRefusal> {
        flush(&mut self.registers, self.divisor, budget)
    }
}

fn validate_mapping(base: usize) -> Result<(), SerialRefusal> {
    if base == 0 || base & 3 != 0 || base.checked_add(REGISTER_BYTES).is_none() {
        return Err(SerialRefusal::InvalidMapping);
    }
    Ok(())
}

fn expected_divisor(clock: u32, baud: u32) -> Result<u32, SerialRefusal> {
    if clock == 0 || baud == 0 || u64::from(baud) * 16 > u64::from(clock) {
        return Err(SerialRefusal::InvalidClockOrBaud);
    }
    let divisor = (u64::from(clock) * 4 + u64::from(baud) / 2) / u64::from(baud);
    if !(64..=0x3f_ffff).contains(&divisor) {
        return Err(SerialRefusal::InvalidClockOrBaud);
    }
    Ok(divisor as u32)
}

trait Registers {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
}

fn check_configuration(r: &mut impl Registers, divisor: u32) -> Result<(), SerialRefusal> {
    let control = r.read(CR);
    if control == u32::MAX {
        return Err(SerialRefusal::Unavailable);
    }
    // Permit RXE and RTS output; ignore reserved read-as-dont-care bits.
    // Reject loopback, IrDA, DTR and CTS/RTS flow control.
    if control & 0xc587 != 0x101 || r.read(LCRH) & 0xff != 0x70 {
        return Err(SerialRefusal::ConfigurationMismatch);
    }
    if r.read(IBRD) & 0xffff != divisor >> 6
        || r.read(FBRD) & 0x3f != divisor & 0x3f
        || r.read(IMSC) & 0x7ff != 0
    {
        return Err(SerialRefusal::ConfigurationMismatch);
    }
    Ok(())
}

fn validate_budget(budget: u32) -> Result<(), SerialRefusal> {
    if budget == 0 || budget > MAXIMUM_STATUS_POLLS {
        return Err(SerialRefusal::InvalidBudget);
    }
    Ok(())
}

fn transmit(
    r: &mut impl Registers,
    divisor: u32,
    bytes: &[u8],
    mut budget: u32,
) -> Result<usize, TransmitRefusal> {
    let mut accepted_bytes = 0;
    let result = (|| {
        validate_budget(budget)?;
        if bytes.len() > MAXIMUM_WRITE_BYTES {
            return Err(SerialRefusal::MessageTooLong);
        }
        check_configuration(r, divisor)?;
        for byte in bytes {
            wait(r, &mut budget, |flags| flags & TXFF == 0)?;
            r.write(DR, u32::from(*byte));
            accepted_bytes += 1;
        }
        Ok(accepted_bytes)
    })();
    result.map_err(|reason| TransmitRefusal {
        accepted_bytes,
        reason,
    })
}

fn flush(r: &mut impl Registers, divisor: u32, mut budget: u32) -> Result<(), SerialRefusal> {
    validate_budget(budget)?;
    check_configuration(r, divisor)?;
    wait(r, &mut budget, |flags| flags & (TXFE | BUSY) == TXFE)
}

fn wait(
    r: &mut impl Registers,
    budget: &mut u32,
    ready: impl Fn(u32) -> bool,
) -> Result<(), SerialRefusal> {
    while *budget != 0 {
        *budget -= 1;
        let flags = r.read(FR);
        if flags == u32::MAX {
            return Err(SerialRefusal::Unavailable);
        }
        if ready(flags) {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(SerialRefusal::PollBudgetExhausted)
}

#[cfg(target_arch = "aarch64")]
struct Mmio {
    base: usize,
}
#[cfg(target_arch = "aarch64")]
impl Registers for Mmio {
    fn read(&mut self, offset: usize) -> u32 {
        // SAFETY: from_preconfigured requires exclusive live Device mapping;
        // private offsets are fixed, aligned and inside that mapping.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let value = ((self.base + offset) as *const u32).read_volatile();
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            u32::from_le(value)
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        // SAFETY: same mapping contract; barriers order all device accesses.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            ((self.base + offset) as *mut u32).write_volatile(value.to_le());
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
}

#[cfg(test)]
#[path = "serial/tests.rs"]
mod tests;
