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
