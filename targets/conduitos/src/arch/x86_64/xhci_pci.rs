//! Exact legacy PCI configuration discovery for the first xHCI Base.

use super::{
    super::io::{inl, outl},
    XhciError,
};

const CONFIG_ADDRESS: u16 = 0x0cf8;
const CONFIG_DATA: u16 = 0x0cfc;
const XHCI_CLASS: u32 = 0x0c03_3000;

pub(super) struct PciController {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor: u16,
    pub device_id: u16,
    pub bar: u64,
}

pub(super) fn discover() -> Result<PciController, XhciError> {
    let mut saw_usb = false;
    for bus in 0..=u8::MAX {
        for device in 0..32 {
            for function in 0..8 {
                let id = read(bus, device, function, 0);
                if id & 0xffff == 0xffff {
                    continue;
                }
                let class = read(bus, device, function, 8) & 0xffff_ff00;
                saw_usb |= class >> 16 == 0x0c03;
                if class != XHCI_CLASS {
                    continue;
                }
                let command = read(bus, device, function, 4);
                write(bus, device, function, 4, command | 0x6);
                let low = read(bus, device, function, 0x10);
                if low & 1 != 0 || low & 0xffff_fff0 == 0 {
                    return Err(XhciError::InvalidBar);
                }
                let bar = if low & 0x6 == 0x4 {
                    u64::from(low & 0xffff_fff0)
                        | (u64::from(read(bus, device, function, 0x14)) << 32)
                } else {
                    u64::from(low & 0xffff_fff0)
                };
                return Ok(PciController {
                    bus,
                    device,
                    function,
                    vendor: id as u16,
                    device_id: (id >> 16) as u16,
                    bar,
                });
            }
        }
    }
    Err(if saw_usb {
        XhciError::WrongClass
    } else {
        XhciError::Absent
    })
}

pub(super) fn address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xfc)
}

fn read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    unsafe {
        outl(CONFIG_ADDRESS, address(bus, device, function, offset));
        inl(CONFIG_DATA)
    }
}

fn write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    unsafe {
        outl(CONFIG_ADDRESS, address(bus, device, function, offset));
        outl(CONFIG_DATA, value);
    }
}

/// Read-only capability observation. Presence is not IRQ route qualification.
/// The conventional PCI list has at most48 aligned entries in0x40..0xfc.
#[cfg(any(feature = "native-latency-observation", target_os = "none"))]
pub(super) fn observe_msix(controller: &PciController) -> Option<(u8, u16, u8, u32)> {
    msix_capability(|offset| {
        read(
            controller.bus,
            controller.device,
            controller.function,
            offset,
        )
    })
}

#[cfg(any(test, feature = "native-latency-observation", target_os = "none"))]
fn msix_capability(mut read: impl FnMut(u8) -> u32) -> Option<(u8, u16, u8, u32)> {
    if read(4) & (1 << 20) == 0 {
        return None;
    }
    let mut offset = (read(0x34) & 0xff) as u8;
    let mut visited = 0_u64;
    for _ in 0..48 {
        if offset < 0x40 || offset & 3 != 0 {
            return None;
        }
        let bit = 1_u64 << (offset / 4);
        if visited & bit != 0 {
            return None;
        }
        visited |= bit;
        let header = read(offset);
        if header & 0xff == 0x11 {
            // MSI-X's12-byte capability must fit conventional config space.
            let table_offset = offset.checked_add(4)?;
            offset.checked_add(8)?;
            let table = read(table_offset);
            return Some((
                offset,
                ((header >> 16) as u16 & 0x7ff) + 1,
                (table & 7) as u8,
                table & !7,
            ));
        }
        offset = ((header >> 8) & 0xff) as u8;
        if offset == 0 {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod capability_tests {
    use super::msix_capability;
    fn config() -> [u32; 64] {
        let mut words = [0; 64];
        words[1] = 1 << 20;
        words[0x34 / 4] = 0x40;
        words
    }
    #[test]
    fn capability_observation_retains_table_identity_without_enabling_it() {
        let mut words = config();
        words[0x40 / 4] = 0x9005;
        words[0x90 / 4] = (15 << 16) | 0x11;
        words[0x94 / 4] = 0x3000;
        assert_eq!(
            msix_capability(|offset| words[offset as usize / 4]),
            Some((0x90, 16, 0, 0x3000))
        );
    }
    #[test]
    fn malformed_or_cyclic_lists_terminate_without_guessing_a_route() {
        for pointer in [0, 0x3c, 0x41, 0xfc] {
            let mut words = config();
            words[0x34 / 4] = pointer;
            words[0xfc / 4] = 0x11;
            assert_eq!(msix_capability(|offset| words[offset as usize / 4]), None);
        }
        let mut words = config();
        words[0x40 / 4] = 0x4405;
        words[0x44 / 4] = 0x4005;
        let mut reads = 0;
        assert_eq!(
            msix_capability(|offset| {
                reads += 1;
                words[offset as usize / 4]
            }),
            None
        );
        assert_eq!(reads, 4);
    }
    #[test]
    fn status_and_complete_capability_extent_are_required() {
        let mut words = config();
        words[0x40 / 4] = 0x11;
        words[1] = 0;
        assert_eq!(msix_capability(|offset| words[offset as usize / 4]), None);
        words[1] = 1 << 20;
        words[0x34 / 4] = 0xf4;
        words[0xf4 / 4] = 0x11;
        assert_eq!(
            msix_capability(|offset| words[offset as usize / 4]),
            Some((0xf4, 1, 0, 0))
        );
    }
}

#[cfg(target_os = "none")]
pub(super) fn set_msix_control(controller: &PciController, offset: u8, control: u16) {
    let header = read(
        controller.bus,
        controller.device,
        controller.function,
        offset,
    );
    write(
        controller.bus,
        controller.device,
        controller.function,
        offset,
        (header & 0xffff) | (u32::from(control) << 16),
    );
}

#[cfg(target_os = "none")]
pub(super) fn msix_control(controller: &PciController, offset: u8) -> u16 {
    (read(
        controller.bus,
        controller.device,
        controller.function,
        offset,
    ) >> 16) as u16
}
