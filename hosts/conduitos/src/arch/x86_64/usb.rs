//! One bounded root-attached USB device realized through the xHCI Base.
//!
//! This module retains structural USB truth only. It neither parses HID reports
//! nor advertises a semantic input capability.

use core::{
    hint::spin_loop,
    ptr::{read_volatile, write_volatile},
};

use super::xhci::{Event, XhciReady};

#[path = "usb_descriptor.rs"]
pub(super) mod descriptor;
#[path = "usb_error.rs"]
mod error;

pub use descriptor::UsbDevice;
use descriptor::{
    MAX_CONFIGURATION_BYTES, device_from_descriptor, parse_configuration, validate_header,
};
pub use error::UsbError;
pub const MAX_CONTROL_TRANSFERS: u8 = 5;
pub const MAX_OUTSTANDING_CONTROL_TRANSFERS: u8 = 1;
pub const MAX_ENUMERATION_RETRIES: u8 = 0;
pub const USB_SIGN_SLOTS: u8 = 12;
const TRANSFER_TRBS: usize = 32;
const PORT_POLL_STEPS: u32 = 2_000_000;

#[repr(C, align(4096))]
pub(super) struct UsbDma {
    pub(super) device_context: [u8; 2048],
    pub(super) input_context: [u8; 2112],
    transfer_ring: [[u32; 4]; TRANSFER_TRBS],
    descriptor: [u8; MAX_CONFIGURATION_BYTES],
}

pub(super) static mut USB_DMA: UsbDma = UsbDma {
    device_context: [0; 2048],
    input_context: [0; 2112],
    transfer_ring: [[0; 4]; TRANSFER_TRBS],
    descriptor: [0; MAX_CONFIGURATION_BYTES],
};

struct ControlRing {
    enqueue: usize,
    cycle: u32,
    physical: u64,
    buffer_physical: u64,
    root_port: u8,
    short_packets: u8,
}

#[derive(Clone, Copy)]
struct ControlRequest {
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
    input: bool,
}

pub fn enumerate_one(
    controller: &mut XhciReady,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<UsbDevice, UsbError> {
    let dma_virtual = core::ptr::addr_of_mut!(USB_DMA) as u64;
    let dma_physical = image_virtual_to_physical(dma_virtual).ok_or(UsbError::DmaAddressInvalid)?;
    if dma_physical & 0xfff != 0 {
        return Err(UsbError::DmaAddressInvalid);
    }
    if !matches!(controller.context_bytes(), 32 | 64) {
        return Err(UsbError::ContextGeometry);
    }
    unsafe {
        USB_DMA = UsbDma {
            device_context: [0; 2048],
            input_context: [0; 2112],
            transfer_ring: [[0; 4]; TRANSFER_TRBS],
            descriptor: [0; MAX_CONFIGURATION_BYTES],
        }
    };
    let root_port = attached_root_port(controller)?;
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
    let context = controller.context_bytes();
    let device_phys = dma_physical + core::mem::offset_of!(UsbDma, device_context) as u64;
    let input_phys = dma_physical + core::mem::offset_of!(UsbDma, input_context) as u64;
    let ring_phys = dma_physical + core::mem::offset_of!(UsbDma, transfer_ring) as u64;
    let buffer_phys = dma_physical + core::mem::offset_of!(UsbDma, descriptor) as u64;
    prepare_address_context(context, root_port, speed, initial_packet, ring_phys)?;
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
            core::ptr::addr_of!(USB_DMA.device_context)
                .cast::<u8>()
                .add(12)
                .cast::<u32>(),
        ) & 0xff) as u8
    };
    if device_address == 0 {
        return Err(UsbError::AddressDeviceFailed);
    }
    let mut ring = ControlRing {
        enqueue: 0,
        cycle: 1,
        physical: ring_phys,
        buffer_physical: buffer_phys,
        root_port,
        short_packets: 0,
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
    let ep0 = unsafe { read_volatile(core::ptr::addr_of!(USB_DMA.descriptor[7])) } as u16;
    if ep0 == 0 {
        return Err(UsbError::MalformedDescriptor);
    }
    if ep0 != initial_packet {
        update_ep0(controller, context, slot, input_phys, ep0, ring_phys)?;
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
            length: 18,
            input: true,
        },
    )?;
    if device_length != 18 {
        return Err(UsbError::MalformedDescriptor);
    }
    let device_bytes = unsafe { &USB_DMA.descriptor[..18] };
    validate_header(device_bytes, 18, 1)?;
    let mut result = device_from_descriptor(root_port, slot, device_address, ep0, device_bytes)?;
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
        u16::from_le_bytes(unsafe { [USB_DMA.descriptor[2], USB_DMA.descriptor[3]] }) as usize;
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
    parse_configuration(unsafe { &USB_DMA.descriptor[..total] }, &mut result)?;
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
    Ok(result)
}

