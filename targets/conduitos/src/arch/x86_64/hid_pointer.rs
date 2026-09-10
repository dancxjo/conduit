//! Finite USB HID boot-pointer realization above structural USB enumeration.

use conduit_semantic_catalog::NormalizedPointerSample;
use core::ptr::{read_volatile, write_volatile};

use super::{
    usb::{UsbDevice, descriptor::UsbEndpoint, dma::device_dma_pointer, select_boot_protocol},
    xhci::{Event, XhciError, XhciReady},
};

// QEMU's pinned `usb-mouse` exposes a four-byte boot-pointer endpoint:
// buttons, relative X, relative Y, and wheel. V3 does not claim wheel meaning.
pub const POINTER_REPORT_BYTES: usize = 4;
pub const POINTER_REPORT_BUFFERS: usize = 2;
pub const POINTER_TRANSFER_TRBS: usize = 64;
pub const POINTER_QUEUE_CAPACITY: u64 = 2;
pub const POINTER_DELTA_SCALE: i64 = 4_000;
pub const POINTER_POLL_WINDOWS: u16 = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HidPointerError {
    InterfaceAbsent,
    AmbiguousInterface,
    NonBootInterface,
    EndpointAbsent,
    AmbiguousEndpoint,
    InvalidEndpoint,
    UnsupportedPacketSize,
    SetProtocolFailed,
    ConfigureEndpointFailed,
    DmaAddressInvalid,
    TransferOverflow,
    TransferTimeout,
    TransferStall,
    TransferError,
    WrongDevice,
    WrongEndpoint,
    WrongCompletion,
    DeviceRemoved,
    ReservedButtons,
    SequenceOverflow,
}

