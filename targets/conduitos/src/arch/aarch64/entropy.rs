//! Bounded architectural random source for Root's opaque capability material.
use crate::cryptographic_entropy::{CryptographicEntropySource, EntropyProvider, EntropyRefusal};

pub struct RndrEntropy {
    provider: EntropyProvider,
}
impl RndrEntropy {
    pub fn detect(provider_generation: u64) -> Result<Self, EntropyRefusal> {
        let features: u64;
        unsafe {
            core::arch::asm!("mrs {0}, id_aa64isar0_el1", out(reg) features, options(nostack));
        }
        if features >> 60 == 0 {
            return Err(EntropyRefusal::Unavailable);
        }
        if provider_generation == 0 {
            return Err(EntropyRefusal::InvalidProvider);
        }
        Ok(Self {
            provider: EntropyProvider {
                base_id: "conduitos/base/cryptographic-entropy/rndr",
                provider_instance_id: "conduitos/aarch64/cpu-0/rndr",
                provider_generation,
            },
        })
    }
}
impl CryptographicEntropySource for RndrEntropy {
    fn provider(&self) -> EntropyProvider {
        self.provider
    }
    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        for chunk in output.chunks_mut(8) {
            let mut word = None;
            for _ in 0..32 {
                let candidate: u64;
                let success: u64;
                unsafe {
                    // RNDR's Z flag reports whether fresh output was available.
                    core::arch::asm!("mrs {0}, S3_3_C2_C4_0", "cset {1}, ne",
                        out(reg) candidate, out(reg) success, options(nostack, nomem));
                }
                if success != 0 {
                    word = Some(candidate);
                    break;
                }
            }
            let bytes = word.ok_or(EntropyRefusal::SourceFailure)?.to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}
