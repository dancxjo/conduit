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
    fn keyboard_and_mouse_storage_fit_every_aligned_physical_base() {
        validate::<8>();
        validate::<2>();
        assert_eq!(core::mem::size_of::<CaptureDma<8>>(), 24576);
        assert_eq!(core::mem::size_of::<CaptureDma<2>>(), 12288);
    }
}
