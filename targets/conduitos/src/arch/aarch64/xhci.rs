//! Native leaves for QEMU virt-10.2, highmem=off, iommu=none, root PCI bus.
//! This provider supplies device mechanisms only. Product USB/class admission
//! remains disabled until its ordinary Root and execution proofs are installed.
mod mapping;
mod pci;

use crate::arch::xhci_mechanism::{XhciError, XhciReady, initialize_mapped};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtXhciRefusal {
    Pci(pci::PciRefusal),
    Mapping(mapping::MappingRefusal),
    Controller(XhciError),
}
impl VirtXhciRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pci(error) => error.as_str(),
            Self::Mapping(error) => error.as_str(),
            Self::Controller(error) => error.as_str(),
        }
    }
}

/// Prepare the one native controller under the pinned emulator machine contract.
///
/// # Safety
/// The sole Root must independently establish QEMU virt-10.2 with highmem=off,
/// iommu=none and released firmware ownership. The low Root MMIO map must be
/// installed and the actual image's coherent RAM mapping must remain retained.
/// No other PCI/MMIO/DMA consumer may use the selected controller or newly
/// reserved BAR. These preconditions do not follow from device observation.
pub unsafe fn initialize_xhci(
    _hhdm: u64,
    image_mapping: fn(u64) -> Option<u64>,
) -> Result<XhciReady, VirtXhciRefusal> {
    mapping::map(pci::ECAM as u64, image_mapping).map_err(VirtXhciRefusal::Mapping)?;
    let location = pci::discover(
        |address| unsafe { (address as *const u32).read_volatile() },
        |address, value| unsafe {
            core::arch::asm!("dsb sy", options(nostack));
            (address as *mut u32).write_volatile(value);
            core::arch::asm!("dsb sy", options(nostack));
        },
    )
    .map_err(VirtXhciRefusal::Pci)?;
    let mmio =
        mapping::map(location.bar_physical, image_mapping).map_err(VirtXhciRefusal::Mapping)?;
    // SAFETY: the caller establishes the pinned coherent QEMU DMA contract;
    // native mappings above establish Device-nGnRnE registers. Shared controller
    // publication/acquisition uses AArch64 full-system barriers, not x86 rules.
    unsafe { initialize_mapped(location, mmio, image_mapping) }.map_err(VirtXhciRefusal::Controller)
}
