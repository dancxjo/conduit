//! Finite Host authority for cryptographic key material.
//!
//! Boot identity entropy is deliberately not accepted here. A provider must be
//! explicitly admitted, remain at the admitted generation, and fill the exact
//! caller-owned buffer without a weak fallback.

pub const MAXIMUM_SECRET_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntropyProvider {
    pub base_id: &'static str,
    pub provider_instance_id: &'static str,
    pub provider_generation: u64,
}

impl EntropyProvider {
    const fn is_valid(self) -> bool {
        !self.base_id.is_empty()
            && !self.provider_instance_id.is_empty()
            && self.provider_generation != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntropyRefusal {
    InvalidProvider,
    Unavailable,
    EmptyBuffer,
    BufferTooLarge,
    RequestCapacity,
    StaleProvider,
    SourceFailure,
    WeakOutput,
}

impl EntropyRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidProvider => "cryptographic-entropy-provider-invalid",
            Self::Unavailable => "cryptographic-entropy-unavailable",
            Self::EmptyBuffer => "cryptographic-entropy-buffer-empty",
            Self::BufferTooLarge => "cryptographic-entropy-buffer-too-large",
            Self::RequestCapacity => "cryptographic-entropy-request-capacity",
            Self::StaleProvider => "cryptographic-entropy-provider-stale",
            Self::SourceFailure => "cryptographic-entropy-source-failed",
            Self::WeakOutput => "cryptographic-entropy-weak-output",
        }
    }
}

/// Target-owned primitive beneath the admitted Host Base.
///
/// Implementations must either fill the entire buffer from their reviewed
/// cryptographic source or return an error. They must never substitute boot
/// identity material, clocks, addresses, or deterministic fixture bytes.
pub trait CryptographicEntropySource {
    fn provider(&self) -> EntropyProvider;
    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntropyReceipt {
    pub provider: EntropyProvider,
    pub request_index: u32,
    pub byte_count: u8,
}

/// An admitted, finite capability. `REQUESTS` is the complete lifetime budget;
/// exhaustion refuses rather than silently widening authority.
pub struct CryptographicEntropyBase<S, const REQUESTS: u32> {
    source: S,
    admitted_provider: EntropyProvider,
    requests: u32,
}

impl<S: CryptographicEntropySource, const REQUESTS: u32> CryptographicEntropyBase<S, REQUESTS> {
    pub fn admit(source: S) -> Result<Self, EntropyRefusal> {
        let provider = source.provider();
        if !provider.is_valid() || REQUESTS == 0 {
            return Err(EntropyRefusal::InvalidProvider);
        }
        Ok(Self {
            source,
            admitted_provider: provider,
            requests: 0,
        })
    }

    pub const fn provider(&self) -> EntropyProvider {
        self.admitted_provider
    }

    pub const fn requests_used(&self) -> u32 {
        self.requests
    }

    /// Fill an exact caller-owned key buffer. The buffer is cleared before the
    /// attempt and again on every refusal. The caller owns erasure after a
    /// successful transfer; prefer `with_secret` when scoped use is possible.
    pub fn fill(&mut self, output: &mut [u8]) -> Result<EntropyReceipt, EntropyRefusal> {
        erase(output);
        if output.is_empty() {
            return Err(EntropyRefusal::EmptyBuffer);
        }
        if output.len() > MAXIMUM_SECRET_BYTES {
            return Err(EntropyRefusal::BufferTooLarge);
        }
        if self.source.provider() != self.admitted_provider {
            return Err(EntropyRefusal::StaleProvider);
        }
        let request_index = self
            .requests
            .checked_add(1)
            .filter(|next| *next <= REQUESTS)
            .ok_or(EntropyRefusal::RequestCapacity)?;
        // An admitted attempt consumes capacity even when the hardware source
        // fails. Repeated failure cannot become unbounded hidden work.
        self.requests = request_index;
        if let Err(error) = self.source.fill_exact(output) {
            erase(output);
            return Err(error);
        }
        if output.iter().all(|byte| *byte == 0) {
            erase(output);
            return Err(EntropyRefusal::WeakOutput);
        }
        Ok(EntropyReceipt {
            provider: self.admitted_provider,
            request_index,
            byte_count: output.len() as u8,
        })
    }

