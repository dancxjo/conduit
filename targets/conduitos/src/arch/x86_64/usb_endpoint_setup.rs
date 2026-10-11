//! Native endpoint-context realization; no descriptor parsing or class policy.
#![allow(dead_code)] // Root installation and controller proof remain pending.
use super::endpoint_read::EndpointReceiveDma;
use super::{UsbDevice, dma::device_dma_pointer};
use crate::arch::x86_64::xhci::XhciReady;
use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::{Ordering, fence},
};

/// Advisory descriptor fields supplied by reviewed Source. Root must separately
/// authorize this exact attachment and configuration; these data grant nothing.
#[derive(Clone, Copy)]
pub(crate) struct InboundEndpointParameters {
    pub address: u8,
    pub transfer_type: u8,
    pub packet_field: u16,
    pub interval: u8,
}
/// OUT uses the same advisory descriptor fields, without inbound-only policy.
pub(crate) type OutboundEndpointParameters = InboundEndpointParameters;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum EndpointSetupRefusal {
    Parameters,
    Unsupported,
    Mapping,
    Active,
    Command,
}

/// Move-only receipt of an acknowledged Configure Endpoint command. It cannot
/// be reconstructed from a descriptor, serialized grant or matching identifiers.
pub(crate) struct ConfiguredInboundEndpoint {
    pub(super) slot: u8,
    pub(super) device_epoch: u32,
    pub(super) endpoint_epoch: u64,
    pub(super) dci: u8,
    pub(super) ring_physical: u64,
}

/// Move-only receipt for an acknowledged outbound endpoint configuration.
pub(crate) struct ConfiguredOutboundEndpoint {
    pub(super) slot: u8,
    pub(super) device_epoch: u32,
    pub(super) endpoint_epoch: u64,
    pub(super) dci: u8,
    pub(super) ring_physical: u64,
}

// Interval basis: https://github.com/torvalds/linux/blob/master/drivers/usb/host/xhci-mem.c
// Full/low-speed frame intervals are realized as powers of two microframes.
#[cfg(test)]
fn context_words(
    parameters: InboundEndpointParameters,
    speed: u8,
    ring: u64,
) -> Result<(u8, [u32; 5]), EndpointSetupRefusal> {
    if parameters.address & 0x80 == 0 {
        return Err(EndpointSetupRefusal::Parameters);
    }
    context_words_transport(parameters, speed, ring)
}

fn context_words_transport(
    parameters: InboundEndpointParameters,
    speed: u8,
    ring: u64,
) -> Result<(u8, [u32; 5]), EndpointSetupRefusal> {
    use EndpointSetupRefusal as Error;
    if parameters.address & 0x70 != 0 || parameters.address & 15 == 0 || ring == 0 || ring & 63 != 0
    {
        return Err(Error::Parameters);
    }
    if parameters.packet_field & 0xf800 != 0 {
        return Err(Error::Unsupported);
    }
    let packet = parameters.packet_field;
    let (endpoint_type, interval, payload) = match parameters.transfer_type {
        3 => {
            if parameters.interval == 0 || packet == 0 {
                return Err(Error::Parameters);
            }
            let interval = match speed {
                1 if packet <= 64 => 7 - parameters.interval.leading_zeros() as u8 + 3,
                2 if packet <= 8 => 7 - parameters.interval.leading_zeros() as u8 + 3,
                3 if packet <= 1024 && parameters.interval <= 16 => parameters.interval - 1,
                1..=3 => return Err(Error::Parameters),
                _ => return Err(Error::Unsupported),
            };
            (
                if parameters.address & 0x80 != 0 { 7 } else { 3 },
                interval,
                packet,
            )
        }
        2 => {
            match speed {
                1 if matches!(packet, 8 | 16 | 32 | 64) => {}
                3 if packet == 512 => {}
                1 | 3 => return Err(Error::Parameters),
                _ => return Err(Error::Unsupported),
            }
            let interval = if speed == 3 && parameters.interval != 0 {
                7 - parameters.interval.leading_zeros() as u8
            } else {
                0
            };
            (
                if parameters.address & 0x80 != 0 { 6 } else { 2 },
                interval,
                0,
            )
        }
        _ => return Err(Error::Unsupported),
    };
    let dci = ((parameters.address & 15) << 1) | u8::from(parameters.address & 0x80 != 0);
    Ok((
        dci,
        [
            u32::from(interval) << 16,
            (3 << 1) | (endpoint_type << 3) | (u32::from(packet) << 16),
            ring as u32 | 1,
            (ring >> 32) as u32,
            u32::from(packet) | (u32::from(payload) << 16),
        ],
    ))
}

