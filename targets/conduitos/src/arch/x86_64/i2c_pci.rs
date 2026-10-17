//! Read-only validation of an explicitly selected SMBus controller.
//! Observation does not establish firmware handoff or exclusive ownership.

use super::io::{inl, outl};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct I801PciObservation {
    pub device_id: u16,
    pub port_base: u16,
    pub host_enabled: bool,
    pub smi_enabled: bool,
    pub i2c_mode: bool,
    pub spd_write_disabled: bool,
    pub io_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum I801PciRefusal {
    InvalidSelection,
    Absent,
    UnsupportedController,
    InvalidPortWindow,
}

/// Observe a root-selected PCI function without enabling it or scanning devices.
///
/// # Safety
/// The native root must own serialized access to legacy PCI configuration ports.
/// The returned facts grant no authority to access the observed SMBus window.
pub unsafe fn observe_i801_pci(
    bus: u8,
    device: u8,
    function: u8,
) -> Result<I801PciObservation, I801PciRefusal> {
    if device >= 32 || function >= 8 {
        return Err(I801PciRefusal::InvalidSelection);
    }
    let read = |offset: u8| unsafe {
        let address = 0x8000_0000
            | (u32::from(bus) << 16)
            | (u32::from(device) << 11)
            | (u32::from(function) << 8)
            | u32::from(offset);
        outl(0xcf8, address);
        inl(0xcfc)
    };
    decode(read(0), read(8), read(4), read(0x20), read(0x40))
}

fn decode(
    identity: u32,
    class: u32,
    command: u32,
    bar: u32,
    configuration: u32,
) -> Result<I801PciObservation, I801PciRefusal> {
    if identity as u16 == 0xffff {
        return Err(I801PciRefusal::Absent);
    }
    let device_id = (identity >> 16) as u16;
    // Reviewed ICH5–ICH9 controller geometry; other generations remain refused.
    if identity as u16 != 0x8086
        || !matches!(device_id, 0x24d3 | 0x266a | 0x27da | 0x283e | 0x2930)
        || class & 0xffff_ff00 != 0x0c05_0000
    {
        return Err(I801PciRefusal::UnsupportedController);
    }
    let base = bar & !31;
    if bar & 1 == 0 || base == 0 || base > u32::from(u16::MAX - 31) {
        return Err(I801PciRefusal::InvalidPortWindow);
    }
    Ok(I801PciObservation {
        device_id,
        port_base: base as u16,
        host_enabled: configuration & 1 != 0,
        smi_enabled: configuration & 2 != 0,
        i2c_mode: configuration & 4 != 0,
        spd_write_disabled: configuration & 16 != 0,
        io_enabled: command & 1 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_selection_never_accesses_configuration_ports() {
        // These calls reject before the privileged I/O boundary.
        for (device, function) in [(32, 0), (0, 8), (u8::MAX, u8::MAX)] {
            assert_eq!(
                unsafe { observe_i801_pci(0, device, function) },
                Err(I801PciRefusal::InvalidSelection)
            );
        }
    }

    #[test]
    fn retains_controller_configuration_without_granting_ownership() {
        let observed = decode(0x24d3_8086, 0x0c05_0001, 1, 0x501, 0x17).unwrap();
        assert_eq!(observed.port_base, 0x500);
        assert!(observed.host_enabled && observed.smi_enabled && observed.i2c_mode);
        assert!(observed.spd_write_disabled && observed.io_enabled);
    }

    #[test]
    fn refuses_missing_unknown_and_invalid_windows() {
        assert_eq!(decode(u32::MAX, 0, 0, 0, 0), Err(I801PciRefusal::Absent));
        assert_eq!(
            decode(0x1234_8086, 0x0c05_0000, 1, 0x501, 1),
            Err(I801PciRefusal::UnsupportedController)
        );
        for bar in [0, 1, 0x500, 0x1_0501] {
            assert_eq!(
                decode(0x24d3_8086, 0x0c05_0000, 1, bar, 1),
                Err(I801PciRefusal::InvalidPortWindow)
            );
        }
    }
}
