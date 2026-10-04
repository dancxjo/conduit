//! Physics-only ICH-compatible SMBus I/O realization.
//!
//! Native composition must select and authorize the exact PCI controller and
//! port resource first. This module does not scan peripherals or select devices.
use super::io::{inb, outb};
use crate::i2c_base::{
    I2cDisposition,
    i801::{I801Controller, I801Registers},
};

pub struct I801PortWindow {
    base: u16,
}

impl I801Registers for I801PortWindow {
    fn read(&mut self, offset: u16) -> u8 {
        assert!(offset <= 7, "fixed controller register window");
        unsafe { inb(self.base + offset) }
    }
    fn write(&mut self, offset: u16, value: u8) {
        assert!(offset <= 7, "fixed controller register window");
        unsafe { outb(self.base + offset, value) }
    }
}

/// Construct the physical primitive only after exact native ownership exists.
///
/// # Safety
/// `base` must name the actual admitted ICH-compatible SMBus register window,
/// with exclusive native ownership for the complete provider lifetime. The
/// trusted root must validate the PCI controller and port assignment, enable
/// the controller with I2C_EN=0 and SMI routing disabled, and preserve firmware
/// ownership restrictions. An arbitrary integer or plot value is not permission
/// to call this constructor. Subsequent effects must pass through the admitted
/// I2cHostCall, rather than exposing this primitive to ordinary protocol code.
/// Failed hardware stop quarantines the provider; its resource cannot be
/// reassigned until the native owner independently establishes quiescence.
pub unsafe fn admitted_i801_ports(
    base: u16,
    maximum_polls: u32,
) -> Result<I801Controller<I801PortWindow>, I2cDisposition> {
    if base == 0 || base & 31 != 0 || base.checked_add(31).is_none() {
        return Err(I2cDisposition::Refused);
    }
    I801Controller::new(I801PortWindow { base }, maximum_polls)
}

/// Add I2C block-read geometry only after native controller validation.
///
/// # Safety
/// All requirements of `admitted_i801_ports` apply. The owner must additionally
/// establish ICH5-or-later block-read support, auxiliary CRC and block-buffer
/// mode both disabled, and the actual PCI SPD Write Disable bit supplied here.
pub unsafe fn admitted_i801_block_read_ports(
    base: u16,
    maximum_polls: u32,
    spd_write_disabled: bool,
) -> Result<I801Controller<I801PortWindow>, I2cDisposition> {
    let controller = unsafe { admitted_i801_ports(base, maximum_polls)? };
    Ok(unsafe { controller.with_i2c_block_reads(spd_write_disabled) })
}
