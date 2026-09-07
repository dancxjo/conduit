//! Bounded root-port attachment, reset, and removal transitions.

use core::hint::spin_loop;

use super::{PORT_POLL_STEPS, UsbDevice, UsbError};
use crate::arch::x86_64::xhci::XhciReady;

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
    if controller.port_status(device.root_port) & 1 != 0 {
        return Err(UsbError::StaleDeviceInstance);
    }
    controller
        .disable_removed_slot(device.slot)
        .map_err(UsbError::from)
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
