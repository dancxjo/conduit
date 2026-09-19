//! Exact PCI discovery for the selected transitional VirtIO-net device.

use super::io::{inl, outl};

use super::virtio_net::VirtioNetError;

const CONFIG_ADDRESS: u16 = 0x0cf8;
const CONFIG_DATA: u16 = 0x0cfc;
const VIRTIO_VENDOR: u16 = 0x1af4;
const TRANSITIONAL_NETWORK_DEVICE: u16 = 0x1000;
const ETHERNET_CLASS: u32 = 0x0200_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PciVirtioNet {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub io_base: u16,
}

pub(super) fn discover() -> Result<PciVirtioNet, VirtioNetError> {
    let mut saw_network = false;
    for bus in 0..=u8::MAX {
        for device in 0..32 {
            for function in 0..8 {
                let id = read(bus, device, function, 0);
                if id & 0xffff == 0xffff {
                    continue;
                }
                let class = read(bus, device, function, 8) & 0xffff_ff00;
                saw_network |= class == ETHERNET_CLASS;
                if id as u16 != VIRTIO_VENDOR
                    || (id >> 16) as u16 != TRANSITIONAL_NETWORK_DEVICE
                    || class != ETHERNET_CLASS
                {
                    continue;
                }
                let bar = read(bus, device, function, 0x10);
                if bar & 1 == 0 || bar & 0xffff_fffc == 0 {
                    return Err(VirtioNetError::InvalidBar);
                }
                let io_base =
                    u16::try_from(bar & 0xffff_fffc).map_err(|_| VirtioNetError::InvalidBar)?;
                let command = read(bus, device, function, 4);
                write(bus, device, function, 4, command | 0x5);
                return Ok(PciVirtioNet {
                    bus,
                    device,
                    function,
                    io_base,
                });
            }
        }
    }
    Err(if saw_network {
        VirtioNetError::WrongDevice
    } else {
        VirtioNetError::Absent
    })
}

fn address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_addresses_preserve_exact_bdf_and_word_alignment() {
        assert_eq!(address(0, 3, 0, 0x10), 0x8000_1810);
        assert_eq!(address(0xab, 0x1f, 7, 0x13), 0x80ab_ff10);
    }
}
