//! Exact Mini PiTFT physical resource profile. Peripheral commands live in Source.

/// Match the BCM2835 Model B+ v1.2 identity, independently of warranty flags.
/// Known legacy B+ codes are 0010 (Sony) and 0013 (Embest); new-style codes
/// encode model=3, revision=2, processor=0 and 512 MiB, independent of maker.
pub fn is_model_b_plus_v1_2(revision: u32) -> bool {
    let revision = revision & 0x00ff_ffff;
    matches!(revision, 0x0010 | 0x0013) || revision & 0x00f0_ffff == 0x0090_0032
}

#[cfg(target_arch = "arm")]
mod native;
#[cfg(target_arch = "arm")]
pub use native::{AdmissionRefusal, GpioRegisters, MiniPiTftResources, NativeSpi, admit};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_board_revision_refuses_zero_pi2_and_pi3_targets() {
        for revision in [0x0010, 0x0013, 0x900032, 0x1900032, 0x910032] {
            assert!(is_model_b_plus_v1_2(revision));
        }
        for revision in [
            0, 0x000e, 0x0011, 0x900092, 0x9000c1, 0xa020d3, 0xa01041, 0x900031, 0x900132, 0x800032,
        ] {
            assert!(!is_model_b_plus_v1_2(revision));
        }
    }
}
