//! One finite USB HID boot-keyboard implementation above USB enumeration.
//!
//! This module retains HID-local usages and transitions only. It performs no
//! layout, text, Unicode, or semantic `input/keyboard` conversion.

use core::ptr::{read_volatile, write_volatile};

use super::{
    usb::{USB_DMA, UsbDevice, UsbError, select_boot_protocol},
    xhci::{Event, XhciError, XhciReady},
};

#[path = "hid_report.rs"]
mod report;
use report::{BootReport, derive_transitions, parse_report, retain_transition};
#[path = "hid_session.rs"]
mod session;
pub use session::{HidKeyboardSession, finish_boot_keyboard, receive_first_boot_keyboard_report};

pub const BOOT_REPORT_BYTES: usize = 8;
pub const MAX_TRANSITIONS_PER_REPORT: usize = 20;
pub const REPORT_BUFFERS: usize = 2;
pub const MAX_SESSION_REPORTS: usize = 64;
pub const MAX_SESSION_TRANSITIONS: usize = 64;
pub const MAX_OUTSTANDING_INTERRUPT_TRANSFERS: u8 = 2;
pub const INTERRUPT_TRANSFER_TRBS: usize = 64;
pub const HID_SIGN_SLOTS: u8 = 8;
pub const INTERRUPT_POLL_WINDOWS: u16 = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HidError {
    InterfaceAbsent,
    AmbiguousInterface,
    NonBootInterface,
    MouseProtocol,
    EndpointAbsent,
    AmbiguousEndpoint,
    InvalidEndpoint,
    UnsupportedPacketSize,
    SetProtocolStall,
    SetProtocolError,
    ConfigureEndpointFailed,
    DmaAddressInvalid,
    TransferTimeout,
    TransferStall,
    TransferError,
    WrongDevice,
    WrongEndpoint,
    WrongCompletion,
    DeviceRemoved,
    ShortReport,
    ReservedByte,
    Rollover,
    DuplicateUsage,
    TransitionOverflow,
    TransferOverflow,
}

impl HidError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InterfaceAbsent => "hid-interface-absent",
            Self::AmbiguousInterface => "hid-interface-ambiguous",
            Self::NonBootInterface => "hid-interface-not-boot",
            Self::MouseProtocol => "hid-interface-is-mouse",
            Self::EndpointAbsent => "hid-interrupt-in-absent",
            Self::AmbiguousEndpoint => "hid-interrupt-in-ambiguous",
            Self::InvalidEndpoint => "hid-interrupt-in-invalid",
            Self::UnsupportedPacketSize => "hid-packet-size-unsupported",
            Self::SetProtocolStall => "hid-set-protocol-stall",
            Self::SetProtocolError => "hid-set-protocol-error",
            Self::ConfigureEndpointFailed => "hid-configure-endpoint-failed",
            Self::DmaAddressInvalid => "hid-dma-address-invalid",
            Self::TransferTimeout => "hid-transfer-timeout",
            Self::TransferStall => "hid-transfer-stall",
            Self::TransferError => "hid-transfer-error",
            Self::WrongDevice => "hid-completion-wrong-device",
            Self::WrongEndpoint => "hid-completion-wrong-endpoint",
            Self::WrongCompletion => "hid-completion-wrong-trb",
            Self::DeviceRemoved => "hid-device-removed",
            Self::ShortReport => "hid-report-short",
            Self::ReservedByte => "hid-report-reserved-byte",
            Self::Rollover => "hid-report-rollover",
            Self::DuplicateUsage => "hid-report-duplicate-usage",
            Self::TransitionOverflow => "hid-transition-overflow",
            Self::TransferOverflow => "hid-transfer-overflow",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HidKeyTransition {
    usage: u8,
    pressed: bool,
    modifiers: u8,
}

impl HidKeyTransition {
    pub const fn usage(self) -> u8 {
        self.usage
    }

    pub const fn pressed(self) -> bool {
        self.pressed
    }

    pub const fn modifiers(self) -> u8 {
        self.modifiers
    }

