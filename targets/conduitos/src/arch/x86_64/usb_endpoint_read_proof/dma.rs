//! Root-retained proof storage for one ring and eight independent captures.
use crate::usb_base::endpoint_ring::EndpointRingCursor;

// The ring occupies 1 KiB. Padding places every 2 KiB buffer on a 2 KiB
// boundary, so none can cross a 64 KiB transfer boundary for any aligned base.
#[repr(C, align(4096))]
pub(super) struct ProofDma {
    pub ring: [[u32; 4]; 64],
    padding: [u8; 1024],
    pub buffers: [[u8; 2048]; 8],
    pub input: [u8; 2112],
    pub cursor: Option<EndpointRingCursor>,
}

impl ProofDma {
    pub const fn new() -> Self {
        Self {
            ring: [[0; 4]; 64],
            padding: [0; 1024],
            buffers: [[0; 2048]; 8],
            input: [0; 2112],
            cursor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_capture_storage_fits_every_aligned_physical_base() {
        use crate::arch::x86_64::usb::endpoint_read::window::validate_geometry;
        let ring = core::mem::offset_of!(ProofDma, ring) as u64;
        let buffers = core::mem::offset_of!(ProofDma, buffers) as u64;
        let input = core::mem::offset_of!(ProofDma, input) as u64;
        assert_eq!(core::mem::align_of::<ProofDma>(), 4096);
        assert_eq!(buffers, 2048);
        assert!(input >= buffers + 8 * 2048);
        assert_eq!(input % 64, 0);
        for page in 0..16 {
            let base = 0x10000 + page * 4096;
            validate_geometry::<8>(base + ring, base + buffers).unwrap();
            assert!(base + input + 2112 <= base + core::mem::size_of::<ProofDma>() as u64);
        }
    }
}
