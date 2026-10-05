//! Physics-only ICH-compatible SMBus I/O realization.
//!
//! Native composition must select and authorize the exact PCI controller and
//! port resource first. This module does not scan peripherals or select devices.
use super::i2c_pci::observe_i801_pci;
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

/// Realize an exclusively owned, already configured PCI controller.
///
/// # Safety
/// The root must hold exclusive ownership of the selected function and its
/// port window, including firmware handoff and serialized PCI configuration
/// access. Ownership must outlive the returned provider. Configuration and
/// attachment must remain stable during its lifetime. This function validates
/// hardware facts; it does not establish ownership or electrical permission.
pub unsafe fn admitted_i801_pci_ports(
    bus: u8,
    device: u8,
    function: u8,
    maximum_polls: u32,
) -> Result<I801Controller<I801PortWindow>, I2cDisposition> {
    if maximum_polls == 0 {
        return Err(I2cDisposition::Refused);
    }
    let observed =
        unsafe { observe_i801_pci(bus, device, function) }.map_err(|_| I2cDisposition::Refused)?;
    if !observed.host_enabled || !observed.io_enabled || observed.smi_enabled || observed.i2c_mode {
        return Err(I2cDisposition::Refused);
    }
    // Refuse active hardware and unsupported auxiliary modes without changing
    // firmware state or aborting a transaction owned by another actor.
    let status = unsafe { inb(observed.port_base) };
    let auxiliary = unsafe { inb(observed.port_base + 13) };
    if status & 1 != 0 || auxiliary & 3 != 0 {
        return Err(I2cDisposition::Refused);
    }
    unsafe {
        admitted_i801_block_read_ports(
            observed.port_base,
            maximum_polls,
            observed.spd_write_disabled,
        )
    }
}
