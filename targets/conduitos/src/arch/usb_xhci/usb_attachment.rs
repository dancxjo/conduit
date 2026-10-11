//! Bounded root-port attachment, reset, and removal transitions.

use core::hint::spin_loop;

use super::{PORT_POLL_STEPS, UsbDevice, UsbError};
use crate::arch::xhci_mechanism::XhciReady;

pub fn wait_for_attachment_state(
    controller: &XhciReady,
    root_port: u8,
    attached: bool,
) -> Result<(), UsbError> {
    for _ in 0..PORT_POLL_STEPS {
        if (controller.port_status(root_port) & 1 != 0) == attached {
            return Ok(());
        }
        spin_loop();
    }
    Err(if attached {
        UsbError::NoDevice
    } else {
        UsbError::DeviceVanished
    })
}

pub fn retire_removed_device(
    controller: &mut XhciReady,
    device: &UsbDevice,
) -> Result<u8, UsbError> {
    let dma = super::dma::device_dma_pointer(device)?;
    if controller.port_status(device.root_port) & 1 != 0 {
        return Err(UsbError::StaleDeviceInstance);
    }
    let stale_transfers = controller
        .disable_removed_slot(device.slot)
        .map_err(UsbError::from)?;
    // Successful Disable Slot completion acknowledges all endpoint DMA stop.
    // Only then can later enumeration initialize this fixed storage anew.
    unsafe {
        (*dma).owner_slot = 0;
    }
    Ok(stale_transfers)
}

pub(super) fn attached_root_port(controller: &XhciReady) -> Result<u8, UsbError> {
    let mut found = 0;
    for port in 1..=controller.maximum_ports() {
        if controller.port_status(port) & 1 != 0 {
            if found != 0 {
                return Err(UsbError::MultipleDevices);
            }
            found = port;
        }
    }
    if found == 0 {
        Err(UsbError::NoDevice)
    } else {
        Ok(found)
    }
}

pub(super) fn reset_port(controller: &XhciReady, port: u8) -> Result<(), UsbError> {
    controller.write_port_status(port, controller.port_status(port) | (1 << 4));
    for _ in 0..PORT_POLL_STEPS {
        let status = controller.port_status(port);
        if let Some(result) = classify_port_reset(status) {
            return result;
        }
        spin_loop();
    }
    Err(UsbError::PortResetTimeout)
}

pub(super) fn classify_port_reset(status: u32) -> Option<Result<(), UsbError>> {
    if status & 1 == 0 {
        Some(Err(UsbError::DeviceVanished))
    } else if status & (1 << 4) == 0 {
        Some(if status & 2 != 0 {
            Ok(())
        } else {
            Err(UsbError::PortResetFailed)
        })
    } else {
        None
    }
}

// xHCI 1.2 section 5.4.8: CSC is sticky RW1C. Preserve only ordinary
// RW fields; copying PORTSC wholesale would acknowledge unrelated changes,
// disable the port through PED, or trigger reset/link-state operations.
const CONNECT_STATUS_CHANGE: u32 = 1 << 17;
const PORT_ORDINARY_RW: u32 = (1 << 9) | (3 << 14) | (7 << 25);

/// A baseline established by the exclusive Root after addressing its device.
/// This token records continuity checking, never device possession.
pub struct AttachmentBaseline;
impl AttachmentBaseline {
    pub(crate) fn establish(controller: &XhciReady, port: u8) -> Result<Self, UsbError> {
        let status = controller.port_status(port);
        if status & 1 == 0 {
            return Err(UsbError::DeviceVanished);
        }
        controller.write_port_status(port, baseline_acknowledgement(status));
        validate_attachment_continuity(controller.port_status(port))?;
        Ok(Self)
    }
    pub(crate) fn validate(&self, status: u32) -> Result<(), UsbError> {
        validate_attachment_continuity(status)
    }
    #[cfg(test)]
    pub(super) fn fixture() -> Self {
        Self
    }
}
fn baseline_acknowledgement(status: u32) -> u32 {
    (status & PORT_ORDINARY_RW) | CONNECT_STATUS_CHANGE
}
pub(in crate::arch::usb_xhci) fn validate_attachment_continuity(
    status: u32,
) -> Result<(), UsbError> {
    if status & 1 == 0 {
        Err(UsbError::DeviceVanished)
    } else if status & CONNECT_STATUS_CHANGE != 0 {
        Err(UsbError::StaleDeviceInstance)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod continuity_tests {
    use super::*;
    #[test]
    fn connected_baseline_requires_clear_connect_change() {
        assert_eq!(validate_attachment_continuity(1 | 2 | (1 << 9)), Ok(()));
        assert_eq!(
            validate_attachment_continuity(0),
            Err(UsbError::DeviceVanished)
        );
        assert_eq!(
            validate_attachment_continuity(1 | CONNECT_STATUS_CHANGE),
            Err(UsbError::StaleDeviceInstance)
        );
    }
    #[test]
    fn repeated_disconnect_cannot_restore_the_baseline_through_current_connected_status() {
        let baseline = AttachmentBaseline::fixture();
        assert_eq!(baseline.validate(1), Ok(()));
        for status in [
            CONNECT_STATUS_CHANGE,
            1 | CONNECT_STATUS_CHANGE,
            CONNECT_STATUS_CHANGE,
            1 | CONNECT_STATUS_CHANGE,
        ] {
            assert!(baseline.validate(status).is_err());
        }
        // Validation is read-only; only the initial Root boundary acknowledges CSC.
        assert_eq!(baseline_acknowledgement(1), CONNECT_STATUS_CHANGE);
    }
    #[test]
    fn acknowledgement_cannot_copy_commands_or_other_change_bits() {
        assert_eq!(
            baseline_acknowledgement(u32::MAX),
            PORT_ORDINARY_RW | CONNECT_STATUS_CHANGE
        );
        assert_eq!(
            baseline_acknowledgement(1 | 2 | (1 << 4) | (1 << 21)),
            CONNECT_STATUS_CHANGE
        );
    }
}
