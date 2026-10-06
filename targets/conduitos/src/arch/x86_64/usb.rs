//! A bounded root-attached USB device set realized through the xHCI Base.
//!
//! This module retains structural USB truth only. It neither parses HID reports
//! nor advertises a semantic input capability.

use core::ptr::{read_volatile, write_volatile};

use super::xhci::XhciReady;

#[path = "usb_control.rs"]
mod control;
use control::{ControlRequest, ControlRing, control};

#[path = "usb_attachment.rs"]
mod attachment;
#[path = "usb_descriptor.rs"]
pub(super) mod descriptor;
#[path = "usb_dma.rs"]
pub(in crate::arch::x86_64) mod dma;
#[path = "usb_error.rs"]
mod error;
#[path = "usb_transfer.rs"]
mod transfer;

#[cfg(feature = "scripted-keyboard-proof")]
#[path = "usb_ring_proof.rs"]
mod ring_proof;

#[cfg(test)]
use attachment::classify_port_reset;
use attachment::{attached_root_port, reset_port};
pub use attachment::{retire_removed_device, wait_for_attachment_state};
pub use descriptor::UsbDevice;
use descriptor::{
    MAX_CONFIGURATION_BYTES, device_from_descriptor, parse_configuration, validate_header,
};
pub use dma::USB_DEVICE_DMA_SLOTS;
use dma::{UsbDma, UsbDmaSlot, device_dma_pointer, dma_pointer};
pub use error::UsbError;
#[path = "usb_control_owner.rs"]
mod control_owner;
#[path = "usb_endpoint_read.rs"]
pub(super) mod endpoint_read;
#[cfg(any(test, feature = "scripted-keyboard-proof"))]
#[path = "usb_endpoint_read_proof.rs"]
mod endpoint_read_proof;
#[cfg(feature = "usb-endpoint-read-proof")]
pub use endpoint_read_proof::run_appliance as run_usb_endpoint_read_proof;
#[path = "usb_endpoint_offer.rs"]
mod endpoint_offer;
#[path = "usb_endpoint_setup.rs"]
mod endpoint_setup;
pub use control_owner::{UsbControlCallRefusal, UsbControlHostCall, UsbControlSelection};
#[cfg(feature = "scripted-keyboard-proof")]
#[path = "usb_control_kernel_proof.rs"]
mod kernel_proof;
#[cfg(feature = "scripted-keyboard-proof")]
pub use kernel_proof::run as run_usb_control_kernel_proof;
#[cfg(feature = "scripted-keyboard-proof")]
#[path = "usb_device_probe_proof.rs"]
mod device_probe_proof;
#[cfg(feature = "scripted-keyboard-proof")]
pub use device_probe_proof::{
    run as run_usb_device_probe_proof, run_configuration as run_usb_configuration_probe_proof,
};
#[cfg(test)]
use transfer::{setup_transfer_type, transferred_bytes, validate_transfer_event};
pub const MAX_CONTROL_TRANSFERS: u8 = 5;
pub const MAX_OUTSTANDING_CONTROL_TRANSFERS: u8 = 1;
pub const MAX_ENUMERATION_RETRIES: u8 = 0;
pub const USB_SIGN_SLOTS: u8 = 12;
const TRANSFER_TRBS: usize = 32;
const PORT_POLL_STEPS: u32 = 2_000_000;