// The actual output Slot Context may already include a higher DCI. Adding a
// lower OUT endpoint must not shrink Context Entries and hide a configured IN.
fn slot_with_added_endpoint(slot: u32, dci: u8) -> u32 {
    let entries = ((slot >> 27) & 31).max(u32::from(dci));
    (slot & !(31 << 27)) | (entries << 27)
}

fn validate_dma_regions(input: u64, ring: u64, buffer: u64) -> Result<(), EndpointSetupRefusal> {
    let regions = [(input, 2112_u64), (ring, 1024), (buffer, 2048)];
    if input == 0
        || input & 63 != 0
        || ring == 0
        || ring & 63 != 0
        || buffer == 0
        || ring & 0xffff > 0xfc00
        || buffer & 0xffff > 0xf800
    {
        return Err(EndpointSetupRefusal::Mapping);
    }
    for (index, (start, bytes)) in regions.iter().copied().enumerate() {
        let end = start
            .checked_add(bytes)
            .ok_or(EndpointSetupRefusal::Mapping)?;
        for (other, size) in regions.iter().copied().skip(index + 1) {
            let other_end = other
                .checked_add(size)
                .ok_or(EndpointSetupRefusal::Mapping)?;
            if start < other_end && other < end {
                return Err(EndpointSetupRefusal::Mapping);
            }
        }
    }
    Ok(())
}

/// # Safety
/// Root owns and authorizes this controller/device and unconfigured endpoint.
/// The input context, ring and buffer are stable coherent DMA with these exact
/// physical addresses, not already owned by another transfer. Failure/timeout
/// retains all storage until acknowledged controller/slot stop. Source fields
/// are advisory; invoking this function is an independent Root authority act.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn configure_inbound(
    controller: &mut XhciReady,
    device: &UsbDevice,
    parameters: InboundEndpointParameters,
    endpoint_epoch: u64,
    input: &mut [u8; 2112],
    input_physical: u64,
    dma: &mut EndpointReceiveDma<'_>,
) -> Result<ConfiguredInboundEndpoint, EndpointSetupRefusal> {
    if parameters.address & 0x80 == 0 {
        return Err(EndpointSetupRefusal::Parameters);
    }
    unsafe {
        configure_transport(
            controller,
            device,
            parameters,
            endpoint_epoch,
            input,
            input_physical,
            dma,
        )
    }
}

/// # Safety
/// Root retains exclusively owned coherent OUT DMA until acknowledged stop.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn configure_outbound(
    controller: &mut XhciReady,
    device: &UsbDevice,
    parameters: OutboundEndpointParameters,
    endpoint_epoch: u64,
    input: &mut [u8; 2112],
    input_physical: u64,
    dma: &mut super::endpoint_write_transfer::EndpointTransmitDma<'_>,
) -> Result<ConfiguredOutboundEndpoint, EndpointSetupRefusal> {
    if parameters.address & 0x80 != 0 || parameters.transfer_type != 2 {
        return Err(EndpointSetupRefusal::Parameters);
    }
    let mut transport = EndpointReceiveDma {
        ring: &mut *dma.ring,
        buffer: &mut *dma.buffer,
        cursor: &mut *dma.cursor,
        ring_physical: dma.ring_physical,
        buffer_physical: dma.buffer_physical,
    };
    let configured = unsafe {
        configure_transport(
            controller,
            device,
            parameters,
            endpoint_epoch,
            input,
            input_physical,
            &mut transport,
        )
    }?;
    Ok(ConfiguredOutboundEndpoint {
        slot: configured.slot,
        device_epoch: configured.device_epoch,
        endpoint_epoch: configured.endpoint_epoch,
        dci: configured.dci,
        ring_physical: configured.ring_physical,
    })
}

