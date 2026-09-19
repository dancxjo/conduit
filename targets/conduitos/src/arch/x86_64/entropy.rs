use core::arch::asm;

use crate::cryptographic_entropy::{CryptographicEntropySource, EntropyProvider, EntropyRefusal};

use super::cpu::feature_basis;

const RDRAND_ATTEMPTS_PER_WORD: usize = 32;
const PROVIDER: EntropyProvider = EntropyProvider {
    base_id: "conduitos/base/cryptographic-entropy/rdrand",
    provider_instance_id: "conduitos/x86_64/cpu-0/rdrand",
    provider_generation: 0,
};

/// Exact x86_64 hardware entropy provider. Construction refuses when CPUID
/// does not advertise RDRAND; generation never falls back to boot entropy.
pub struct RdrandEntropy {
    provider: EntropyProvider,
}

impl RdrandEntropy {
    pub fn detect(provider_generation: u64) -> Result<Self, EntropyRefusal> {
        if !feature_basis().rdrand {
            return Err(EntropyRefusal::Unavailable);
        }
        if provider_generation == 0 {
            return Err(EntropyRefusal::InvalidProvider);
        }
        Ok(Self {
            provider: EntropyProvider {
                provider_generation,
                ..PROVIDER
            },
        })
    }
}

impl CryptographicEntropySource for RdrandEntropy {
    fn provider(&self) -> EntropyProvider {
        self.provider
    }

    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        fill_from_words(output, rdrand_word)
    }
}

fn fill_from_words(
    output: &mut [u8],
    mut next_word: impl FnMut() -> Result<u64, EntropyRefusal>,
) -> Result<(), EntropyRefusal> {
    for chunk in output.chunks_mut(size_of::<u64>()) {
        let bytes = next_word()?.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    Ok(())
}

fn rdrand_word() -> Result<u64, EntropyRefusal> {
    for _ in 0..RDRAND_ATTEMPTS_PER_WORD {
        let candidate: u64;
        let success: u8;
        unsafe {
            asm!(
                "rdrand {candidate}",
                "setc {success}",
                candidate = out(reg) candidate,
                success = out(reg_byte) success,
                options(nostack, nomem)
            );
        }
        if success != 0 {
            return Ok(candidate);
        }
    }
    Err(EntropyRefusal::SourceFailure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_words_and_partial_tail_fill_without_growth() {
        let words = [
            0x0706_0504_0302_0100,
            0x0f0e_0d0c_0b0a_0908,
            0x1716_1514_1312_1110,
        ];
        let mut index = 0;
        let mut output = [0; 19];
        fill_from_words(&mut output, || {
            let word = words[index];
            index += 1;
            Ok(word)
        })
        .unwrap();
        assert_eq!(output, core::array::from_fn(|at| at as u8));
        assert_eq!(index, 3);
    }

    #[test]
    fn word_failure_is_not_replaced_with_weak_material() {
        let mut output = [0; 32];
        assert_eq!(
            fill_from_words(&mut output, || Err(EntropyRefusal::SourceFailure)),
            Err(EntropyRefusal::SourceFailure)
        );
        assert_eq!(output, [0; 32]);
    }
}
