//! Finite root-bus ECAM discovery for pinned QEMU virt's low PCI window.
//! No USB attachment, descriptor, or class policy belongs here.
use crate::arch::xhci_mechanism::ControllerLocation;

pub(super) const ECAM: usize = 0x3f00_0000;
const WINDOW_START: u64 = 0x1000_0000;
const WINDOW_END: u64 = 0x3eff_0000;
const BLOCK_BYTES: u64 = 1 << 21;
const XHCI_CLASS: u32 = 0x0c03_3000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciRefusal {
    Absent,
    WrongController,
    MultipleControllers,
    RootTopology,
    InvalidBar,
    BarExtent,
    AllocationConflict,
    ConfigurationRejected,
}
impl PciRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "aarch64-virt-xhci-absent",
            Self::WrongController => "aarch64-virt-xhci-controller-unsupported",
            Self::MultipleControllers => "aarch64-virt-xhci-multiple-controllers",
            Self::RootTopology => "aarch64-virt-xhci-root-topology-unsupported",
            Self::InvalidBar => "aarch64-virt-xhci-bar-invalid",
            Self::BarExtent => "aarch64-virt-xhci-bar-extent-unsupported",
            Self::AllocationConflict => "aarch64-virt-xhci-bar-allocation-conflict",
            Self::ConfigurationRejected => "aarch64-virt-xhci-configuration-rejected",
        }
    }
}

pub(super) fn address(device: u8, function: u8, offset: u16) -> Option<usize> {
    if device >= 32 || function >= 8 || offset >= 4096 || offset & 3 != 0 {
        return None;
    }
    Some(ECAM + ((device as usize) << 15) + ((function as usize) << 12) + offset as usize)
}

/// The caller exclusively owns the pinned root bus and its configuration.
/// Probe writes are restored before geometry checks; no class effects occur.
pub(super) fn discover(
    mut read: impl FnMut(usize) -> u32,
    mut write: impl FnMut(usize, u32),
) -> Result<ControllerLocation, PciRefusal> {
    let mut selected = None;
    let mut endpoints = 0u16;
    let mut saw_usb = false;
    for device in 0..32 {
        for function in 0..8 {
            let base = address(device, function, 0).expect("bounded ECAM coordinates");
            let id = read(base);
            if id & 0xffff == 0xffff {
                continue;
            }
            let class = read(base + 8) & 0xffff_ff00;
            // This finite profile has the one host bridge and direct root-bus
            // endpoints. A downstream bridge could own an unseen BAR window.
            if class >> 24 == 6 && class != 0x0600_0000 {
                return Err(PciRefusal::RootTopology);
            }
            endpoints += u16::from(class >> 24 != 6);
            saw_usb |= class >> 16 == 0x0c03;
            if class != XHCI_CLASS {
                continue;
            }
            if id != 0x000d_1b36 {
                return Err(PciRefusal::WrongController);
            }
            if selected.replace((device, function, base, id)).is_some() {
                return Err(PciRefusal::MultipleControllers);
            }
        }
    }
    let (device, function, base, id) = selected.ok_or(if saw_usb {
        PciRefusal::WrongController
    } else {
        PciRefusal::Absent
    })?;
    let command = read(base + 4) & 0xffff;
    let low = read(base + 0x10);
    let bar_type = low & 0xf;
    if bar_type != 0 && bar_type != 4 {
        return Err(PciRefusal::InvalidBar);
    }
    let wide = bar_type == 4;
    let high = if wide { read(base + 0x14) } else { 0 };
    // Command DWORD writes always carry zero status bits: do not clear W1C
    // status observed beside the command register during a read-modify-write.
    write(base + 4, command & !6);
    write(base + 0x10, u32::MAX);
    if wide {
        write(base + 0x14, u32::MAX);
    }
    let mask_low = read(base + 0x10) & !0xf;
    let mask_high = if wide { read(base + 0x14) } else { u32::MAX };
    if wide {
        write(base + 0x14, high);
    }
    write(base + 0x10, low);
    write(base + 4, command);
    let mask = u64::from(mask_low) | (u64::from(mask_high) << 32);
    let extent = (!mask).wrapping_add(1);
    if !extent.is_power_of_two() || !(4096..=BLOCK_BYTES).contains(&extent) {
        return Err(PciRefusal::BarExtent);
    }
    let original_bar = u64::from(low & !0xf) | (u64::from(high) << 32);
    let bar = if original_bar == 0 {
        // A finite single-endpoint fixture may reserve the dedicated first
        // low-window block. Never guess an address amid other endpoints.
        if endpoints != 1 {
            return Err(PciRefusal::AllocationConflict);
        }
        WINDOW_START
    } else {
        original_bar
    };
    validate_bar(bar, extent)?;
    if original_bar == 0 {
        write(base + 4, command & !6);
        if wide {
            write(base + 0x14, 0);
        }
        write(base + 0x10, bar as u32 | bar_type);
    }
    write(base + 4, command | 6);
    if read(base + 4) & 6 != 6
        || read(base + 0x10) & !0xf != bar as u32
        || (wide && read(base + 0x14) != (bar >> 32) as u32)
    {
        write(base + 4, command & !6);
        if wide {
            write(base + 0x14, high);
        }
        write(base + 0x10, low);
        write(base + 4, command);
        return Err(PciRefusal::ConfigurationRejected);
    }
    Ok(ControllerLocation {
        segment: 0,
        bus: 0,
        device,
        function,
        vendor: id as u16,
        device_id: (id >> 16) as u16,
        bar_physical: bar,
    })
}

fn validate_bar(bar: u64, extent: u64) -> Result<(), PciRefusal> {
    let last = bar.checked_add(extent - 1).ok_or(PciRefusal::InvalidBar)?;
    if bar < WINDOW_START || last >= WINDOW_END || bar & (extent - 1) != 0 {
        return Err(PciRefusal::InvalidBar);
    }
    if bar / BLOCK_BYTES != last / BLOCK_BYTES {
        return Err(PciRefusal::BarExtent);
    }
    Ok(())
}

#[cfg(test)]
#[path = "pci_tests.rs"]
mod tests;
