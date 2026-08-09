//! Bounded content-derived identities for Realm deployment continuity.

use alloc::string::String;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::RealmLifecycleError;

pub const MAX_LIFECYCLE_ID_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DeploymentId(String);

impl DeploymentId {
    pub(crate) fn bound(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActivationId(String);

impl ActivationId {
    pub(crate) fn bound(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) fn validate_lifecycle_ids(values: &[&str]) -> Result<(), RealmLifecycleError> {
    for value in values {
        if value.is_empty() {
            return Err(RealmLifecycleError::EmptyIdentity);
        }
        if value.len() > MAX_LIFECYCLE_ID_BYTES {
            return Err(RealmLifecycleError::IdentityTooLong);
        }
    }
    Ok(())
}

pub(crate) fn bind_lifecycle_identity(domain: &str, values: &[&str], sequence: u64) -> String {
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
