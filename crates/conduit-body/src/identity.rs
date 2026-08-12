use alloc::string::String;
use conduit_core::{CheckedFormId, SourceDocumentId};
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

identity!(SeedId);
identity!(BodyId);
identity!(WakeId);
identity!(PartId);
identity!(MembershipChangeId);
identity!(MembershipProofId);
identity!(CandidateId);
identity!(DiscoveryProofId);

impl SeedId {
    pub fn bind(source: &SourceDocumentId, checked: &CheckedFormId) -> Self {
        Self::bound(bind_identity(
            "seed",
            &[source.as_str(), checked.as_str()],
            0,
        ))
    }
}

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

pub(crate) fn bind_identity(domain: &str, values: &[&str], sequence: u64) -> String {
    let mut digest = Sha256::new();
    for value in core::iter::once(domain).chain(values.iter().copied()) {
        digest.update((value.len() as u32).to_le_bytes());
        digest.update(value.as_bytes());
    }
    digest.update(sequence.to_le_bytes());
    let bytes: [u8; 32] = digest.finalize().into();
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
