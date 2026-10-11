//! Root-retained endpoint DMA and exact administrative native configuration.
use super::super::usb_request::UsbEndpointBootRequest;
use crate::{
    arch, boot,
    usb_base::{
        endpoint_read_contract::ENDPOINT_READ_DATA_BYTES,
        endpoint_read_owner::EndpointReadAttachment, endpoint_ring::EndpointRingCursor,
    },
};

#[repr(C, align(4096))]
struct EndpointDma {
    ring: [[u32; 4]; 64],
    padding: [u8; 1024],
    buffer: [u8; 2048],
    input: [u8; 2112],
    cursor: Option<EndpointRingCursor>,
}
static mut ENDPOINT_DMA: EndpointDma = EndpointDma {
    ring: [[0; 4]; 64],
    padding: [0; 1024],
    buffer: [0; 2048],
    input: [0; 2112],
    cursor: None,
};

pub(super) struct NativeEndpoint {
    pub configured: arch::UsbConfiguredInboundEndpoint,
    pub dma: arch::UsbEndpointReceiveDma<'static>,
    pub attachment: EndpointReadAttachment,
}

/// # Safety
/// Sole Root already reserved the original native attachment/controller and
/// independently approved these exact endpoint parameters. Static storage is
/// quarantined forever after first admission, including failed configuration.
pub(super) unsafe fn configure(
    controller: &mut arch::XhciReady,
    device: &arch::UsbDevice,
    request: &UsbEndpointBootRequest,
) -> Result<NativeEndpoint, &'static str> {
    let pointer = core::ptr::addr_of_mut!(ENDPOINT_DMA);
    let start = pointer as u64;
    let bytes = core::mem::size_of::<EndpointDma>() as u64;
    let physical =
        boot::executable_physical_address(start).ok_or("protocol-usb-endpoint-dma-mapping")?;
    let last = start
        .checked_add(bytes - 1)
        .ok_or("protocol-usb-endpoint-dma-mapping")?;
    let physical_last = physical
        .checked_add(bytes - 1)
        .ok_or("protocol-usb-endpoint-dma-mapping")?;
    if physical & 4095 != 0
        || boot::executable_physical_address(last) != Some(physical_last)
        || (0..bytes).step_by(4096).any(|offset| {
            boot::executable_physical_address(start + offset) != Some(physical + offset)
        })
    {
        return Err("protocol-usb-endpoint-dma-mapping");
    }
    // CLAIMED forbids a second admission; cursor also preserves local quarantine.
    let storage = unsafe { &mut *pointer };
    if storage.cursor.is_some() {
        return Err("protocol-usb-endpoint-already-owned");
    }
    storage.cursor = Some(EndpointRingCursor::new(64).map_err(|_| "protocol-usb-endpoint-ring")?);
    let mut dma = arch::UsbEndpointReceiveDma {
        ring: &mut storage.ring,
        buffer: &mut storage.buffer,
        cursor: storage
            .cursor
            .as_mut()
            .ok_or("protocol-usb-endpoint-ring")?,
        ring_physical: physical + core::mem::offset_of!(EndpointDma, ring) as u64,
        buffer_physical: physical + core::mem::offset_of!(EndpointDma, buffer) as u64,
    };
    // Only controller geometry is realized here. Source retains every USB class
    // request and payload; descriptors never substitute administrative authority.
    let configured = unsafe {
        arch::configure_usb_inbound(
            controller,
            device,
            arch::UsbInboundEndpointParameters {
                address: request.address,
                transfer_type: request.transfer_type,
                packet_field: request.maximum_packet,
                interval: request.interval,
            },
            u64::from(request.endpoint_epoch),
            &mut storage.input,
            physical + core::mem::offset_of!(EndpointDma, input) as u64,
            &mut dma,
        )
    }
    .map_err(|_| "protocol-usb-endpoint-configuration-refused")?;
    Ok(NativeEndpoint {
        configured,
        dma,
        attachment: EndpointReadAttachment {
            slot: device.slot,
            generation: u64::from(device.attachment_epoch),
            endpoint_generation: u64::from(request.endpoint_epoch),
            endpoint_dci: ((request.address & 15) << 1) | 1,
            maximum_data_bytes: ENDPOINT_READ_DATA_BYTES,
            resource_bytes: bytes,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_endpoint_storage_is_separate_and_fits_native_dma_boundaries() {
        assert_eq!(core::mem::align_of::<EndpointDma>(), 4096);
        assert_eq!(core::mem::offset_of!(EndpointDma, ring), 0);
        assert_eq!(core::mem::offset_of!(EndpointDma, buffer), 2048);
        assert_eq!(core::mem::offset_of!(EndpointDma, input), 4096);
        assert!(core::mem::size_of::<EndpointDma>() >= 4096 + 2112);
        for page in 0..16 {
            let physical = 0x10000 + page * 4096;
            assert!((physical & 0xffff) <= 0xfc00);
            assert!(((physical + 2048) & 0xffff) <= 0xf800);
        }
    }
}