impl HidPointerError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InterfaceAbsent => "hid-pointer-interface-absent",
            Self::AmbiguousInterface => "hid-pointer-interface-ambiguous",
            Self::NonBootInterface => "hid-pointer-interface-not-boot",
            Self::EndpointAbsent => "hid-pointer-interrupt-in-absent",
            Self::AmbiguousEndpoint => "hid-pointer-interrupt-in-ambiguous",
            Self::InvalidEndpoint => "hid-pointer-interrupt-in-invalid",
            Self::UnsupportedPacketSize => "hid-pointer-packet-size-unsupported",
            Self::SetProtocolFailed => "hid-pointer-set-protocol-failed",
            Self::ConfigureEndpointFailed => "hid-pointer-configure-endpoint-failed",
            Self::DmaAddressInvalid => "hid-pointer-dma-address-invalid",
            Self::TransferOverflow => "hid-pointer-transfer-overflow",
            Self::TransferTimeout => "hid-pointer-transfer-timeout",
            Self::TransferStall => "hid-pointer-transfer-stall",
            Self::TransferError => "hid-pointer-transfer-error",
            Self::WrongDevice => "hid-pointer-completion-wrong-device",
            Self::WrongEndpoint => "hid-pointer-completion-wrong-endpoint",
            Self::WrongCompletion => "hid-pointer-completion-wrong-trb",
            Self::DeviceRemoved => "hid-pointer-device-removed",
            Self::ReservedButtons => "hid-pointer-buttons-reserved",
            Self::SequenceOverflow => "hid-pointer-sequence-overflow",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidPointerReady {
    pub interface_number: u8,
    pub endpoint_address: u8,
    pub endpoint_dci: u8,
    pub endpoint_interval: u8,
    pub report_buffers: u8,
    pub transfer_trbs: u8,
    dma_physical: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidPointerSession {
    ready: HidPointerReady,
    next_transfer: usize,
    position_x: i64,
    position_y: i64,
    primary_pressed: bool,
    sequence: u64,
}

#[repr(C, align(4096))]
struct PointerDma {
    input_context: [u8; 2112],
    transfer_ring: [[u32; 4]; POINTER_TRANSFER_TRBS],
    reports: [[u8; POINTER_REPORT_BYTES]; POINTER_REPORT_BUFFERS],
}

static mut POINTER_DMA: PointerDma = PointerDma {
    input_context: [0; 2112],
    transfer_ring: [[0; 4]; POINTER_TRANSFER_TRBS],
    reports: [[0; POINTER_REPORT_BYTES]; POINTER_REPORT_BUFFERS],
};

pub fn prepare_boot_pointer(
    controller: &mut XhciReady,
    device: &UsbDevice,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<HidPointerReady, HidPointerError> {
    let (interface, endpoint) = match_pointer(device)?;
    select_boot_protocol(
        controller,
        device,
        interface.number,
        image_virtual_to_physical,
    )
    .map_err(|_| HidPointerError::SetProtocolFailed)?;
    let dma_virtual = core::ptr::addr_of_mut!(POINTER_DMA) as u64;
    let dma_physical =
        image_virtual_to_physical(dma_virtual).ok_or(HidPointerError::DmaAddressInvalid)?;
    if dma_physical & 0xfff != 0 {
        return Err(HidPointerError::DmaAddressInvalid);
    }
    unsafe {
        POINTER_DMA = PointerDma {
            input_context: [0; 2112],
            transfer_ring: [[0; 4]; POINTER_TRANSFER_TRBS],
            reports: [[0; POINTER_REPORT_BYTES]; POINTER_REPORT_BUFFERS],
        };
    }
    let dci = endpoint_dci(endpoint.address)?;
    configure_endpoint(controller, device, endpoint, dci, dma_physical)?;
    Ok(HidPointerReady {
        interface_number: interface.number,
        endpoint_address: endpoint.address,
        endpoint_dci: dci,
        endpoint_interval: endpoint.interval,
        report_buffers: POINTER_REPORT_BUFFERS as u8,
        transfer_trbs: POINTER_TRANSFER_TRBS as u8,
        dma_physical,
    })
}

pub const fn start_pointer_session(ready: HidPointerReady) -> HidPointerSession {
    HidPointerSession {
        ready,
        next_transfer: 0,
        position_x: 500_000,
        position_y: 500_000,
        primary_pressed: false,
        sequence: 0,
    }
}

impl HidPointerSession {
    pub const fn position(&self) -> (i64, i64) {
        (self.position_x, self.position_y)
    }

    pub const fn primary_pressed(&self) -> bool {
        self.primary_pressed
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn receive(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
    ) -> Result<NormalizedPointerSample, HidPointerError> {
        if self.next_transfer >= POINTER_TRANSFER_TRBS {
            return Err(HidPointerError::TransferOverflow);
        }
        let index = self.next_transfer;
        submit_report(controller, device, self.ready, index)?;
        let report = unsafe { POINTER_DMA.reports[index % POINTER_REPORT_BUFFERS] };
        let sample = apply_report(self.position_x, self.position_y, self.sequence, report)?;
        self.position_x = sample.position_x;
        self.position_y = sample.position_y;
        self.primary_pressed = sample.primary_pressed;
        self.sequence = sample.sequence;
        self.next_transfer += 1;
        Ok(sample)
    }
}

fn match_pointer(
    device: &UsbDevice,
) -> Result<
    (
        super::usb::descriptor::UsbInterface,
        super::usb::descriptor::UsbEndpoint,
    ),
    HidPointerError,
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
        if interface.subclass != 1 || interface.protocol != 2 || interface.alternate_setting != 0 {
            continue;
        }
        if matched.replace(interface).is_some() {
            return Err(HidPointerError::AmbiguousInterface);
        }
    }
    let interface = matched.ok_or(if saw_hid {
        HidPointerError::NonBootInterface
    } else {
        HidPointerError::InterfaceAbsent
    })?;
    let interface_index = device.interfaces[..usize::from(device.interface_count)]
        .iter()
        .position(|candidate| *candidate == interface)
        .ok_or(HidPointerError::InterfaceAbsent)? as u8;
    let mut endpoint = None;
    for candidate in device.endpoints[..usize::from(device.endpoint_count)]
        .iter()
        .copied()
        .filter(|candidate| candidate.interface_index == interface_index)
    {
        if !candidate.direction_in || candidate.transfer_type != 3 {
            continue;
        }
        if endpoint.replace(candidate).is_some() {
            return Err(HidPointerError::AmbiguousEndpoint);
        }
    }
    let endpoint = endpoint.ok_or(HidPointerError::EndpointAbsent)?;
    if endpoint.maximum_packet_size != POINTER_REPORT_BYTES as u16 {
        return Err(HidPointerError::UnsupportedPacketSize);
    }
    if endpoint.interval == 0 {
        return Err(HidPointerError::InvalidEndpoint);
    }
    Ok((interface, endpoint))
}

fn endpoint_dci(address: u8) -> Result<u8, HidPointerError> {
    let number = address & 0x0f;
    if number == 0 || address & 0x70 != 0 || address & 0x80 == 0 {
        Err(HidPointerError::InvalidEndpoint)
    } else {
        Ok(number * 2 + 1)
    }
}

fn configure_endpoint(
    controller: &mut XhciReady,
    device: &UsbDevice,
    endpoint: UsbEndpoint,
    dci: u8,
    dma_physical: u64,
) -> Result<(), HidPointerError> {
    let usb_dma = device_dma_pointer(device).map_err(|_| HidPointerError::DmaAddressInvalid)?;
    let context = controller.context_bytes();
    let input_physical = dma_physical + core::mem::offset_of!(PointerDma, input_context) as u64;
    let ring_physical = dma_physical + core::mem::offset_of!(PointerDma, transfer_ring) as u64;
    unsafe {
        POINTER_DMA.input_context = [0; 2112];
        write_input_u32(4, 1 | (1 << dci));
        for offset in (0..context).step_by(4) {
            let value = read_volatile(
                core::ptr::addr_of!((*usb_dma).device_context)
                    .cast::<u8>()
                    .add(offset)
                    .cast::<u32>(),
            );
            write_input_u32(context + offset, value);
        }
        let slot_context = read_volatile(
            core::ptr::addr_of!(POINTER_DMA.input_context)
                .cast::<u8>()
                .add(context)
                .cast::<u32>(),
        );
        write_input_u32(
            context,
            (slot_context & !(0x1f << 27)) | (u32::from(dci) << 27),
        );
        let speed = (read_volatile(core::ptr::addr_of!((*usb_dma).device_context).cast::<u32>())
            >> 20)
            & 0xf;
        let interval = match speed {
            1 | 2 => endpoint
                .interval
                .checked_add(2)
                .ok_or(HidPointerError::InvalidEndpoint)?,
            3..=5 if endpoint.interval <= 16 => endpoint.interval - 1,
            _ => return Err(HidPointerError::InvalidEndpoint),
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
        .map_err(|_| HidPointerError::ConfigureEndpointFailed)?;
    if event.completion_code == 1 && event.slot == device.slot {
        Ok(())
    } else {
        Err(HidPointerError::ConfigureEndpointFailed)
    }
}

unsafe fn write_input_u32(offset: usize, value: u32) {
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(POINTER_DMA.input_context)
                .cast::<u8>()
                .add(offset)
                .cast::<u32>(),
            value,
        )
    }
}

fn submit_report(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: HidPointerReady,
    index: usize,
) -> Result<(), HidPointerError> {
    let ring = ready.dma_physical + core::mem::offset_of!(PointerDma, transfer_ring) as u64;
    let report_slot = index % POINTER_REPORT_BUFFERS;
    let buffer = ready.dma_physical
        + core::mem::offset_of!(PointerDma, reports) as u64
        + (report_slot * POINTER_REPORT_BYTES) as u64;
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(POINTER_DMA.transfer_ring[index]),
            [
                buffer as u32,
                (buffer >> 32) as u32,
                POINTER_REPORT_BYTES as u32,
                (1 << 10) | (1 << 5) | 1,
            ],
        );
    }
    controller.ring_endpoint(device.slot, ready.endpoint_dci);
    let mut completed = None;
    let mut retired_relinquished_completion = false;
    for _ in 0..POINTER_POLL_WINDOWS {
        ensure_present(controller.port_status(device.root_port))?;
        match controller.next_event() {
            Ok(event) if event.event_type == 34 => return Err(HidPointerError::DeviceRemoved),
            Ok(event)
                if index == 0
                    && event.event_type == 32
                    && event.slot != device.slot
                    && !retired_relinquished_completion =>
            {
                // The keyboard source deliberately arms its next report before
                // servicing semantic input. A pointer-mode handoff may therefore
                // inherit exactly one completion from that relinquished USB slot.
                // Retire only that first foreign transfer completion; the pointer
                // endpoint and TRB identity remain exact below.
                retired_relinquished_completion = true;
            }
            Ok(event) => {
                completed = Some(event);
                break;
            }
            Err(XhciError::CommandTimeout) => {}
            Err(_) => return Err(HidPointerError::TransferError),
        }
    }
    validate_event(
        completed.ok_or(HidPointerError::TransferTimeout)?,
        device.slot,
        ready.endpoint_dci,
        ring + (index * 16) as u64,
    )?;
    ensure_present(controller.port_status(device.root_port))
}