pub fn enumerate_one(
    controller: &mut XhciReady,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<UsbDevice, UsbError> {
    enumerate_one_at_epoch(controller, image_virtual_to_physical, 1)
}

pub fn enumerate_one_at_epoch(
    controller: &mut XhciReady,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
    attachment_epoch: u32,
) -> Result<UsbDevice, UsbError> {
    let root_port = attached_root_port(controller)?;
    enumerate_root_port_at_epoch(
        controller,
        image_virtual_to_physical,
        root_port,
        UsbDmaSlot::PRIMARY,
        attachment_epoch,
    )
}

fn enumerate_root_port_at_epoch(
    controller: &mut XhciReady,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
    root_port: u8,
    dma_slot: UsbDmaSlot,
    attachment_epoch: u32,
) -> Result<UsbDevice, UsbError> {
    if attachment_epoch == 0 {
        return Err(UsbError::StaleDeviceInstance);
    }
    if root_port == 0 || root_port > controller.maximum_ports() {
        return Err(UsbError::RootPortInvalid);
    }
    if controller.port_status(root_port) & 1 == 0 {
        return Err(UsbError::NoDevice);
    }
    let dma = dma_pointer(dma_slot);
    if unsafe { (*dma).owner_slot != 0 } {
        return Err(UsbError::StaleDeviceInstance);
    }
    let dma_virtual = dma as u64;
    let dma_physical = image_virtual_to_physical(dma_virtual).ok_or(UsbError::DmaAddressInvalid)?;
    if dma_physical & 0xfff != 0 {
        return Err(UsbError::DmaAddressInvalid);
    }
    if !matches!(controller.context_bytes(), 32 | 64) {
        return Err(UsbError::ContextGeometry);
    }
    unsafe {
        *dma = UsbDma {
            device_context: [0; 2048],
            input_context: [0; 2112],
            transfer_ring: [[0; 4]; TRANSFER_TRBS],
            descriptor: [0; MAX_CONFIGURATION_BYTES],
            control_cursor: crate::usb_base::control_ring::ControlRingCursor::new(),
            owner_slot: 0,
            owner_root_port: root_port,
            owner_epoch: attachment_epoch,
        }
    };
    reset_port(controller, root_port)?;
    let speed = ((controller.port_status(root_port) >> 10) & 0xf) as u8;
    let initial_packet = match speed {
        1 | 2 => 8,
        3 => 64,
        4 | 5 => 512,
        _ => return Err(UsbError::UnsupportedTopology),
    };
    let enable = controller
        .command([0, 0, 0, 9 << 10])
        .map_err(|_| UsbError::EnableSlotFailed)?;
    if enable.completion_code != 1 || enable.slot == 0 {
        return Err(UsbError::EnableSlotFailed);
    }
    let slot = enable.slot;
    // From here, storage remains owned until this exact slot is disabled.
    unsafe {
        (*dma).owner_slot = slot;
    }
    let context = controller.context_bytes();
    let device_phys = dma_physical + core::mem::offset_of!(UsbDma, device_context) as u64;
    let input_phys = dma_physical + core::mem::offset_of!(UsbDma, input_context) as u64;
    let ring_phys = dma_physical + core::mem::offset_of!(UsbDma, transfer_ring) as u64;
    let buffer_phys = dma_physical + core::mem::offset_of!(UsbDma, descriptor) as u64;
    prepare_address_context(dma, context, root_port, speed, initial_packet, ring_phys)?;
    controller.set_device_context(slot, device_phys);
    let address = controller
        .command([
            input_phys as u32,
            (input_phys >> 32) as u32,
            0,
            (11 << 10) | (u32::from(slot) << 24),
        ])
        .map_err(|_| UsbError::AddressDeviceFailed)?;
    if address.completion_code != 1 {
        return Err(UsbError::AddressDeviceFailed);
    }
    let device_address = unsafe {
        (read_volatile(
            core::ptr::addr_of!((*dma).device_context)
                .cast::<u8>()
                .add(12)
                .cast::<u32>(),
        ) & 0xff) as u8
    };
    if device_address == 0 {
        return Err(UsbError::AddressDeviceFailed);
    }
    let mut ring = ControlRing {
        physical: ring_phys,
        buffer_physical: buffer_phys,
        root_port,
        slot,
        short_packets: 0,
        dma,
    };
    let first = control(
        controller,
        &mut ring,
        slot,
        ControlRequest {
            request_type: 0x80,
            request: 6,
            value: 0x0100,
            index: 0,
            length: 8,
            input: true,
        },
    )?;
    if first < 8 {
        return Err(UsbError::MalformedDescriptor);
    }
    let ep0 = unsafe { read_volatile(core::ptr::addr_of!((*dma).descriptor[7])) } as u16;
    if ep0 == 0 {
        return Err(UsbError::MalformedDescriptor);
    }
    if ep0 != initial_packet {
        update_ep0(controller, dma, context, slot, input_phys, ep0, ring_phys)?;
    }
    let device_length = control(
        controller,
        &mut ring,
        slot,
        ControlRequest {
            request_type: 0x80,
            request: 6,
            value: 0x0100,
            index: 0,
            // Legacy enumeration overrequests one bounded packet; the device
            // descriptor still must complete with exactly 18 actual octets.
            // This exercises short Data Stage + final Status Stage handling.
            length: 64,
            input: true,
        },
    )?;
    if device_length != 18 {
        return Err(UsbError::MalformedDescriptor);
    }
    let device_bytes = unsafe { &(&(*dma).descriptor)[..18] };
    validate_header(device_bytes, 18, 1)?;
    let mut result = device_from_descriptor(root_port, slot, device_address, ep0, device_bytes)?;
    result.attachment_epoch = attachment_epoch;
    result.dma_slot = dma_slot.index();
    let header_length = control(
        controller,
        &mut ring,
        slot,
        ControlRequest {
            request_type: 0x80,
            request: 6,
            value: 0x0200,
            index: 0,
            length: 9,
            input: true,
        },
    )?;
    if header_length != 9 {
        return Err(UsbError::MalformedDescriptor);
    }
    let total =
        u16::from_le_bytes(unsafe { [(*dma).descriptor[2], (*dma).descriptor[3]] }) as usize;
    if total > MAX_CONFIGURATION_BYTES {
        return Err(UsbError::OversizedConfiguration);
    }
    if total < 9 {
        return Err(UsbError::MalformedDescriptor);
    }
    let configuration_length = control(
        controller,
        &mut ring,
        slot,
        ControlRequest {
            request_type: 0x80,
            request: 6,
            value: 0x0200,
            index: 0,
            length: total as u16,
            input: true,
        },
    )?;
    if configuration_length != total {
        return Err(UsbError::MalformedDescriptor);
    }
    parse_configuration(unsafe { &(&(*dma).descriptor)[..total] }, &mut result)?;
    control(
        controller,
        &mut ring,
        slot,
        ControlRequest {
            request_type: 0,
            request: 9,
            value: u16::from(result.configuration_value),
            index: 0,
            length: 0,
            input: false,
        },
    )?;
    result.control_transfers = MAX_CONTROL_TRANSFERS;
    result.short_packets = ring.short_packets;
    result.outstanding_control_transfer_limit = MAX_OUTSTANDING_CONTROL_TRANSFERS;
    result.enumeration_retries = MAX_ENUMERATION_RETRIES;
    result.sign_slots = USB_SIGN_SLOTS;
    result.transfer_trbs = TRANSFER_TRBS as u8;
    result.dma_bytes = core::mem::size_of::<UsbDma>() as u16;
    result.dma_alignment = core::mem::align_of::<UsbDma>() as u16;
    result.port_poll_steps = PORT_POLL_STEPS;
    #[cfg(feature = "scripted-keyboard-proof")]
    ring_proof::run(controller, &mut ring)?;
    Ok(result)
}

/// Enumerates at most the three admitted root devices in ascending port order,
/// assigning each an independent fixed DMA slot. The caller supplies the
/// attachment epoch for each admitted position so reattachment cannot inherit
/// an earlier device identity.
pub fn enumerate_attached_at_epochs(
    controller: &mut XhciReady,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
    attachment_epochs: [u32; USB_DEVICE_DMA_SLOTS as usize],
) -> Result<[Option<UsbDevice>; USB_DEVICE_DMA_SLOTS as usize], UsbError> {
    let dma_slots = [
        UsbDmaSlot::PRIMARY,
        UsbDmaSlot::SECONDARY,
        UsbDmaSlot::TERTIARY,
    ];
    let mut devices = [None, None, None];
    let mut count = 0_usize;
    for root_port in 1..=controller.maximum_ports() {
        if controller.port_status(root_port) & 1 == 0 {
            continue;
        }
        if count == devices.len() {
            return Err(UsbError::MultipleDevices);
        }
        devices[count] = Some(enumerate_root_port_at_epoch(
            controller,
            image_virtual_to_physical,
            root_port,
            dma_slots[count],
            attachment_epochs[count],
        )?);
        count += 1;
    }
    if count == 0 {
        Err(UsbError::NoDevice)
    } else {
        Ok(devices)
    }
}

pub(super) fn select_boot_protocol(
    controller: &mut XhciReady,
    device: &UsbDevice,
    interface: u8,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<(), UsbError> {
    let dma = device_dma_pointer(device)?;
    let dma_virtual = dma as u64;
    let dma_physical = image_virtual_to_physical(dma_virtual).ok_or(UsbError::DmaAddressInvalid)?;
    let mut ring = ControlRing {
        physical: dma_physical + core::mem::offset_of!(UsbDma, transfer_ring) as u64,
        buffer_physical: dma_physical + core::mem::offset_of!(UsbDma, descriptor) as u64,
        root_port: device.root_port,
        slot: device.slot,
        short_packets: 0,
        dma,
    };
    control(
        controller,
        &mut ring,
        device.slot,
        ControlRequest {
            request_type: 0x21,
            request: 11,
            value: 0,
            index: u16::from(interface),
            length: 0,
            input: false,
        },
    )?;
    Ok(())
}

fn prepare_address_context(
    dma: *mut UsbDma,
    context: usize,
    port: u8,
    speed: u8,
    packet: u16,
    ring: u64,
) -> Result<(), UsbError> {
    if context * 33 > 2112 {
        return Err(UsbError::ContextGeometry);
    }
    unsafe {
        write_context_u32(dma, 4, 3);
        write_context_u32(dma, context, (u32::from(speed) << 20) | (1 << 27));
        write_context_u32(dma, context + 4, u32::from(port) << 16);
        let ep = context * 2;
        write_context_u32(dma, ep + 4, (3 << 1) | (4 << 3) | (u32::from(packet) << 16));
        write_context_u32(dma, ep + 8, ring as u32 | 1);
        write_context_u32(dma, ep + 12, (ring >> 32) as u32);
        write_context_u32(dma, ep + 16, 8);
    }
    Ok(())
}

fn update_ep0(
    controller: &mut XhciReady,
    dma: *mut UsbDma,
    context: usize,
    slot: u8,
    input: u64,
    packet: u16,
    ring: u64,
) -> Result<(), UsbError> {
    unsafe {
        (*dma).input_context = [0; 2112];
        write_context_u32(dma, 4, 2);
        let ep = context * 2;
        write_context_u32(dma, ep + 4, (3 << 1) | (4 << 3) | (u32::from(packet) << 16));
        write_context_u32(dma, ep + 8, ring as u32 | 1);
        write_context_u32(dma, ep + 12, (ring >> 32) as u32);
        write_context_u32(dma, ep + 16, 8);
    }
    let event = controller.command([
        input as u32,
        (input >> 32) as u32,
        0,
        (13 << 10) | (u32::from(slot) << 24),
    ])?;
    if event.completion_code == 1 {
        Ok(())
    } else {
        Err(UsbError::AddressDeviceFailed)
    }
}

unsafe fn write_context_u32(dma: *mut UsbDma, offset: usize, value: u32) {
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!((*dma).input_context)
                .cast::<u8>()
                .add(offset)
                .cast::<u32>(),
            value,
        )
    };
}

#[cfg(test)]
#[path = "usb_tests.rs"]
mod tests;
