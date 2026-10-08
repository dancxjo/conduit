//! Zkr's ES16 samples require 2:1 cryptographic conditioning.
//!
//! RISC-V scalar crypto, Entropy Source Requirements: each 256-bit input
//! block can yield 128 full-entropy bits through a vetted hash. Never treat
//! WAIT/BIST/DEAD status or custom CSR bits as entropy.
use crate::cryptographic_entropy::{EntropyRefusal, MAXIMUM_SECRET_BYTES};
use sha2::{Digest, Sha256};

const POLLS_PER_BLOCK: usize = 1024;

pub(super) fn fill(
    output: &mut [u8],
    mut read: impl FnMut() -> Result<u64, EntropyRefusal>,
) -> Result<(), EntropyRefusal> {
    output.fill(0);
    if output.is_empty() {
        return Err(EntropyRefusal::EmptyBuffer);
    }
    if output.len() > MAXIMUM_SECRET_BYTES {
        return Err(EntropyRefusal::BufferTooLarge);
    }
    let mut result = [0_u8; MAXIMUM_SECRET_BYTES];
    for chunk in result[..output.len()].chunks_mut(16) {
        let mut samples = [0_u8; 32];
        let mut collected = 0;
        for _ in 0..POLLS_PER_BLOCK {
            let value = read()?;
            match (value >> 30) & 3 {
                0 => {
                    // A new self-test alarm discards this partial block.
                    samples.fill(0);
                    collected = 0;
                }
                1 => {}
                2 => {
                    samples[collected..collected + 2]
                        .copy_from_slice(&(value as u16).to_le_bytes());
                    collected += 2;
                    if collected == samples.len() {
                        break;
                    }
                }
                _ => return Err(EntropyRefusal::SourceFailure),
            }
        }
        if collected != samples.len() {
            return Err(EntropyRefusal::SourceFailure);
        }
        let digest = Sha256::digest(samples);
        chunk.copy_from_slice(&digest[..chunk.len()]);
    }
    output.copy_from_slice(&result[..output.len()]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_only_successful_samples_and_ignores_custom_bits() {
        let mut calls = 0;
        let mut output = [0; 16];
        fill(&mut output, || {
            calls += 1;
            Ok(if calls % 2 == 1 {
                0x4000_ffff // WAIT's low bits are never consumed.
            } else {
                0xffff_ffff_bfff_0000 | (calls / 2 - 1)
            })
        })
        .unwrap();
        let samples: [u8; 32] =
            core::array::from_fn(|i| if i % 2 == 0 { (i / 2) as u8 } else { 0 });
        assert_eq!(output, Sha256::digest(samples)[..16]);
        assert_eq!(calls, 32);
    }

    #[test]
    fn unavailable_dead_and_stalled_sources_leave_no_partial_output() {
        for failure in [None, Some(0x4000_0000), Some(0xc000_ffff)] {
            let mut calls = 0;
            let mut output = [0xaa; 32];
            let result = fill(&mut output, || {
                calls += 1;
                if calls <= 16 {
                    Ok(0x8000_0000 | calls)
                } else {
                    failure.ok_or(EntropyRefusal::Unavailable)
                }
            });
            assert!(result.is_err());
            assert_eq!(output, [0; 32]);
            assert!(calls <= 16 + POLLS_PER_BLOCK as u64);
        }
    }

    #[test]
    fn self_test_discards_partial_samples_and_requests_are_bounded() {
        let mut calls = 0;
        let mut output = [0; 16];
        fill(&mut output, || {
            calls += 1;
            Ok(match calls {
                1..=8 => 0x8000_ffff,
                9 => 0,
                _ => 0x8000_1234,
            })
        })
        .unwrap();
        let samples: [u8; 32] = core::array::from_fn(|i| if i % 2 == 0 { 0x34 } else { 0x12 });
        assert_eq!(output, Sha256::digest(samples)[..16]);
        assert_eq!(calls, 25);
        assert_eq!(fill(&mut [], || panic!()), Err(EntropyRefusal::EmptyBuffer));
        assert_eq!(
            fill(&mut [0; MAXIMUM_SECRET_BYTES + 1], || panic!()),
            Err(EntropyRefusal::BufferTooLarge)
        );
    }
}
