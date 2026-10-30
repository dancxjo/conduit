use alloc::string::String;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::BodyLifecycleError;

pub const MAX_LIFECYCLE_ID_BYTES: usize = 256;

macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub(crate) fn bound(value: String) -> Self {
                Self(value)
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

identity!(BodyId);
identity!(WakeId);
identity!(PartId);
identity!(MembershipChangeId);
identity!(MembershipProofId);
identity!(CandidateId);
identity!(DiscoveryProofId);
#[cfg(feature = "authenticated-admission")]
identity!(AdmissionId);
#[cfg(feature = "authenticated-admission")]
identity!(MembershipCredentialId);
#[cfg(feature = "authenticated-admission")]
identity!(SpawnInvitationId);

impl PartId {
    pub fn bind(
        body_id: &BodyId,
        durable_subject: &str,
        sequence: u64,
    ) -> Result<Self, BodyLifecycleError> {
        validate_ids(&[body_id.as_str(), durable_subject])?;
        Ok(Self::bound(bind_identity(
            "part",
            &[body_id.as_str(), durable_subject],
            sequence,
        )))
    }
}

impl MembershipProofId {
    /// Binds an opaque, non-secret reference to admission or future continuity proof.
    pub fn bind(proof_reference: &str) -> Result<Self, BodyLifecycleError> {
        validate_ids(&[proof_reference])?;
        Ok(Self::bound(bind_identity(
            "membership-proof",
            &[proof_reference],
            0,
        )))
    }
}

impl DiscoveryProofId {
    /// Opaque reference to transport/discovery proof state, never a membership credential.
    pub fn bind(proof_reference: &str) -> Result<Self, BodyLifecycleError> {
        validate_ids(&[proof_reference])?;
        Ok(Self::bound(bind_identity(
            "discovery-proof",
            &[proof_reference],
            0,
        )))
    }
}

pub(crate) fn validate_ids(values: &[&str]) -> Result<(), BodyLifecycleError> {
    for value in values {
        if value.is_empty() {
            return Err(BodyLifecycleError::EmptyIdentity);
        }
        if value.len() > MAX_LIFECYCLE_ID_BYTES {
            return Err(BodyLifecycleError::IdentityTooLong);
        }
    }
    Ok(())
}

/// Exact legacy identity transcript rendered into caller-owned fixed bytes.
/// No identity owner is constructed merely to compare an existing digest.
pub(crate) fn bound_identity_bytes(domain: &str, values: &[&str], sequence: u64) -> [u8; 64] {
    let mut digest = Sha256::new();
    for value in core::iter::once(domain).chain(values.iter().copied()) {
        digest.update((value.len() as u32).to_le_bytes());
        digest.update(value.as_bytes());
    }
    digest.update(sequence.to_le_bytes());
    let bytes: [u8; 32] = digest.finalize().into();
    let mut output = [0; 64];
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (pair, byte) in output.as_chunks_mut::<2>().0.iter_mut().zip(bytes) {
        pair[0] = HEX[usize::from(byte >> 4)];
        pair[1] = HEX[usize::from(byte & 15)];
    }
    output
}

pub(crate) fn bind_identity(domain: &str, values: &[&str], sequence: u64) -> String {
    let bytes = bound_identity_bytes(domain, values, sequence);
    // The fixed hexadecimal alphabet is always UTF-8, with unchanged wire text.
    String::from(core::str::from_utf8(&bytes).expect("hexadecimal identity"))
}