#[allow(clippy::too_many_arguments)]
unsafe fn configure_transport(
    controller: &mut XhciReady,
    device: &UsbDevice,
    parameters: InboundEndpointParameters,
    endpoint_epoch: u64,
    input: &mut [u8; 2112],
    input_physical: u64,
    dma: &mut EndpointReceiveDma<'_>,
) -> Result<ConfiguredInboundEndpoint, EndpointSetupRefusal> {
    use EndpointSetupRefusal as Error;
    if input.as_ptr() as usize & 63 != 0
        || endpoint_epoch == 0
        || input_physical == 0
        || input_physical & 63 != 0
        || input_physical.checked_add(2112).is_none()
        || dma.cursor.ordinary_slots() != 63
    {
        return Err(Error::Mapping);
    }
    validate_dma_regions(input_physical, dma.ring_physical, dma.buffer_physical)?;
    dma.cursor.ensure_ready().map_err(|_| Error::Active)?;
    // Fresh configuration must not reinterpret an old ring's producer position.
    if !dma.cursor.is_fresh() {
        return Err(Error::Active);
    }
    let native = device_dma_pointer(device).map_err(|_| Error::Mapping)?;
    let context = controller.context_bytes();
    if !matches!(context, 32 | 64) {
        return Err(Error::Unsupported);
    }
    let slot =
        unsafe { read_volatile(core::ptr::addr_of!((*native).device_context).cast::<u32>()) };
    let speed = ((slot >> 20) & 15) as u8;
    let (dci, words) = context_words_transport(parameters, speed, dma.ring_physical)?;
    let endpoint_offset = usize::from(dci) * context;
    let endpoint_state = unsafe {
        read_volatile(
            core::ptr::addr_of!((*native).device_context)
                .cast::<u8>()
                .add(endpoint_offset)
                .cast::<u32>(),
        )
    } & 7;
    if endpoint_state != 0 {
        return Err(Error::Active);
    }
    input.fill(0);
    for trb in dma.ring.iter_mut() {
        for word in trb {
            unsafe { write_volatile(word, 0) };
        }
    }
    let write = |input: &mut [u8; 2112], offset: usize, value: u32| unsafe {
        write_volatile(input.as_mut_ptr().add(offset).cast::<u32>(), value)
    };
    write(input, 4, 1 | (1 << dci));
    for offset in (0..context).step_by(4) {
        let value = unsafe {
            read_volatile(
                core::ptr::addr_of!((*native).device_context)
                    .cast::<u8>()
                    .add(offset)
                    .cast::<u32>(),
            )
        };
        write(input, context + offset, value);
    }
    write(input, context, slot_with_added_endpoint(slot, dci));
    for (word, value) in words.into_iter().enumerate() {
        write(input, context * (usize::from(dci) + 1) + word * 4, value);
    }
    fence(Ordering::Release);
    dma.cursor
        .begin_configuration()
        .map_err(|_| Error::Active)?;
    let event = controller
        .command([
            input_physical as u32,
            (input_physical >> 32) as u32,
            0,
            (12 << 10) | (u32::from(device.slot) << 24),
        ])
        .map_err(|_| Error::Command)?;
    if event.completion_code != 1 || event.slot != device.slot {
        return Err(Error::Command);
    }
    unsafe { dma.cursor.complete_configuration() }.map_err(|_| Error::Active)?;
    Ok(ConfiguredInboundEndpoint {
        slot: device.slot,
        device_epoch: device.attachment_epoch,
        endpoint_epoch,
        dci,
        ring_physical: dma.ring_physical,
    })
}

#[cfg(test)]
#[path = "usb_endpoint_setup/tests.rs"]
mod tests;

#[cfg(test)]
mod outbound_tests {
    use super::*;
    #[test]
    fn adding_either_direction_preserves_existing_slot_context_entries() {
        let other_fields = (3 << 20) | (1 << 26) | 0x1234;
        for (existing, next, expected) in [(3, 2, 3), (2, 3, 3), (7, 2, 7), (1, 30, 30)] {
            let slot = other_fields | (existing << 27);
            let updated = slot_with_added_endpoint(slot, next);
            assert_eq!((updated >> 27) & 31, expected);
            assert_eq!(updated & !(31 << 27), other_fields);
        }
    }

    #[test]
    fn bulk_out_configuration_has_even_dci_and_output_endpoint_type() {
        for (speed, packet) in [(1, 8), (1, 64), (3, 512)] {
            let parameters = InboundEndpointParameters {
                address: 0x02,
                transfer_type: 2,
                packet_field: packet,
                interval: 0,
            };
            let (dci, words) = context_words_transport(parameters, speed, 0x4000).unwrap();
            assert_eq!(dci, 4);
            assert_eq!((words[1] >> 3) & 7, 2);
            assert_eq!(words[1] >> 16, u32::from(packet));
            assert!(context_words(parameters, speed, 0x4000).is_err());
        }
    }
}
