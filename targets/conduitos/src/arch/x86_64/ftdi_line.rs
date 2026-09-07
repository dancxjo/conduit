//! Exact bounded FTDI FT232BM byte carrier for QEMU's `usb-serial` device.

use core::ptr::{read_volatile, write_volatile};

use super::{
    usb::{UsbDevice, descriptor::UsbEndpoint, dma::device_dma_pointer},
    xhci::{Event, XhciError, XhciReady},
};

pub const FTDI_VENDOR_ID: u16 = 0x0403;
pub const FTDI_PRODUCT_ID: u16 = 0x6001;
pub const FTDI_DEVICE_VERSION: u16 = 0x0400;
pub const FTDI_PACKET_BYTES: usize = 64;
pub const FTDI_PAYLOAD_BYTES: usize = FTDI_PACKET_BYTES - 2;
pub const FTDI_TRANSFER_TRBS: usize = 32;
pub const FTDI_POLL_WINDOWS: u16 = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FtdiLineError {
    WrongDevice,
    InterfaceAbsent,
    AmbiguousInterface,
    EndpointAbsent,
    AmbiguousEndpoint,
    InvalidEndpoint,
    ConfigureEndpointsFailed,
    DmaAddressInvalid,
    EmptyPayload,
    OversizedPayload,
    TransferOverflow,
    TransferTimeout,
    TransferStall,
    TransferError,
    WrongCompletion,
    WrongSlot,
    WrongEndpoint,
    DeviceRemoved,
    InvalidStatus,
}

impl FtdiLineError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongDevice => "ftdi-line-device-mismatch",
            Self::InterfaceAbsent => "ftdi-line-interface-absent",
            Self::AmbiguousInterface => "ftdi-line-interface-ambiguous",
            Self::EndpointAbsent => "ftdi-line-endpoint-absent",
            Self::AmbiguousEndpoint => "ftdi-line-endpoint-ambiguous",
            Self::InvalidEndpoint => "ftdi-line-endpoint-invalid",
            Self::ConfigureEndpointsFailed => "ftdi-line-configure-endpoints-failed",
            Self::DmaAddressInvalid => "ftdi-line-dma-address-invalid",
            Self::EmptyPayload => "ftdi-line-payload-empty",
            Self::OversizedPayload => "ftdi-line-payload-oversized",
            Self::TransferOverflow => "ftdi-line-transfer-overflow",
            Self::TransferTimeout => "ftdi-line-transfer-timeout",
            Self::TransferStall => "ftdi-line-transfer-stall",
            Self::TransferError => "ftdi-line-transfer-error",
            Self::WrongCompletion => "ftdi-line-completion-wrong-trb",
            Self::WrongSlot => "ftdi-line-completion-wrong-slot",
            Self::WrongEndpoint => "ftdi-line-completion-wrong-endpoint",
            Self::DeviceRemoved => "ftdi-line-device-removed",
            Self::InvalidStatus => "ftdi-line-status-invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FtdiLineReady {
    pub interface_number: u8,
    pub input_endpoint_address: u8,
    pub output_endpoint_address: u8,
    pub input_dci: u8,
    pub output_dci: u8,
    pub packet_bytes: u16,
    pub payload_bytes: u16,
    pub transfer_trbs_per_direction: u8,
    dma_physical: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FtdiLineSession {
    ready: FtdiLineReady,
    next_input: usize,
    next_output: usize,
}

#[repr(C, align(4096))]
struct FtdiDma {
    input_context: [u8; 2112],
    input_ring: [[u32; 4]; FTDI_TRANSFER_TRBS],
    output_ring: [[u32; 4]; FTDI_TRANSFER_TRBS],
    input_packet: [u8; FTDI_PACKET_BYTES],
    output_packet: [u8; FTDI_PACKET_BYTES],
}

static mut FTDI_DMA: FtdiDma = FtdiDma {
    input_context: [0; 2112],
    input_ring: [[0; 4]; FTDI_TRANSFER_TRBS],
    output_ring: [[0; 4]; FTDI_TRANSFER_TRBS],
    input_packet: [0; FTDI_PACKET_BYTES],
    output_packet: [0; FTDI_PACKET_BYTES],
};

