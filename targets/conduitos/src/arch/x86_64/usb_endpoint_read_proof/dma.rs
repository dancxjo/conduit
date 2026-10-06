//! Root-retained proof storage for one ring and eight independent captures.
// The appliance retains the same eight-buffer layout used by native Root.
pub(super) type ProofDma = super::super::capture_dma::CaptureDma<8>;

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