pub(super) fn select_boot_protocol(
    controller: &mut XhciReady,
    device: &UsbDevice,
    interface: u8,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<(), UsbError> {
    let dma_virtual = core::ptr::addr_of_mut!(USB_DMA) as u64;
    let dma_physical = image_virtual_to_physical(dma_virtual).ok_or(UsbError::DmaAddressInvalid)?;
    let mut ring = ControlRing {
        enqueue: 14,
        cycle: 1,
        physical: dma_physical + core::mem::offset_of!(UsbDma, transfer_ring) as u64,
        buffer_physical: dma_physical + core::mem::offset_of!(UsbDma, descriptor) as u64,
        root_port: device.root_port,
        short_packets: 0,
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

fn attached_root_port(controller: &XhciReady) -> Result<u8, UsbError> {
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

fn reset_port(controller: &XhciReady, port: u8) -> Result<(), UsbError> {
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

fn classify_port_reset(status: u32) -> Option<Result<(), UsbError>> {
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

fn prepare_address_context(
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
        write_context_u32(4, 3);
        write_context_u32(context, (u32::from(speed) << 20) | (1 << 27));
        write_context_u32(context + 4, u32::from(port) << 16);
        let ep = context * 2;
        write_context_u32(ep + 4, (3 << 1) | (4 << 3) | (u32::from(packet) << 16));
        write_context_u32(ep + 8, ring as u32 | 1);
        write_context_u32(ep + 12, (ring >> 32) as u32);
        write_context_u32(ep + 16, 8);
    }
    Ok(())
}

fn update_ep0(
    controller: &mut XhciReady,
    context: usize,
    slot: u8,
    input: u64,
    packet: u16,
    ring: u64,
) -> Result<(), UsbError> {
    unsafe {
        USB_DMA.input_context = [0; 2112];
        write_context_u32(4, 2);
        let ep = context * 2;
        write_context_u32(ep + 4, (3 << 1) | (4 << 3) | (u32::from(packet) << 16));
        write_context_u32(ep + 8, ring as u32 | 1);
        write_context_u32(ep + 12, (ring >> 32) as u32);
        write_context_u32(ep + 16, 8);
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

unsafe fn write_context_u32(offset: usize, value: u32) {
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(USB_DMA.input_context)
                .cast::<u8>()
                .add(offset)
                .cast::<u32>(),
            value,
        )
    };
}

fn control(
    controller: &mut XhciReady,
    ring: &mut ControlRing,
    slot: u8,
    request: ControlRequest,
) -> Result<usize, UsbError> {
    let ControlRequest {
        request_type,
        request,
        value,
        index,
        length,
        input,
    } = request;
    let count = if length == 0 { 2 } else { 3 };
    if ring.enqueue + count >= TRANSFER_TRBS {
        return Err(UsbError::TransferRingFull);
    }
    let setup = [
        u32::from(request_type) | (u32::from(request) << 8) | (u32::from(value) << 16),
        u32::from(index) | (u32::from(length) << 16),
        8,
        (2 << 10) | (1 << 6) | (if input { 3 << 16 } else { 0 }) | ring.cycle,
    ];
    put_transfer(ring.enqueue, setup);
    ring.enqueue += 1;
    if length != 0 {
        let data = [
            ring.buffer_physical as u32,
            (ring.buffer_physical >> 32) as u32,
            u32::from(length),
            (3 << 10) | (u32::from(input) << 16) | ring.cycle,
        ];
        put_transfer(ring.enqueue, data);
        ring.enqueue += 1;
    }
    let status_index = ring.enqueue;
    put_transfer(
        status_index,
        [
            0,
            0,
            0,
            (4 << 10) | (u32::from(!input || length == 0) << 16) | (1 << 5) | ring.cycle,
        ],
    );
    ring.enqueue += 1;
    controller.ring_endpoint(slot, 1);
    let mut event = controller.next_event()?;
    for _ in 0..TRANSFER_TRBS {
        if event.event_type != 34 {
            break;
        }
        event = controller.next_event()?;
    }
    if validate_transfer_event(event, slot, ring.physical + (status_index * 16) as u64)? {
        ring.short_packets = ring.short_packets.saturating_add(1);
    }
    if controller.port_status(ring.root_port) & 1 == 0 {
        return Err(UsbError::DeviceVanished);
    }
    Ok(usize::from(length.saturating_sub(event.residual as u16)))
}

fn put_transfer(index: usize, trb: [u32; 4]) {
    unsafe { write_volatile(core::ptr::addr_of_mut!(USB_DMA.transfer_ring[index]), trb) };
}

fn validate_transfer_event(event: Event, slot: u8, pointer: u64) -> Result<bool, UsbError> {
    if event.event_type != 32 || event.pointer != pointer {
        return Err(UsbError::WrongController);
    }
    if event.slot != slot {
        return Err(UsbError::WrongSlot);
    }
    if event.endpoint != 1 {
        return Err(UsbError::WrongEndpoint);
    }
    match event.completion_code {
        1 => Ok(false),
        13 => Ok(true),
        6 => Err(UsbError::ControlStall),
        _ => Err(UsbError::ControlError),
    }
}

#[cfg(test)]
#[path = "usb_tests.rs"]
mod tests;
