//! Exclusive onboard ACT LED mechanism for BCM2835 Model B+ v1.2.
//! Linux bcm2835-rpi-b-plus.dts identifies GPIO47 as active high:
//! https://raw.githubusercontent.com/torvalds/linux/master/arch/arm/boot/dts/broadcom/bcm2835-rpi-b-plus.dts
//! Timing and diagnostic meanings belong to the boot composition Root.
use crate::digital_base::bcm2835::Registers;

const FUNCTION: usize = 0x10;
const SHIFT: u32 = 21;
const FUNCTION_MASK: u32 = 7 << SHIFT;
const OUTPUT: u32 = 1 << SHIFT;
const SET: usize = 0x20;
const CLEAR: usize = 0x2c;
const LED: u32 = 1 << 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    InterruptsEnabled,
    BoardUnavailable,
    WrongBoard,
    AlreadyClaimed,
    AlternateFunction,
    OwnershipLost,
    Revoked,
}

/// A single retained contact. Generic register injection supports conformance;
/// physical construction remains the unsafe, exact-board native entrance.
pub struct Indicator<R: Registers> {
    registers: R,
    owned: bool,
}
impl<R: Registers> Indicator<R> {
    fn admit(mut registers: R) -> Result<Self, Refusal> {
        let function = registers.read(FUNCTION);
        if (function >> SHIFT) & 7 > 1 {
            return Err(Refusal::AlternateFunction);
        }
        // Firmware has explicitly handed this contact over. Set its safe latch
        // before changing input to output, preserving every unrelated field.
        registers.write(CLEAR, LED);
        registers.write(FUNCTION, (function & !FUNCTION_MASK) | OUTPUT);
        Ok(Self {
            registers,
            owned: true,
        })
    }

    pub fn set(&mut self, on: bool) -> Result<(), Refusal> {
        if !self.owned {
            return Err(Refusal::Revoked);
        }
        if self.registers.read(FUNCTION) & FUNCTION_MASK != OUTPUT {
            self.owned = false;
            return Err(Refusal::OwnershipLost);
        }
        self.registers.write(if on { SET } else { CLEAR }, LED);
        Ok(())
    }

    pub fn revoke(&mut self) {
        if self.owned {
            if self.registers.read(FUNCTION) & FUNCTION_MASK == OUTPUT {
                self.registers.write(CLEAR, LED);
            }
            self.owned = false;
        }
    }
}
impl<R: Registers> Drop for Indicator<R> {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(target_arch = "arm")]
mod native {
    use super::*;
    use core::sync::atomic::{AtomicBool, Ordering};
    static CLAIMED: AtomicBool = AtomicBool::new(false);
    const GPIO: usize = 0x2020_0000;

    pub struct NativeRegisters {
        _exclusive: (),
    }
    unsafe impl Registers for NativeRegisters {
        fn read(&mut self, offset: usize) -> u32 {
            barrier();
            let value = unsafe { core::ptr::read_volatile((GPIO + offset) as *const u32) };
            barrier();
            value
        }
        fn write(&mut self, offset: usize, value: u32) {
            barrier();
            unsafe { core::ptr::write_volatile((GPIO + offset) as *mut u32, value) };
            barrier();
        }
    }
    impl Drop for NativeRegisters {
        fn drop(&mut self) {
            CLAIMED.store(false, Ordering::Release);
        }
    }
    pub type BootIndicator = Indicator<NativeRegisters>;

    /// # Safety
    /// Trusted Root must possess the exact Model B+ v1.2 ACT LED/GPIO47 and
    /// establish explicit firmware handoff, including when firmware left it an
    /// output. IRQs must be disabled, device MMIO ordered and function-select
    /// changes serialized against firmware, DMA and every other GPIO owner.
    /// Board recognition alone establishes neither handoff nor authority.
    pub unsafe fn admit() -> Result<BootIndicator, Refusal> {
        if super::super::interrupts_enabled() {
            return Err(Refusal::InterruptsEnabled);
        }
        let revision = super::super::firmware_board_revision().ok_or(Refusal::BoardUnavailable)?;
        if !super::super::minipitft::is_model_b_plus_v1_2(revision) {
            return Err(Refusal::WrongBoard);
        }
        CLAIMED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Refusal::AlreadyClaimed)?;
        Indicator::admit(NativeRegisters { _exclusive: () })
    }
    fn barrier() {
        unsafe {
            core::arch::asm!("mcr p15, 0, {zero}, c7, c10, 4", zero=in(reg)0u32, options(nostack,preserves_flags));
        }
    }
}
#[cfg(target_arch = "arm")]
pub use native::{BootIndicator, admit};

#[cfg(test)]
#[path = "boot_indicator/tests.rs"]
mod tests;
