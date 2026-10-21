//! x86 native discovery and mapping for the shared bounded xHCI mechanism.

#[path = "xhci_irq.rs"]
mod irq;
#[path = "xhci_pci.rs"]
mod pci;
pub(super) use irq::ReceiveBudget;
pub(crate) use irq::idle_after_poll;
#[cfg(feature = "native-latency-observation")]
pub(super) use irq::observation;

use super::super::xhci_mechanism::{ControllerLocation, initialize_mapped};
pub(crate) use super::super::xhci_mechanism::{Event, POLL_STEPS};
pub use super::super::xhci_mechanism::{XhciError, XhciReady};

pub fn initialize_xhci(
    hhdm: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<XhciReady, XhciError> {
    let controller = pci::discover()?;
    #[cfg(feature = "native-latency-observation")]
    observe_interrupt_capability(&controller);
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
    let ready = unsafe { initialize_mapped(location, mmio, image_virtual_to_physical)? };
    #[cfg(target_os = "none")]
    irq::configure(&controller, mmio, &ready);
    Ok(ready)
}

#[cfg(test)]
mod tests {
    #[test]
    fn pci_coordinates_are_exact() {
        assert_eq!(super::pci::address(2, 3, 1, 0x14), 0x8002_1914);
    }
}

#[cfg(feature = "native-latency-observation")]
fn observe_interrupt_capability(controller: &pci::PciController) {
    use core::fmt::Write;
    let mut record = crate::sign_format::FixedText::new();
    let x2apic_advertised = core::arch::x86_64::__cpuid(1).ecx & (1 << 21) != 0;
    if let Some((offset, entries, bir, table_offset)) = pci::observe_msix(controller) {
        let _ = writeln!(
            record,
            "CONDUIT_XHCI_IRQ_OBSERVATION {{\"schema\":\"conduit.conduitos/xhci-irq-observation@1\",\"route\":\"bounded-poll\",\"delivery_qualified\":false,\"bus\":{},\"device\":{},\"function\":{},\"msix_capability\":{},\"msix_entries\":{},\"table_bir\":{},\"table_offset\":{},\"x2apic_advertised\":{}}}",
            controller.bus,
            controller.device,
            controller.function,
            offset,
            entries,
            bir,
            table_offset,
            x2apic_advertised
        );
    } else {
        let _ = writeln!(
            record,
            "CONDUIT_XHCI_IRQ_OBSERVATION {{\"schema\":\"conduit.conduitos/xhci-irq-observation@1\",\"route\":\"bounded-poll\",\"delivery_qualified\":false,\"msix_capability\":null,\"x2apic_advertised\":{}}}",
            x2apic_advertised
        );
    }
    let _ = super::serial::append_boot_diagnostic(record.as_bytes());
}
