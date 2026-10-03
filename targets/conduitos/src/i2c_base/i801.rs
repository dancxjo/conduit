//! Intel ICH-compatible SMBus physics leaf, with no peripheral protocol knowledge.
//!
//! The supported transaction subset is send/receive byte and byte-data (two
//! written bytes, or one prefix byte plus one read byte under repeated START).
//! Other geometry refuses before traffic. There is no device-specific fallback.
use super::{I2cDisposition, I2cProvider, I2cTransaction};

const STATUS: u16 = 0;
const CONTROL: u16 = 2;
const COMMAND: u16 = 3;
const ADDRESS: u16 = 4;
const DATA: u16 = 5;
const BUSY: u8 = 1;
const COMPLETE: u8 = 2;
const DEVICE_ERROR: u8 = 4;
const IN_USE: u8 = 64;
const ARBITRATION: u8 = 8;
const FAILED: u8 = 16;
const CLEAR_STATUS: u8 = COMPLETE | DEVICE_ERROR | ARBITRATION | FAILED;
const KILL: u8 = 2;
const START: u8 = 64;
const BYTE: u8 = 4;
const BYTE_DATA: u8 = 8;

/// Access is restricted to the selected controller's small physical register
/// window. Native mapping/port possession belongs to its trusted owner.
pub trait I801Registers {
    fn read(&mut self, offset: u16) -> u8;
    fn write(&mut self, offset: u16, value: u8);
}

pub struct I801Controller<R> {
    registers: R,
    maximum_polls: u32,
    revoked: bool,
}

impl<R: I801Registers> I801Controller<R> {
    /// Construct only after native ownership and a finite polling budget exist.
    /// Poll count is a work bound, not a wall-clock or real-time guarantee.
    pub fn new(registers: R, maximum_polls: u32) -> Result<Self, I2cDisposition> {
        if maximum_polls == 0 {
            return Err(I2cDisposition::Refused);
        }
        Ok(Self {
            registers,
            maximum_polls,
            revoked: false,
        })
    }

    fn stop_timed_out(&mut self) -> I2cDisposition {
        self.registers.write(CONTROL, KILL);
        for _ in 0..self.maximum_polls {
            if self.registers.read(STATUS) & BUSY == 0 {
                self.registers.write(CONTROL, 0);
                self.registers.write(STATUS, CLEAR_STATUS | IN_USE);
                return I2cDisposition::TimedOut;
            }
            core::hint::spin_loop();
        }
        // Failed stop is loss, never successful cancellation/quiescence. No
        // later request can reuse this provider or pretend the bus is idle.
        self.revoked = true;
        I2cDisposition::ProviderLost
    }
}

impl<R: I801Registers> I2cProvider for I801Controller<R> {
    fn transact(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        if self.revoked {
            return Err(I2cDisposition::ProviderLost);
        }
        let (protocol, prefix, data) = match (request.write(), request.read_length()) {
            ([], 1) => (BYTE, None, None),
            ([byte], 0) => (BYTE, Some(*byte), None),
            ([prefix], 1) => (BYTE_DATA, Some(*prefix), None),
            ([prefix, data], 0) => (BYTE_DATA, Some(*prefix), Some(*data)),
            _ => return Err(I2cDisposition::Unsupported),
        };
        if input.len() != usize::from(request.read_length()) {
            return Err(I2cDisposition::Refused);
        }
        // Reading an idle IN_USE semaphore acquires it. A pre-existing owner
        // must not be cleared, killed or overwritten by this provider.
        let initial = self.registers.read(STATUS);
        if initial & IN_USE != 0 {
            return Err(I2cDisposition::Refused);
        }
        if initial & BUSY != 0 {
            self.registers.write(STATUS, IN_USE);
            return Err(I2cDisposition::Refused);
        }
        self.registers.write(STATUS, CLEAR_STATUS);
        self.registers.write(
            ADDRESS,
            (request.address() << 1) | u8::from(request.read_length() != 0),
        );
        if let Some(prefix) = prefix {
            self.registers.write(COMMAND, prefix);
        }
        if let Some(data) = data {
            self.registers.write(DATA, data);
        }
        self.registers.write(CONTROL, protocol | START);
        for _ in 0..self.maximum_polls {
            let status = self.registers.read(STATUS);
            if status & BUSY == 0 && status & CLEAR_STATUS != 0 {
                self.registers.write(STATUS, CLEAR_STATUS);
                let outcome = if status & DEVICE_ERROR != 0 {
                    // DEV_ERR conflates unclaimed cycles and a hardware device
                    // timeout. Do not invent a more precise NACK diagnosis.
                    Err(I2cDisposition::DeviceError)
                } else if status & ARBITRATION != 0 {
                    Err(I2cDisposition::ArbitrationLost)
                } else if status & FAILED != 0 {
                    self.revoked = true;
                    Err(I2cDisposition::ProviderLost)
                } else if request.read_length() == 1 {
                    input[0] = self.registers.read(DATA);
                    Ok(1)
                } else {
                    Ok(0)
                };
                self.registers.write(STATUS, IN_USE);
                return outcome;
            }
            core::hint::spin_loop();
        }
        Err(self.stop_timed_out())
    }

    fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[cfg(test)]
mod tests;
