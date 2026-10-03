//! Byte-at-a-time ICH5 I2C block reads, without SMBus count-byte semantics.
//!
//! Register geometry follows the native i801 I2C block-read contract:
//! https://github.com/torvalds/linux/blob/master/drivers/i2c/busses/i2c-i801.c
//! The byte count is admitted by the request; no count is read from the device.
use super::*;

const DATA1: u16 = 6;
const BLOCK_DATA: u16 = 7;
const LAST_BYTE: u8 = 32;
const I2C_BLOCK: u8 = 24;

impl<R: I801Registers> I801Controller<R> {
    pub(super) fn read_block(
        &mut self,
        address: u8,
        prefix: u8,
        address_read_bit: bool,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        let initial = self.registers.read(STATUS);
        if initial & IN_USE != 0 {
            return Err(I2cDisposition::Refused);
        }
        if initial & BUSY != 0 {
            self.registers.write(STATUS, IN_USE);
            return Err(I2cDisposition::Refused);
        }
        self.registers.write(STATUS, CLEAR_STATUS | BYTE_DONE);
        self.registers
            .write(ADDRESS, (address << 1) | u8::from(address_read_bit));
        // I2C block reads use DATA1 for the prefix, unlike SMBus byte-data.
        self.registers.write(DATA1, prefix);
        self.registers.write(CONTROL, I2C_BLOCK | START);
        let mut count = 0;
        // One total work budget covers all byte-ready and completion waits.
        for _ in 0..self.maximum_polls {
            let status = self.registers.read(STATUS);
            if status & (DEVICE_ERROR | ARBITRATION | FAILED) != 0 {
                let error = if status & FAILED != 0 {
                    self.revoked = true;
                    I2cDisposition::ProviderLost
                } else if status & ARBITRATION != 0 {
                    I2cDisposition::ArbitrationLost
                } else {
                    I2cDisposition::DeviceError
                };
                if status & BUSY != 0 {
                    return Err(self.stop_after(error));
                }
                self.registers.write(STATUS, CLEAR_STATUS | BYTE_DONE);
                self.registers.write(STATUS, IN_USE);
                return Err(error);
            }
            if status & BYTE_DONE != 0 {
                if count == input.len() {
                    // An extra byte is a controller contract failure. Stop the
                    // bus before releasing ownership; never overrun admission.
                    self.revoked = true;
                    return Err(self.stop_after(I2cDisposition::ProviderLost));
                }
                input[count] = self.registers.read(BLOCK_DATA);
                count += 1;
                if count == input.len() - 1 {
                    self.registers.write(CONTROL, I2C_BLOCK | LAST_BYTE);
                }
                self.registers.write(STATUS, BYTE_DONE);
            }
            if status & BUSY == 0 && status & COMPLETE != 0 {
                self.registers.write(STATUS, CLEAR_STATUS | BYTE_DONE);
                self.registers.write(STATUS, IN_USE);
                // Early controller completion is an actual short read, not
                // padding or an implicit follow-up transaction.
                return Ok(count);
            }
            core::hint::spin_loop();
        }
        Err(self.stop_timed_out())
    }
}

#[cfg(test)]
mod tests;
