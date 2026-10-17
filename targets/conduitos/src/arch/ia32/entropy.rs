//! Bounded hardware entropy; no fallback to timestamps or boot identifiers.
use crate::cryptographic_entropy::{CryptographicEntropySource, EntropyProvider, EntropyRefusal};

pub struct RdrandEntropy {
    provider: EntropyProvider,
}
impl RdrandEntropy {
    pub fn detect(provider_generation: u64) -> Result<Self, EntropyRefusal> {
        if core::arch::x86::__cpuid(1).ecx & (1 << 30) == 0 {
            return Err(EntropyRefusal::Unavailable);
        }
        if provider_generation == 0 {
            return Err(EntropyRefusal::InvalidProvider);
        }
        Ok(Self {
            provider: EntropyProvider {
                base_id: "conduitos/base/cryptographic-entropy/rdrand",
                provider_instance_id: "conduitos/ia32/cpu-0/rdrand",
                provider_generation,
            },
        })
    }
}
impl CryptographicEntropySource for RdrandEntropy {
    fn provider(&self) -> EntropyProvider {
        self.provider
    }
    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        for chunk in output.chunks_mut(4) {
            let mut word = None;
            for _ in 0..32 {
                let candidate: u32;
                let success: u8;
                unsafe {
                    core::arch::asm!("rdrand {0:e}", "setc {1}", out(reg) candidate,
                    out(reg_byte) success, options(nostack, nomem));
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