fn ensure_present(port_status: u32) -> Result<(), HidPointerError> {
    if port_status & 1 == 0 {
        Err(HidPointerError::DeviceRemoved)
    } else {
        Ok(())
    }
}

fn validate_event(
    event: Event,
    slot: u8,
    endpoint: u8,
    pointer: u64,
) -> Result<(), HidPointerError> {
    if event.event_type != 32 || event.pointer != pointer {
        return Err(HidPointerError::WrongCompletion);
    }
    if event.slot != slot {
        return Err(HidPointerError::WrongDevice);
    }
    if event.endpoint != endpoint {
        return Err(HidPointerError::WrongEndpoint);
    }
    match event.completion_code {
        1 if event.residual == 0 => Ok(()),
        6 => Err(HidPointerError::TransferStall),
        _ => Err(HidPointerError::TransferError),
    }
}

fn apply_report(
    position_x: i64,
    position_y: i64,
    sequence: u64,
    report: [u8; POINTER_REPORT_BYTES],
) -> Result<NormalizedPointerSample, HidPointerError> {
    if report[0] & !0x07 != 0 {
        return Err(HidPointerError::ReservedButtons);
    }
    let delta_x = i64::from(report[1] as i8) * POINTER_DELTA_SCALE;
    let delta_y = i64::from(report[2] as i8) * POINTER_DELTA_SCALE;
    Ok(NormalizedPointerSample {
        position_x: position_x.saturating_add(delta_x).clamp(0, 1_000_000),
        position_y: position_y.saturating_add(delta_y).clamp(0, 1_000_000),
        delta_x,
        delta_y,
        primary_pressed: report[0] & 1 != 0,
        coalesced: 0,
        dropped: 0,
        queue_capacity: POINTER_QUEUE_CAPACITY,
        sequence: sequence
            .checked_add(1)
            .ok_or(HidPointerError::SequenceOverflow)?,
    })
}

#[cfg(test)]
#[path = "hid_pointer_tests.rs"]
mod tests;