pub fn prepare_ftdi_line(
    controller: &mut XhciReady,
    device: &UsbDevice,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<FtdiLineReady, FtdiLineError> {
    let (interface, input, output) = match_ftdi(device)?;
    let input_dci = endpoint_dci(input.address)?;
    let output_dci = endpoint_dci(output.address)?;
    let dma_virtual = core::ptr::addr_of_mut!(FTDI_DMA) as u64;
    let dma_physical = image_virtual_to_physical(dma_virtual)
        .filter(|physical| physical & 0xfff == 0)
        .ok_or(FtdiLineError::DmaAddressInvalid)?;
    unsafe {
        FTDI_DMA = FtdiDma {
            input_context: [0; 2112],
            input_ring: [[0; 4]; FTDI_TRANSFER_TRBS],
            output_ring: [[0; 4]; FTDI_TRANSFER_TRBS],
            input_packet: [0; FTDI_PACKET_BYTES],
            output_packet: [0; FTDI_PACKET_BYTES],
        };
    }
    configure_endpoints(
        controller,
        device,
        [(input, input_dci), (output, output_dci)],
        dma_physical,
    )?;
    Ok(FtdiLineReady {
        interface_number: interface.number,
        input_endpoint_address: input.address,
        output_endpoint_address: output.address,
        input_dci,
        output_dci,
        packet_bytes: FTDI_PACKET_BYTES as u16,
        payload_bytes: FTDI_PAYLOAD_BYTES as u16,
        transfer_trbs_per_direction: FTDI_TRANSFER_TRBS as u8,
        dma_physical,
    })
}

pub const fn start_ftdi_line_session(ready: FtdiLineReady) -> FtdiLineSession {
    FtdiLineSession {
        ready,
        next_input: 0,
        next_output: 0,
    }
}

impl FtdiLineSession {
    pub fn send(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        payload: &[u8],
    ) -> Result<(), FtdiLineError> {
        if payload.is_empty() {
            return Err(FtdiLineError::EmptyPayload);
        }
        if payload.len() > FTDI_PACKET_BYTES {
            return Err(FtdiLineError::OversizedPayload);
        }
        let index = self.next_output;
        if index >= FTDI_TRANSFER_TRBS {
            return Err(FtdiLineError::TransferOverflow);
        }
        unsafe {
            FTDI_DMA.output_packet = [0; FTDI_PACKET_BYTES];
            FTDI_DMA.output_packet[..payload.len()].copy_from_slice(payload);
        }
        let sent = submit(
            controller,
            device,
            self.ready,
            index,
            payload.len(),
            TransferDirection::Output,
        )?;
        if sent != payload.len() {
            return Err(FtdiLineError::TransferError);
        }
        self.next_output += 1;
        Ok(())
    }

    pub fn receive(
        &mut self,
        controller: &mut XhciReady,
        device: &UsbDevice,
        output: &mut [u8; FTDI_PAYLOAD_BYTES],
    ) -> Result<usize, FtdiLineError> {
        let index = self.next_input;
        if index >= FTDI_TRANSFER_TRBS {
            return Err(FtdiLineError::TransferOverflow);
        }
        unsafe { FTDI_DMA.input_packet = [0; FTDI_PACKET_BYTES] };
        let received = submit(
            controller,
            device,
            self.ready,
            index,
            FTDI_PACKET_BYTES,
            TransferDirection::Input,
        )?;
        if received < 2 {
            return Err(FtdiLineError::InvalidStatus);
        }
        let packet = unsafe { FTDI_DMA.input_packet };
        if packet[0] & 1 == 0 {
            return Err(FtdiLineError::InvalidStatus);
        }
        let payload = received - 2;
        output[..payload].copy_from_slice(&packet[2..received]);
        self.next_input += 1;
        Ok(payload)
    }
}

fn match_ftdi(
    device: &UsbDevice,
) -> Result<
    (
        super::usb::descriptor::UsbInterface,
        UsbEndpoint,
        UsbEndpoint,
    ),
    FtdiLineError,
> {
    if device.vendor_id != FTDI_VENDOR_ID
        || device.product_id != FTDI_PRODUCT_ID
        || device.device_version != FTDI_DEVICE_VERSION
    {
        return Err(FtdiLineError::WrongDevice);
    }
    let mut matched = None;
    for interface in device.interfaces[..usize::from(device.interface_count)]
        .iter()
        .copied()
        .filter(|interface| {
            interface.class == 0xff
                && interface.subclass == 0xff
                && interface.protocol == 0xff
                && interface.alternate_setting == 0
        })
    {
        if matched.replace(interface).is_some() {
            return Err(FtdiLineError::AmbiguousInterface);
        }
    }
    let interface = matched.ok_or(FtdiLineError::InterfaceAbsent)?;
    let interface_index = device.interfaces[..usize::from(device.interface_count)]
        .iter()
        .position(|candidate| *candidate == interface)
        .ok_or(FtdiLineError::InterfaceAbsent)? as u8;
    let mut input = None;
    let mut output = None;
    for endpoint in device.endpoints[..usize::from(device.endpoint_count)]
        .iter()
        .copied()
        .filter(|endpoint| endpoint.interface_index == interface_index)
    {
        if !matches!(endpoint.address, 0x81 | 0x02) {
            return Err(FtdiLineError::InvalidEndpoint);
        }
        if endpoint.transfer_type != 2 || endpoint.maximum_packet_size != FTDI_PACKET_BYTES as u16 {
            return Err(FtdiLineError::InvalidEndpoint);
        }
        let slot = if endpoint.direction_in {
            &mut input
        } else {
            &mut output
        };
        if slot.replace(endpoint).is_some() {
            return Err(FtdiLineError::AmbiguousEndpoint);
        }
    }
    Ok((
        interface,
        input.ok_or(FtdiLineError::EndpointAbsent)?,
        output.ok_or(FtdiLineError::EndpointAbsent)?,
    ))
}

fn endpoint_dci(address: u8) -> Result<u8, FtdiLineError> {
    let number = address & 0x0f;
    if number == 0 || address & 0x70 != 0 {
        return Err(FtdiLineError::InvalidEndpoint);
    }
    Ok(number * 2 + u8::from(address & 0x80 != 0))
}

fn configure_endpoints(
    controller: &mut XhciReady,
    device: &UsbDevice,
    endpoints: [(UsbEndpoint, u8); 2],
    dma_physical: u64,
) -> Result<(), FtdiLineError> {
    let usb_dma = device_dma_pointer(device).map_err(|_| FtdiLineError::DmaAddressInvalid)?;
    let context = controller.context_bytes();
    let input_physical = dma_physical + core::mem::offset_of!(FtdiDma, input_context) as u64;
    unsafe {
        FTDI_DMA.input_context = [0; 2112];
        write_input_u32(
            4,
            1 | endpoints
                .iter()
                .fold(0_u32, |bits, (_, dci)| bits | (1_u32 << dci)),
        );
        for offset in (0..context).step_by(4) {
            let value = read_volatile(
                core::ptr::addr_of!((*usb_dma).device_context)
                    .cast::<u8>()
                    .add(offset)
                    .cast::<u32>(),
            );
            write_input_u32(context + offset, value);
        }
        let maximum_dci = endpoints.iter().map(|(_, dci)| *dci).max().unwrap_or(1);
        let slot = read_volatile(
            core::ptr::addr_of!(FTDI_DMA.input_context)
                .cast::<u8>()
                .add(context)
                .cast::<u32>(),
        );
        write_input_u32(
            context,
            (slot & !(0x1f << 27)) | (u32::from(maximum_dci) << 27),
        );
        for (endpoint, dci) in endpoints {
            let ring = dma_physical
                + if endpoint.direction_in {
                    core::mem::offset_of!(FtdiDma, input_ring) as u64
                } else {
                    core::mem::offset_of!(FtdiDma, output_ring) as u64
                };
            let ep = context * (usize::from(dci) + 1);
            write_input_u32(ep, 0);
            let endpoint_type = if endpoint.direction_in { 6 } else { 2 };
            write_input_u32(
                ep + 4,
                (3 << 1) | (endpoint_type << 3) | (u32::from(endpoint.maximum_packet_size) << 16),
            );
            write_input_u32(ep + 8, ring as u32 | 1);
            write_input_u32(ep + 12, (ring >> 32) as u32);
            write_input_u32(
                ep + 16,
                u32::from(endpoint.maximum_packet_size)
                    | (u32::from(endpoint.maximum_packet_size) << 16),
            );
        }
    }
    let event = controller
        .command([
            input_physical as u32,
            (input_physical >> 32) as u32,
            0,
            (12 << 10) | (u32::from(device.slot) << 24),
        ])
        .map_err(|_| FtdiLineError::ConfigureEndpointsFailed)?;
    if event.completion_code == 1 && event.slot == device.slot {
        Ok(())
    } else {
        Err(FtdiLineError::ConfigureEndpointsFailed)
    }
}

unsafe fn write_input_u32(offset: usize, value: u32) {
    unsafe {
        write_volatile(
            core::ptr::addr_of_mut!(FTDI_DMA.input_context)
                .cast::<u8>()
                .add(offset)
                .cast::<u32>(),
            value,
        )
    }
}

fn submit(
    controller: &mut XhciReady,
    device: &UsbDevice,
    ready: FtdiLineReady,
    index: usize,
    length: usize,
    direction: TransferDirection,
) -> Result<usize, FtdiLineError> {
    let (dci, ring_offset, buffer_offset) = match direction {
        TransferDirection::Input => (
            ready.input_dci,
            core::mem::offset_of!(FtdiDma, input_ring),
            core::mem::offset_of!(FtdiDma, input_packet),
        ),
        TransferDirection::Output => (
            ready.output_dci,
            core::mem::offset_of!(FtdiDma, output_ring),
            core::mem::offset_of!(FtdiDma, output_packet),
        ),
    };
    let ring = ready.dma_physical + ring_offset as u64;
    let buffer = ready.dma_physical + buffer_offset as u64;
    let trb = ring + (index * 16) as u64;
    let entry = [
        buffer as u32,
        (buffer >> 32) as u32,
        length as u32,
        (1 << 10) | (1 << 5) | 1,
    ];
    unsafe {
        let destination = match direction {
            TransferDirection::Input => core::ptr::addr_of_mut!(FTDI_DMA.input_ring[index]),
            TransferDirection::Output => core::ptr::addr_of_mut!(FTDI_DMA.output_ring[index]),
        };
        write_volatile(destination, entry);
    }
    controller.ring_endpoint(device.slot, dci);
    let mut completed = None;
    for _ in 0..FTDI_POLL_WINDOWS {
        ensure_present(controller.port_status(device.root_port))?;
        match controller.next_event() {
            Ok(event) if event.event_type == 34 => return Err(FtdiLineError::DeviceRemoved),
            Ok(event) => {
                completed = Some(event);
                break;
            }
            Err(XhciError::CommandTimeout) => {}
            Err(_) => return Err(FtdiLineError::TransferError),
        }
    }
    let event = completed.ok_or(FtdiLineError::TransferTimeout)?;
    ensure_present(controller.port_status(device.root_port))?;
    validate_event(event, device.slot, dci, trb, length)
}

#[derive(Clone, Copy)]
enum TransferDirection {
    Input,
    Output,
}

fn ensure_present(port_status: u32) -> Result<(), FtdiLineError> {
    if port_status & 1 == 0 {
        Err(FtdiLineError::DeviceRemoved)
    } else {
        Ok(())
    }
}

fn validate_event(
    event: Event,
    slot: u8,
    endpoint: u8,
    pointer: u64,
    requested: usize,
) -> Result<usize, FtdiLineError> {
    if event.event_type != 32 || event.pointer != pointer {
        return Err(FtdiLineError::WrongCompletion);
    }
    if event.slot != slot {
        return Err(FtdiLineError::WrongSlot);
    }
    if event.endpoint != endpoint {
        return Err(FtdiLineError::WrongEndpoint);
    }
    if usize::try_from(event.residual).unwrap_or(usize::MAX) > requested {
        return Err(FtdiLineError::TransferError);
    }
    match event.completion_code {
        1 | 13 => Ok(requested - event.residual as usize),
        6 => Err(FtdiLineError::TransferStall),
        _ => Err(FtdiLineError::TransferError),
    }
}

#[cfg(test)]
#[path = "ftdi_line_tests.rs"]
mod tests;
