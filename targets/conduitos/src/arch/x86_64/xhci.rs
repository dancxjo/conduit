//! x86 native discovery and mapping for the shared bounded xHCI mechanism.

#[path = "xhci_pci.rs"]
mod pci;

use super::super::xhci_mechanism::{ControllerLocation, initialize_mapped};
pub(crate) use super::super::xhci_mechanism::{Event, POLL_STEPS};
pub use super::super::xhci_mechanism::{XhciError, XhciReady};

pub fn initialize_xhci(
    hhdm: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<XhciReady, XhciError> {
    let controller = pci::discover()?;
    let mmio = super::mmio::map(controller.bar, hhdm, image_virtual_to_physical)
        .map_err(|_| XhciError::InvalidLayout)?;
    let location = ControllerLocation {
        segment: 0,
        bus: controller.bus,
        device: controller.device,
        function: controller.function,
        vendor: controller.vendor,
        device_id: controller.device_id,
        bar_physical: controller.bar,
    };
    // SAFETY: this is the existing sole boot-local x86 controller reservation.
    // Native device mappings and coherent x86 DMA retain the prior mechanism's
    // ordering contract; neither discovery nor mapping grants class authority.
    unsafe { initialize_mapped(location, mmio, image_virtual_to_physical) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pci_coordinates_are_exact() {
        assert_eq!(super::pci::address(2, 3, 1, 0x14), 0x8002_1914);
    }
}
