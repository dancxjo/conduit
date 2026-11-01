//! Affine physical custody after capability revocation. This creates readiness,
//! never a new operation grant, old-budget reset, or initialized peripheral.
use super::*;
use sha2::{Digest, Sha256};
pub(crate) struct NativeReturnSeal([u8; 32]);
pub(crate) fn identity_seal(identity: &WaitNativeIdentity, generation: u64) -> NativeReturnSeal {
    let mut hash = Sha256::new();
    hash.update(b"conduit.native/monotonic-wait/ready-continuation@1");
    for field in [
        identity.host_id.as_str(),
        identity.boot_id.as_str(),
        identity.base_id.as_str(),
        identity.provider_instance_id.as_str(),
        identity.resource_pool_id.as_str(),
        identity.resource_generation_id.0.as_str(),
        identity.envelope_id.as_str(),
        identity.artifact_id.as_str(),
        identity.implementation_id.as_str(),
        identity.base_implementation_id.as_str(),
    ] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    hash.update(generation.to_le_bytes());
    NativeReturnSeal(hash.finalize().into())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReattachRefusal {
    NoReadyOrigin,
    GenerationExhausted,
    GenerationMismatch,
    IdentityMismatch,
}
/// Actual revoked, idle returned provider. Only a Ready-origin token can be
/// reattached; a raw unsafe binding remains available only through into_parts.
/// No public fields, Clone, serialization, grant, or bearer survive here.
pub struct RetiredWaitBase<P> {
    provider: P,
    attachment: WaitAttachment,
    identity: Option<NativeReturnSeal>,
    generation: u64,
}
impl<P> RetiredWaitBase<P> {
    pub(crate) fn new(
        provider: P,
        attachment: WaitAttachment,
        identity: Option<NativeReturnSeal>,
        generation: u64,
    ) -> Self {
        Self {
            provider,
            attachment,
            identity,
            generation,
        }
    }
    pub fn provider_generation(&self) -> u64 {
        self.generation
    }
    pub fn attachment(&self) -> &WaitAttachment {
        &self.attachment
    }
    /// Deliberately discards continuation provenance while returning raw custody.
    pub fn into_parts(self) -> (P, WaitAttachment) {
        (self.provider, self.attachment)
    }
    pub fn check_reattach(&self, candidate: &WaitNativeIdentity) -> Result<(), ReattachRefusal> {
        let old = self
            .identity
            .as_ref()
            .ok_or(ReattachRefusal::NoReadyOrigin)?;
        let next = self
            .generation
            .checked_add(1)
            .ok_or(ReattachRefusal::GenerationExhausted)?;
        if candidate.provider_generation != next {
            return Err(ReattachRefusal::GenerationMismatch);
        }
        let expected = identity_seal(candidate, self.generation);
        // Full256bit identity comparison, separate from operation bearer authority.
        let difference = old
            .0
            .iter()
            .zip(expected.0)
            .fold(0u8, |v, (a, b)| v | (a ^ b));
        if difference != 0 {
            return Err(ReattachRefusal::IdentityMismatch);
        }
        Ok(())
    }
}
impl<P: MonotonicWaitProvider> RetiredWaitBase<P> {
    #[allow(clippy::result_large_err)] // No allocation and no physical owner loss on refusal.
    pub fn reattach(
        self,
        candidate: WaitNativeIdentity,
    ) -> Result<ReadyMonotonicWaitBase<P>, (Self, WaitNativeIdentity, ReattachRefusal)> {
        if let Err(error) = self.check_reattach(&candidate) {
            return Err((self, candidate, error));
        }
        Ok(ReadyMonotonicWaitBase {
            identity: candidate,
            attachment: self.attachment,
            provider: self.provider,
        })
    }
}
