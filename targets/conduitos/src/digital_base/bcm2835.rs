//! Single-pin BCM2835 GPIO mechanism, without peripheral or button semantics.
use super::{DigitalDisposition, DigitalProvider, DigitalRequest};
const SET: usize = 0x1c;
const CLEAR: usize = 0x28;
const LEVEL: usize = 0x34;

/// # Safety
/// The machine owner retains the exact selected pin exclusively and guarantees
/// ordered, bounded register access to the BCM2835 GPIO window. Function-select
/// read/modify/write must be serialized against other admissions and firmware.
/// Electrical configuration and external pulls are independently established.
pub unsafe trait Registers {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Input,
    Output { initial_high: bool, safe_high: bool },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionRefusal {
    InvalidPin,
    Busy,
}

pub struct Pin<R: Registers> {
    registers: R,
    function_offset: usize,
    function_shift: u8,
    bank_offset: usize,
    mask: u32,
    direction: Direction,
    revoked: bool,
}
impl<R: Registers> Pin<R> {
    /// Admit only an unconfigured input pin. Existing alternate functions and
    /// output owners refuse without writes. This constructor grants no authority.
    pub fn admit(
        mut registers: R,
        pin: u8,
        direction: Direction,
    ) -> Result<Self, AdmissionRefusal> {
        if pin > 53 {
            return Err(AdmissionRefusal::InvalidPin);
        }
        let function_offset = usize::from(pin / 10) * 4;
        let function_shift = (pin % 10) * 3;
        let function = registers.read(function_offset);
        if (function >> function_shift) & 7 != 0 {
            return Err(AdmissionRefusal::Busy);
        }
        let mut selected = Self {
            registers,
            function_offset,
            function_shift,
            bank_offset: usize::from(pin / 32) * 4,
            mask: 1 << (pin % 32),
            direction,
            revoked: false,
        };
        if let Direction::Output { initial_high, .. } = direction {
            // Set the latch while still an input; enabling output cannot glitch
            // through an old, unknown latch value. Preserve unrelated functions.
            selected.drive(initial_high);
            selected
                .registers
                .write(function_offset, function | (1 << function_shift));
        }
        Ok(selected)
    }
    fn drive(&mut self, high: bool) {
        self.registers
            .write(if high { SET } else { CLEAR } + self.bank_offset, self.mask);
    }
    fn owns_function(&mut self) -> bool {
        let expected = u32::from(matches!(self.direction, Direction::Output { .. }));
        (self.registers.read(self.function_offset) >> self.function_shift) & 7 == expected
    }
}
impl<R: Registers> DigitalProvider for Pin<R> {
    fn access(&mut self, request: DigitalRequest) -> Result<bool, DigitalDisposition> {
        if self.revoked {
            return Err(DigitalDisposition::ProviderLost);
        }
        if !self.owns_function() {
            // Do not overwrite a function now controlled by an unknown owner.
            self.revoked = true;
            return Err(DigitalDisposition::ProviderLost);
        }
        if let DigitalRequest::Drive(high) = request {
            if self.direction == Direction::Input {
                return Err(DigitalDisposition::Unsupported);
            }
            self.drive(high);
        }
        Ok(self.registers.read(LEVEL + self.bank_offset) & self.mask != 0)
    }
    fn revoke(&mut self) {
        if self.revoked {
            return;
        }
        if self.owns_function() {
            if let Direction::Output { safe_high, .. } = self.direction {
                self.drive(safe_high);
            }
        }
        // Keep an owned output driven at its declared safe level. Floating it
        // is not equivalent to a safe level. Re-admission needs a Root handoff.
        self.revoked = true;
    }
}
impl<R: Registers> Drop for Pin<R> {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(test)]
#[path = "bcm2835_tests.rs"]
mod tests;
