//! Root-retained native ring, capture buffers and endpoint configuration storage.
//! Constructing storage does not map it, admit authority or configure hardware.
#![allow(dead_code)] // Ordinary Root installation is being connected.
use crate::usb_base::endpoint_ring::EndpointRingCursor;

// Preserve the proved geometry: the 1 KiB ring and padding place every 2 KiB
// buffer at a 2 KiB boundary for any page-aligned physical allocation.
#[repr(C, align(4096))]
pub(crate) struct CaptureDma<const N: usize> {
    pub ring: [[u32; 4]; 64],
    padding: [u8; 1024],
    pub buffers: [[u8; 2048]; N],
    pub input: [u8; 2112],
    pub cursor: Option<EndpointRingCursor>,
}

impl<const N: usize> CaptureDma<N> {
    /// Root must keep this allocation rooted until acknowledged hardware stop,
    /// including after configuration, binding or Play failures. An unused cursor
    /// is distinct from permission to clear an already configured DMA allocation.
    pub const fn new() -> Self {
        Self {
            ring: [[0; 4]; 64],
            padding: [0; 1024],
            buffers: [[0; 2048]; N],
            input: [0; 2112],
            cursor: None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum CaptureDmaRefusal {
    AlreadyOwned,
    Mapping,
    Geometry(super::endpoint_read::EndpointNativeRefusal),
    Ring(crate::usb_base::endpoint_ring::EndpointRingRefusal),
    Configuration(super::endpoint_setup::EndpointSetupRefusal),
}

impl<const N: usize> CaptureDma<N> {
    /// Map every retained page and configure the exact native capture ring.
    /// Configuration failure retains the cursor and allocation for Root stop.
    ///
    /// # Safety
    /// Root owns this attachment and coherent DMA mapping, independently of
    /// descriptor observations. Storage remains rooted after every error until
    /// an acknowledged hardware stop; Root may not clear/rebind its cursor.
    pub unsafe fn configure(
        &mut self,
        controller: &mut crate::arch::x86_64::xhci::XhciReady,
        device: &super::UsbDevice,
        parameters: super::endpoint_setup::InboundEndpointParameters,
        endpoint_epoch: u64,
        mapping: fn(u64) -> Option<u64>,
    ) -> Result<
        (
            super::endpoint_setup::ConfiguredInboundEndpoint,
            super::endpoint_read::window::EndpointReadWindowDma<'_, N>,
        ),
        CaptureDmaRefusal,
    > {
        if self.cursor.is_some() {
            return Err(CaptureDmaRefusal::AlreadyOwned);
        }
        let physical = mapped_base(
            self as *const Self as u64,
            core::mem::size_of::<Self>(),
            mapping,
        )?;
        let ring_physical = physical + core::mem::offset_of!(Self, ring) as u64;
        let buffers_physical = physical + core::mem::offset_of!(Self, buffers) as u64;
        super::endpoint_read::window::validate_geometry::<N>(ring_physical, buffers_physical)
            .map_err(CaptureDmaRefusal::Geometry)?;
        self.cursor =
            Some(EndpointRingCursor::with_maximum_pending(64, N).map_err(CaptureDmaRefusal::Ring)?);
        let configured = {
            let mut dma = super::endpoint_read::EndpointReceiveDma {
                ring: &mut self.ring,
                buffer: &mut self.buffers[0],
                cursor: self.cursor.as_mut().unwrap(),
                ring_physical,
                buffer_physical: buffers_physical,
            };
            unsafe {
                super::endpoint_setup::configure_inbound(
                    controller,
                    device,
                    parameters,
                    endpoint_epoch,
                    &mut self.input,
                    physical + core::mem::offset_of!(Self, input) as u64,
                    &mut dma,
                )
            }
            .map_err(CaptureDmaRefusal::Configuration)?
        };
        Ok((
            configured,
            super::endpoint_read::window::EndpointReadWindowDma {
                ring: &mut self.ring,
                buffers: &mut self.buffers,
                cursor: self.cursor.as_mut().unwrap(),
                ring_physical,
                buffers_physical,
            },
        ))
    }
}

fn mapped_base(
    virtual_start: u64,
    bytes: usize,
    mapping: impl Fn(u64) -> Option<u64>,
) -> Result<u64, CaptureDmaRefusal> {
    let physical = mapping(virtual_start).ok_or(CaptureDmaRefusal::Mapping)?;
    if bytes == 0
        || bytes % 4096 != 0
        || virtual_start & 4095 != 0
        || physical == 0
        || physical & 4095 != 0
        || virtual_start.checked_add(bytes as u64).is_none()
        || physical.checked_add(bytes as u64).is_none()
    {
        return Err(CaptureDmaRefusal::Mapping);
    }
    for offset in (0..bytes).step_by(4096) {
        if mapping(virtual_start + offset as u64) != Some(physical + offset as u64) {
            return Err(CaptureDmaRefusal::Mapping);
        }
    }
    Ok(physical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate<const N: usize>() {
        use crate::arch::x86_64::usb::endpoint_read::window::validate_geometry;
        let ring = core::mem::offset_of!(CaptureDma<N>, ring) as u64;
        let buffers = core::mem::offset_of!(CaptureDma<N>, buffers) as u64;
        let input = core::mem::offset_of!(CaptureDma<N>, input) as u64;
        assert_eq!(core::mem::align_of::<CaptureDma<N>>(), 4096);
        assert_eq!(buffers, 2048);
        assert!(input >= buffers + (N * 2048) as u64);
        assert_eq!(input % 64, 0);
        for page in 0..16 {
            let base = 0x10000 + page * 4096;
            validate_geometry::<N>(base + ring, base + buffers).unwrap();
            assert!(base + input + 2112 <= base + core::mem::size_of::<CaptureDma<N>>() as u64);
        }
    }

    #[test]
    fn capture_mapping_requires_every_page_and_checked_full_extent() {
        assert_eq!(
            mapped_base(0x20000, 24576, |address| Some(address + 0x40000)).unwrap(),
            0x60000
        );
        for missing in [0, 4096, 8192, 12288, 16384, 20480] {
            assert!(matches!(
                mapped_base(0x20000, 24576, |address| {
                    (address != 0x20000 + missing).then_some(address + 0x40000)
                }),
                Err(CaptureDmaRefusal::Mapping)
            ));
        }
        assert!(
            mapped_base(0x20000, 24576, |address| Some(
                address + if address == 0x22000 { 0x41000 } else { 0x40000 }
            ))
            .is_err()
        );
        for (start, bytes, physical) in [
            (0x20001, 12288, 0x60000),
            (0x20000, 0, 0x60000),
            (0x20000, 12289, 0x60000),
            (0x20000, 12288, 0),
            (0x20000, 12288, 0x60001),
            (u64::MAX & !4095, 12288, 0x60000),
            (0x20000, 12288, u64::MAX & !4095),
        ] {
            assert!(mapped_base(start, bytes, |_| Some(physical)).is_err());
        }
    }

    #[test]
    fn keyboard_and_mouse_storage_fit_every_aligned_physical_base() {
        validate::<8>();
        validate::<2>();
        assert_eq!(core::mem::size_of::<CaptureDma<8>>(), 24576);
        assert_eq!(core::mem::size_of::<CaptureDma<2>>(), 12288);
    }
}