    /// Supply a fixed, stack-owned secret only for the duration of `use_secret`
    /// and erase it with volatile writes before returning.
    pub fn with_secret<const BYTES: usize, T>(
        &mut self,
        use_secret: impl FnOnce(&[u8; BYTES], EntropyReceipt) -> T,
    ) -> Result<T, EntropyRefusal> {
        let mut secret = ScopedSecret::<BYTES>::new();
        let receipt = self.fill(&mut secret.bytes)?;
        Ok(use_secret(&secret.bytes, receipt))
    }
}

struct ScopedSecret<const BYTES: usize> {
    bytes: [u8; BYTES],
}

impl<const BYTES: usize> ScopedSecret<BYTES> {
    const fn new() -> Self {
        Self { bytes: [0; BYTES] }
    }
}

impl<const BYTES: usize> Drop for ScopedSecret<BYTES> {
    fn drop(&mut self) {
        erase(&mut self.bytes);
    }
}

fn erase(bytes: &mut [u8]) {
    for byte in bytes {
        // Volatile stores keep secret erasure observable to the machine even
        // when the buffer is otherwise dead after this point.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixtureSource {
        provider: EntropyProvider,
        result: Result<u8, EntropyRefusal>,
    }

    impl CryptographicEntropySource for FixtureSource {
        fn provider(&self) -> EntropyProvider {
            self.provider
        }

        fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
            let byte = self.result?;
            output.fill(byte);
            Ok(())
        }
    }

    const PROVIDER: EntropyProvider = EntropyProvider {
        base_id: "base/entropy/rdrand",
        provider_instance_id: "cpu/0/rdrand",
        provider_generation: 7,
    };

    #[test]
    fn admission_and_request_capacity_are_exact() {
        let source = FixtureSource {
            provider: PROVIDER,
            result: Ok(0x5a),
        };
        let mut base = CryptographicEntropyBase::<_, 1>::admit(source).unwrap();
        let mut key = [0; 32];
        let receipt = base.fill(&mut key).unwrap();
        assert_eq!(key, [0x5a; 32]);
        assert_eq!(receipt.provider, PROVIDER);
        assert_eq!(receipt.request_index, 1);
        assert_eq!(receipt.byte_count, 32);

        key.fill(0xff);
        assert_eq!(base.fill(&mut key), Err(EntropyRefusal::RequestCapacity));
        assert_eq!(key, [0; 32]);
    }

    #[test]
    fn invalid_bounds_failure_and_weak_output_leave_no_bytes() {
        let mut empty = [];
        let source = FixtureSource {
            provider: PROVIDER,
            result: Ok(1),
        };
        let mut base = CryptographicEntropyBase::<_, 3>::admit(source).unwrap();
        assert_eq!(base.fill(&mut empty), Err(EntropyRefusal::EmptyBuffer));

        let mut oversized = [9; MAXIMUM_SECRET_BYTES + 1];
        assert_eq!(
            base.fill(&mut oversized),
            Err(EntropyRefusal::BufferTooLarge)
        );
        assert!(oversized.iter().all(|byte| *byte == 0));

        let source = FixtureSource {
            provider: PROVIDER,
            result: Err(EntropyRefusal::SourceFailure),
        };
        let mut base = CryptographicEntropyBase::<_, 1>::admit(source).unwrap();
        let mut key = [9; 32];
        assert_eq!(base.fill(&mut key), Err(EntropyRefusal::SourceFailure));
        assert_eq!(key, [0; 32]);
        assert_eq!(base.requests_used(), 1);
        assert_eq!(base.fill(&mut key), Err(EntropyRefusal::RequestCapacity));

        let source = FixtureSource {
            provider: PROVIDER,
            result: Ok(0),
        };
        let mut base = CryptographicEntropyBase::<_, 1>::admit(source).unwrap();
        key.fill(9);
        assert_eq!(base.fill(&mut key), Err(EntropyRefusal::WeakOutput));
        assert_eq!(key, [0; 32]);
    }

    #[test]
    fn changed_generation_refuses_before_source_use() {
        struct ChangingSource(u64);
        impl CryptographicEntropySource for ChangingSource {
            fn provider(&self) -> EntropyProvider {
                EntropyProvider {
                    provider_generation: self.0,
                    ..PROVIDER
                }
            }

            fn fill_exact(&mut self, _: &mut [u8]) -> Result<(), EntropyRefusal> {
                panic!("stale provider must refuse before use")
            }
        }

        let mut base = CryptographicEntropyBase::<_, 1>::admit(ChangingSource(7)).unwrap();
        base.source.0 = 8;
        let mut key = [9; 32];
        assert_eq!(base.fill(&mut key), Err(EntropyRefusal::StaleProvider));
        assert_eq!(key, [0; 32]);
    }

    #[test]
    fn scoped_secret_is_available_only_inside_the_callback() {
        let source = FixtureSource {
            provider: PROVIDER,
            result: Ok(0xa5),
        };
        let mut base = CryptographicEntropyBase::<_, 1>::admit(source).unwrap();
        let digest = base
            .with_secret::<32, _>(|secret, receipt| {
                assert_eq!(receipt.byte_count, 32);
                secret.iter().fold(0_u8, |sum, byte| sum ^ byte)
            })
            .unwrap();
        assert_eq!(digest, 0);
    }
}
