//! Intel ICH-compatible SMBus physics leaf, with no peripheral protocol knowledge.
//!
//! The supported transaction subset is send/receive byte and byte-data (two
//! written bytes, or one prefix byte plus one read byte under repeated START).
//! An explicitly validated ICH5-or-later profile also permits bounded I2C block
//! reads. Other geometry refuses before traffic. No device fallback is implicit.
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
const BYTE_DONE: u8 = 128;
mod block_read;

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
    block_read_address_bit: Option<bool>,
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
            block_read_address_bit: None,
        })
    }

    /// Enable only the native controller's independently established geometry.
    ///
    /// # Safety
    /// The owner must have validated ICH5-or-later I2C block-read support,
    /// disabled auxiliary CRC and 32-byte buffer mode, and retained exclusive
    /// register ownership. `spd_write_disabled` must match the actual retained
    /// PCI host configuration; it determines the controller's read address bit.
    /// Neither a peripheral probe nor a request establishes these facts.
    pub unsafe fn with_i2c_block_reads(mut self, spd_write_disabled: bool) -> Self {
        self.block_read_address_bit = Some(spd_write_disabled);
        self
    }

    fn stop_timed_out(&mut self) -> I2cDisposition {
        self.stop_after(I2cDisposition::TimedOut)
    }

    fn stop_after(&mut self, disposition: I2cDisposition) -> I2cDisposition {
        self.registers.write(CONTROL, KILL);
        for _ in 0..self.maximum_polls {
            if self.registers.read(STATUS) & BUSY == 0 {
                self.registers.write(CONTROL, 0);
                self.registers
                    .write(STATUS, CLEAR_STATUS | IN_USE | BYTE_DONE);
                return disposition;
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
        if let ([prefix], 2..=32) = (request.write(), request.read_length()) {
            let Some(address_read_bit) = self.block_read_address_bit else {
                return Err(I2cDisposition::Unsupported);
            };
            if input.len() != usize::from(request.read_length()) {
                return Err(I2cDisposition::Refused);
            }
            return self.read_block(request.address(), *prefix, address_read_bit, input);
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