    /// Crosses the local adapter boundary only after HID report validation.
    pub const fn into_local_rescue(self) -> crate::local_rescue::ValidatedLocalTransition {
        crate::local_rescue::ValidatedLocalTransition::from_validated_hid(
            self.usage,
            self.pressed,
            self.modifiers,
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct HidProof {
    pub interface_number: u8,
    pub endpoint_address: u8,
    pub endpoint_dci: u8,
    pub endpoint_maximum_packet_size: u16,
    pub endpoint_interval: u8,
    pub set_protocol_transfers: u8,
    pub interrupt_transfers: u8,
    pub report_bytes: u8,
    pub report_buffers: u8,
    pub maximum_outstanding_interrupt_transfers: u8,
    pub maximum_transitions_per_report: u8,
    pub transfer_trbs: u8,
    pub dma_bytes: u16,
    pub dma_alignment: u16,
    pub sign_slots: u8,
    pub interrupt_poll_windows: u16,
    pub transition_count: u8,
    pub transitions: [HidKeyTransition; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidKeyboardReady {
    pub interface_number: u8,
    pub endpoint_address: u8,
    pub endpoint_dci: u8,
    pub endpoint_maximum_packet_size: u16,
    pub endpoint_interval: u8,
    pub report_buffers: u16,
    pub transition_slots: u16,
    pub operation_slots: u16,
    dma_physical: u64,
}

#[repr(C, align(4096))]
struct HidDma {
    input_context: [u8; 2112],
    transfer_ring: [[u32; 4]; INTERRUPT_TRANSFER_TRBS],
    reports: [[u8; BOOT_REPORT_BYTES]; REPORT_BUFFERS],
}

static mut HID_DMA: HidDma = HidDma {
    input_context: [0; 2112],
    transfer_ring: [[0; 4]; INTERRUPT_TRANSFER_TRBS],
    reports: [[0; BOOT_REPORT_BYTES]; REPORT_BUFFERS],
};

pub fn run_boot_keyboard(
    controller: &mut XhciReady,
    device: &UsbDevice,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<HidProof, HidError> {
    let ready = prepare_boot_keyboard(controller, device, image_virtual_to_physical)?;
    receive_boot_keyboard(controller, device, ready)
}

pub fn prepare_boot_keyboard(
    controller: &mut XhciReady,
    device: &UsbDevice,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<HidKeyboardReady, HidError> {
    let (interface, endpoint) = match_keyboard(device)?;
    select_boot_protocol(
        controller,
        device,
        interface.number,
        image_virtual_to_physical,
    )
    .map_err(map_protocol_error)?;
    let dma_virtual = core::ptr::addr_of_mut!(HID_DMA) as u64;
    let dma_physical = image_virtual_to_physical(dma_virtual).ok_or(HidError::DmaAddressInvalid)?;
    if dma_physical & 0xfff != 0 {
        return Err(HidError::DmaAddressInvalid);
    }
    unsafe {
        HID_DMA = HidDma {
            input_context: [0; 2112],
            transfer_ring: [[0; 4]; INTERRUPT_TRANSFER_TRBS],
            reports: [[0; BOOT_REPORT_BYTES]; REPORT_BUFFERS],
        };
    }
    let dci = endpoint_dci(endpoint.address)?;
    configure_interrupt_endpoint(controller, device, endpoint, dci, dma_physical)?;
    Ok(HidKeyboardReady {
        interface_number: interface.number,
        endpoint_address: endpoint.address,
        endpoint_dci: dci,
        endpoint_maximum_packet_size: endpoint.maximum_packet_size,
        endpoint_interval: endpoint.interval,
        report_buffers: REPORT_BUFFERS as u16,
        transition_slots: HID_SIGN_SLOTS as u16,
        operation_slots: MAX_OUTSTANDING_INTERRUPT_TRANSFERS as u16,
        dma_physical,
    })
}

pub fn receive_boot_keyboard(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: HidKeyboardReady,
) -> Result<HidProof, HidError> {
    let session = receive_first_boot_keyboard_report(controller, device, ready)?;
    finish_boot_keyboard(controller, device, session)
}

fn map_protocol_error(error: UsbError) -> HidError {
    if error == UsbError::ControlStall {
        HidError::SetProtocolStall
    } else {
        HidError::SetProtocolError
    }
}

fn match_keyboard(
    device: &UsbDevice,
) -> Result<
    (
        super::usb::descriptor::UsbInterface,
        super::usb::descriptor::UsbEndpoint,
    ),
    HidError,
> {
    let mut matched = None;
    let mut saw_hid = false;
    for interface in device.interfaces[..usize::from(device.interface_count)]
        .iter()
        .copied()
    {
        if interface.class != 3 {
            continue;
        }
        saw_hid = true;
        if interface.protocol == 2 {
            return Err(HidError::MouseProtocol);
        }
        if interface.subclass != 1 || interface.protocol != 1 || interface.alternate_setting != 0 {
            continue;
        }
        if matched.replace(interface).is_some() {
            return Err(HidError::AmbiguousInterface);
        }
    }
    let interface = matched.ok_or(if saw_hid {
        HidError::NonBootInterface
    } else {
        HidError::InterfaceAbsent
    })?;
    let mut endpoint = None;
    let index = device.interfaces[..usize::from(device.interface_count)]
        .iter()
        .position(|candidate| *candidate == interface)
        .ok_or(HidError::InterfaceAbsent)? as u8;
    for candidate in device.endpoints[..usize::from(device.endpoint_count)]
        .iter()
        .copied()
        .filter(|candidate| candidate.interface_index == index)
    {
        if !candidate.direction_in || candidate.transfer_type != 3 {
            continue;
        }
        if endpoint.replace(candidate).is_some() {
            return Err(HidError::AmbiguousEndpoint);
        }
    }
    let endpoint = endpoint.ok_or(HidError::EndpointAbsent)?;
    if endpoint.maximum_packet_size != BOOT_REPORT_BYTES as u16 {
        return Err(HidError::UnsupportedPacketSize);
    }
    if endpoint.interval == 0 {
        return Err(HidError::InvalidEndpoint);
    }
    Ok((interface, endpoint))
}

fn endpoint_dci(address: u8) -> Result<u8, HidError> {
    let number = address & 0x0f;
    if number == 0 || address & 0x70 != 0 || address & 0x80 == 0 {
        return Err(HidError::InvalidEndpoint);
    }
    Ok(number * 2 + 1)
}

fn configure_interrupt_endpoint(
    controller: &mut XhciReady,
    device: &UsbDevice,
    endpoint: super::usb::descriptor::UsbEndpoint,
    dci: u8,
    dma_physical: u64,
) -> Result<(), HidError> {
    let context = controller.context_bytes();
    let input_physical = dma_physical + core::mem::offset_of!(HidDma, input_context) as u64;
    let ring_physical = dma_physical + core::mem::offset_of!(HidDma, transfer_ring) as u64;
    unsafe {
        HID_DMA.input_context = [0; 2112];
        write_input_u32(4, 1 | (1 << dci));
        for offset in (0..context).step_by(4) {
            let value = read_volatile(
                core::ptr::addr_of!(USB_DMA.device_context)
                    .cast::<u8>()
                    .add(offset)
                    .cast::<u32>(),
            );
            write_input_u32(context + offset, value);
        }
        let slot_context = read_volatile(
            core::ptr::addr_of!(HID_DMA.input_context)
                .cast::<u8>()
                .add(context)
                .cast::<u32>(),
        );
        write_input_u32(
            context,
            (slot_context & !(0x1f << 27)) | (u32::from(dci) << 27),
        );
        let slot_speed =
            (read_volatile(core::ptr::addr_of!(USB_DMA.device_context).cast::<u32>()) >> 20) & 0xf;
        let interval = match slot_speed {
            1 | 2 => endpoint
                .interval
                .checked_add(2)
                .ok_or(HidError::InvalidEndpoint)?,
            3..=5 if endpoint.interval <= 16 => endpoint.interval - 1,
            _ => return Err(HidError::InvalidEndpoint),
        };
        let ep = context * (usize::from(dci) + 1);
        write_input_u32(ep, u32::from(interval) << 16);
        write_input_u32(
            ep + 4,
            (3 << 1) | (7 << 3) | (u32::from(endpoint.maximum_packet_size) << 16),
        );
        write_input_u32(ep + 8, ring_physical as u32 | 1);
        write_input_u32(ep + 12, (ring_physical >> 32) as u32);
        write_input_u32(
            ep + 16,
            u32::from(endpoint.maximum_packet_size)
                | (u32::from(endpoint.maximum_packet_size) << 16),
        );
    }
    let event = controller
        .command([
            input_physical as u32,
            (input_physical >> 32) as u32,
            0,
            (12 << 10) | (u32::from(device.slot) << 24),
        ])
        .map_err(|_| HidError::ConfigureEndpointFailed)?;
    if event.completion_code != 1 || event.slot != device.slot {
        return Err(HidError::ConfigureEndpointFailed);
    }
    Ok(())
}

unsafe fn write_input_u32(offset: usize, value: u32) {
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(HID_DMA.input_context)
                .cast::<u8>()
                .add(offset)
                .cast::<u32>(),
            value,
        )
    }
}

pub(super) fn receive_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    dci: u8,
    index: usize,
    dma_physical: u64,
) -> Result<(), HidError> {
    let ring = dma_physical + core::mem::offset_of!(HidDma, transfer_ring) as u64;
    if index == 0 {
        for report_index in 0..REPORT_BUFFERS {
            let buffer = dma_physical
                + core::mem::offset_of!(HidDma, reports) as u64
                + (report_index * BOOT_REPORT_BYTES) as u64;
            unsafe {
                write_volatile(
                    core::ptr::addr_of_mut!(HID_DMA.transfer_ring[report_index]),
                    [
                        buffer as u32,
                        (buffer >> 32) as u32,
                        BOOT_REPORT_BYTES as u32,
                        (1 << 10) | (1 << 5) | 1,
                    ],
                );
            }
        }
        controller.ring_endpoint(device.slot, dci);
        super::serial::early_write(b"CONDUIT_BOOT_STAGE hid-awaiting-qemu-key\n");
    } else if index >= REPORT_BUFFERS {
        if index >= MAX_SESSION_REPORTS {
            return Err(HidError::TransferOverflow);
        }
        let buffer_slot = index % REPORT_BUFFERS;
        let buffer = dma_physical
            + core::mem::offset_of!(HidDma, reports) as u64
            + (buffer_slot * BOOT_REPORT_BYTES) as u64;
        unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(HID_DMA.transfer_ring[index]),
                [
                    buffer as u32,
                    (buffer >> 32) as u32,
                    BOOT_REPORT_BYTES as u32,
                    (1 << 10) | (1 << 5) | 1,
                ],
            );
        }
        controller.ring_endpoint(device.slot, dci);
    }
    let mut completed = None;
    for _ in 0..INTERRUPT_POLL_WINDOWS {
        ensure_device_present(controller.port_status(device.root_port))?;
        match controller.next_event() {
            Ok(event) if event.event_type == 34 => return Err(HidError::DeviceRemoved),
            Ok(event) => {
                completed = Some(event);
                break;
            }
            Err(XhciError::CommandTimeout) => {}
            Err(_) => {
                ensure_device_present(controller.port_status(device.root_port))?;
                return Err(HidError::TransferError);
            }
        }
    }
    let event = completed.ok_or(HidError::TransferTimeout)?;
    validate_interrupt_event(event, device.slot, dci, ring + (index * 16) as u64)?;
    ensure_device_present(controller.port_status(device.root_port))?;
    Ok(())
}

fn ensure_device_present(port_status: u32) -> Result<(), HidError> {
    if port_status & 1 == 0 {
        Err(HidError::DeviceRemoved)
    } else {
        Ok(())
    }
}

fn validate_interrupt_event(
    event: Event,
    slot: u8,
    endpoint: u8,
    pointer: u64,
) -> Result<(), HidError> {
    if event.event_type != 32 || event.pointer != pointer {
        return Err(HidError::WrongCompletion);
    }
    if event.slot != slot {
        return Err(HidError::WrongDevice);
    }
    if event.endpoint != endpoint {
        return Err(HidError::WrongEndpoint);
    }
    match event.completion_code {
        1 if event.residual == 0 => Ok(()),
        1 | 13 => Err(HidError::ShortReport),
        6 => Err(HidError::TransferStall),
        _ => Err(HidError::TransferError),
    }
}

#[cfg(test)]
#[path = "hid_tests.rs"]
mod tests;
