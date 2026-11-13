//! Finite BCM2835 hardware RNG source for Root capability material.
use super::{read_counter, read32, write32};
use crate::cryptographic_entropy::{CryptographicEntropySource, EntropyProvider, EntropyRefusal};
const BASE: usize = super::PERIPHERAL_BASE + 0x10_4000;
pub struct HardwareEntropy;
impl HardwareEntropy {
    /// Read control/readiness only. Random output words never enter diagnostics.
    pub fn readiness() -> (u32, u32) {
        unsafe { (read32(BASE), read32(BASE + 4)) }
    }
}
impl CryptographicEntropySource for HardwareEntropy {
    fn provider(&self) -> EntropyProvider {
        EntropyProvider {
            base_id: "conduitos/base/cryptographic-entropy/bcm2835",
            provider_instance_id: "conduitos/bcm2835/rng/0",
            provider_generation: 1,
        }
    }
    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        unsafe {
            // Preserve a generator already warmed by firmware. Restarting its
            // discard phase for every key request would make readiness depend
            // on the caller's request cadence.
            if read32(BASE) & 1 == 0 {
                write32(BASE + 4, 0x40000);
                write32(BASE, 1);
            }
        }
        let start = read_counter() as u32;
        for chunk in output.chunks_mut(4) {
            let mut word = None;
            // A separate generous work ceiling complements the two-second
            // timer bound without truncating the hardware warm-up on a fast CPU.
            for _ in 0..16_000_000 {
                if (read_counter() as u32).wrapping_sub(start) > 2_000_000 {
                    break;
                }
                if unsafe { read32(BASE + 4) } >> 24 != 0 {
                    word = Some(unsafe { read32(BASE + 8) });
                    break;
                }
            }
            let bytes = word.ok_or(EntropyRefusal::Unavailable)?.to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}
