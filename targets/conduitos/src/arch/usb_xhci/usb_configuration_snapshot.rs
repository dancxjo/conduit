//! Root-owned standard descriptor discovery; contains no class interpretation.
use super::attachment::AttachmentBaseline;
use super::{
    ControlRequest, ControlRing, UsbDevice, UsbDma, UsbError, control, device_dma_pointer,
};
use crate::arch::xhci_mechanism::XhciReady;
use alloc::vec::Vec;

/// Frozen observation of the original attachment, never native possession.
/// Private construction prevents callers from substituting authored bytes.
pub struct UsbConfigurationSnapshot {
    controller: ControllerIdentity,
    baseline: AttachmentBaseline,
    root_port: u8,
    slot: u8,
    dma_slot: u8,
    epoch: u32,
    requested: u16,
    bytes: Vec<u8>,
}
// Structural facts validate original ownership; they cannot establish it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ControllerIdentity {
    segment: u8,
    bus: u8,
    device: u8,
    function: u8,
    vendor: u16,
    device_id: u16,
    bar_physical: u64,
}
impl ControllerIdentity {
    fn from_native(controller: &XhciReady) -> Self {
        Self {
            segment: controller.segment,
            bus: controller.bus,
            device: controller.device,
            function: controller.function,
            vendor: controller.vendor,
            device_id: controller.device_id,
            bar_physical: controller.bar_physical,
        }
    }
}
impl UsbConfigurationSnapshot {
    pub(crate) fn facts(&self) -> [u64; 11] {
        [
            u64::from(self.controller.segment),
            u64::from(self.controller.bus),
            u64::from(self.controller.device),
            u64::from(self.controller.function),
            u64::from(self.controller.vendor),
            u64::from(self.controller.device_id),
            self.controller.bar_physical,
            u64::from(self.root_port),
            u64::from(self.slot),
            u64::from(self.dma_slot),
            u64::from(self.epoch),
        ]
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn requested(&self) -> u16 {
        self.requested
    }
    pub fn validate(&self, controller: &XhciReady, device: &UsbDevice) -> Result<(), UsbError> {
        // Inspect retained facts before consulting DMA or reading any MMIO.
        self.validate_controller(ControllerIdentity::from_native(controller))?;
        if !self.matches(
            device.root_port,
            device.slot,
            device.dma_slot,
            device.attachment_epoch,
        ) {
            return Err(UsbError::StaleDeviceInstance);
        }
        device_dma_pointer(device)?;
        self.baseline
            .validate(controller.port_status(device.root_port))
    }
    fn validate_controller(&self, controller: ControllerIdentity) -> Result<(), UsbError> {
        if controller != self.controller {
            return Err(UsbError::StaleDeviceInstance);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn fixture(bytes: &[u8], requested: u16) -> Self {
        Self {
            controller: ControllerIdentity {
                segment: 0,
                bus: 0,
                device: 3,
                function: 0,
                vendor: 0x1b36,
                device_id: 0x000d,
                bar_physical: 0x1000,
            },
            baseline: AttachmentBaseline::fixture(),
            root_port: 1,
            slot: 2,
            dma_slot: 0,
            epoch: 7,
            requested,
            bytes: bytes.to_vec(),
        }
    }
    fn matches(&self, port: u8, slot: u8, dma: u8, epoch: u32) -> bool {
        epoch != 0
            && port != 0
            && slot != 0
            && self.root_port == port
            && self.slot == slot
            && self.dma_slot == dma
            && self.epoch == epoch
    }
}
/// The exclusive Root reserves exactly two additional standard discovery calls.
/// Neither discovery nor this observation grants a class implementation.
/// Root supplies the baseline established once after addressing. Discovery never
/// acknowledges connect changes, including changes during pure preflight.
/// Failed transfers retain uncertain native DMA through existing control_raw.
pub fn discover_configuration(
    controller: &mut XhciReady,
    device: &mut UsbDevice,
    mapping: fn(u64) -> Option<u64>,
    baseline: AttachmentBaseline,
) -> Result<UsbConfigurationSnapshot, UsbError> {
    baseline.validate_native(controller, device)?;
    let dma = device_dma_pointer(device)?;
    let physical = super::control_owner::transfer::mapped_dma_physical(dma as u64, mapping)?;
    let mut ring = ControlRing {
        physical: physical + core::mem::offset_of!(UsbDma, transfer_ring) as u64,
        buffer_physical: physical + core::mem::offset_of!(UsbDma, descriptor) as u64,
        root_port: device.root_port,
        slot: device.slot,
        short_packets: 0,
        dma,
    };
    let request = |length| ControlRequest {
        request_type: 0x80,
        request: 6,
        value: 0x0200,
        index: 0,
        length,
        input: true,
    };
    let actual = control(controller, &mut ring, device.slot, request(9))?;
    if actual != 9 {
        return Err(UsbError::MalformedDescriptor);
    }
    let header = unsafe { &(*dma).descriptor };
    let total = u16::from_le_bytes([header[2], header[3]]);
    if total < 9 {
        return Err(UsbError::MalformedDescriptor);
    }
    if usize::from(total) > super::MAX_CONFIGURATION_BYTES {
        return Err(UsbError::OversizedConfiguration);
    }
    let actual = control(controller, &mut ring, device.slot, request(total))?;
    let snapshot = UsbConfigurationSnapshot {
        controller: ControllerIdentity::from_native(controller),
        baseline,
        root_port: device.root_port,
        slot: device.slot,
        dma_slot: device.dma_slot,
        epoch: device.attachment_epoch,
        requested: total,
        bytes: unsafe { (&(*dma).descriptor)[..actual].to_vec() },
    };
    snapshot.validate(controller, device)?;
    device.control_transfers = device
        .control_transfers
        .checked_add(2)
        .ok_or(UsbError::TransferEnvelope)?;
    device.short_packets = device
        .short_packets
        .checked_add(ring.short_packets)
        .ok_or(UsbError::TransferEnvelope)?;
    Ok(snapshot)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_attachment_is_required_even_with_equal_descriptor_bytes() {
        let observed = UsbConfigurationSnapshot {
            controller: UsbConfigurationSnapshot::fixture(&[], 0).controller,
            baseline: AttachmentBaseline::fixture(),
            root_port: 1,
            slot: 2,
            dma_slot: 0,
            epoch: 7,
            requested: 9,
            bytes: alloc::vec![9; 9],
        };
        assert!(observed.matches(1, 2, 0, 7));
        for facts in [
            (1, 2, 0, 8),
            (2, 2, 0, 7),
            (1, 3, 0, 7),
            (1, 2, 1, 7),
            (1, 2, 0, 0),
        ] {
            assert!(!observed.matches(facts.0, facts.1, facts.2, facts.3));
        }
    }
    #[test]
    fn foreign_controller_is_rejected_without_mmio_or_dma_access() {
        let observed = UsbConfigurationSnapshot::fixture(&[9; 9], 9);
        assert_eq!(observed.validate_controller(observed.controller), Ok(()));
        let original = observed.controller;
        let mut wrong = original;
        wrong.bar_physical += 4096;
        assert_eq!(
            observed.validate_controller(wrong),
            Err(UsbError::StaleDeviceInstance)
        );
        let mut wrong = original;
        wrong.device += 1;
        assert_eq!(
            observed.validate_controller(wrong),
            Err(UsbError::StaleDeviceInstance)
        );
        let mut wrong = original;
        wrong.segment += 1;
        assert_eq!(
            observed.validate_controller(wrong),
            Err(UsbError::StaleDeviceInstance)
        );
    }
}
