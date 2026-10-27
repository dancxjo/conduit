use alloc::string::String;
use portable_atomic_util::Arc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::BodyLifecycleError;

pub const MAX_LIFECYCLE_ID_BYTES: usize = 256;

macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(Arc<str>);

        impl $name {
            pub(crate) fn bound(value: String) -> Self {
                Self(Arc::from(value))
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

#[cfg(test)]
mod immutable_id_tests {
    extern crate std;
    use super::*;
    #[test]
    fn clones_share_immutable_bytes_and_keep_thread_contracts() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<BodyId>();
        send_sync::<PartId>();
        send_sync::<MembershipProofId>();
        let original = BodyId::bound(String::from("body/shared-id"));
        let cloned = original.clone();
        assert!(Arc::ptr_eq(&original.0, &cloned.0));
        assert_eq!(original, cloned);
        assert_eq!(original.as_str(), "body/shared-id");
        use core::hash::{Hash, Hasher};
        fn hash(value: &impl Hash) -> u64 {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            value.hash(&mut h);
            h.finish()
        }
        assert_eq!(hash(&original), hash(&String::from(original.as_str())));
        assert!(BodyId::bound(String::from("a")) < BodyId::bound(String::from("b")));
    }
    #[test]
    fn immutable_id_json_and_postcard_keep_exact_string_wire() {
        let value = BodyId::bound(String::from("body/wire-parity"));
        assert_eq!(
            serde_json::to_vec(&value).unwrap(),
            serde_json::to_vec(value.as_str()).unwrap()
        );
        assert_eq!(
            postcard::to_allocvec(&value).unwrap(),
            postcard::to_allocvec(value.as_str()).unwrap()
        );
        let decoded: BodyId = serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(decoded, value);
        // Wire decoding alone does not intern independently encoded identities.
        assert!(!Arc::ptr_eq(&decoded.0, &value.0));
    }
}
