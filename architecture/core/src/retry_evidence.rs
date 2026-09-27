//! Exact retained evidence for an explicitly requested effectful retry.
//!
//! This is not a retry loop and it does not infer commit truth from a failed
//! Host Call. A provider protocol must produce the receipt, and a bounded
//! owner must retain it after the originating Play retires.

use crate::{
    ActivePlayIdentity, HostCallContractId, ImplementationId, KindId, KindIdentity, PlacementId,
    ReplayBehavior, SignIdentity,
};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAXIMUM_RETAINED_RETRY_EVIDENCE: usize = 256;
pub const MAXIMUM_RETRY_FINGERPRINT_DOMAIN_BYTES: usize = 128;
pub const MAXIMUM_RETRY_FINGERPRINT_PAYLOAD_BYTES: usize = 4_096;

/// A non-zero, domain-separated identity for a provider-owned operation or
/// receipt. It is evidence identity, not capability or authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RetryFingerprint([u8; 32]);

impl RetryFingerprint {
    pub fn derive(domain: &str, payload: &[u8]) -> Result<Self, RetryEvidenceRefusal> {
        if domain.is_empty()
            || domain.len() > MAXIMUM_RETRY_FINGERPRINT_DOMAIN_BYTES
            || payload.is_empty()
            || payload.len() > MAXIMUM_RETRY_FINGERPRINT_PAYLOAD_BYTES
        {
            return Err(RetryEvidenceRefusal::InvalidFingerprint);
        }
        let mut digest = Sha256::new();
        digest.update(b"conduit/retry-fingerprint@1");
        digest.update((domain.len() as u32).to_le_bytes());
        digest.update(domain.as_bytes());
        digest.update((payload.len() as u32).to_le_bytes());
        digest.update(payload);
        let value: [u8; 32] = digest.finalize().into();
        Ok(Self(value))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn is_valid(&self) -> bool {
        self.0 != [0; 32]
    }
}

/// Stable semantic identity of one effect across replacement Plans and Plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryOperationIdentity {
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub implementation_id: ImplementationId,
    pub operation_key_kind: KindId,
    pub operation_key_fingerprint: RetryFingerprint,
}

/// Inspectable provenance of the attempt which produced retained evidence.
/// None of these play-local fields substitutes for the stable operation key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryAttemptProvenance {
    pub active_play: ActivePlayIdentity,
    pub placement_id: PlacementId,
    pub host_call_contract_id: HostCallContractId,
    pub request_sequence: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetainedTransactionDisposition {
    NotCommitted,
    Committed,
    Unknown,
}

/// Exact provider-protocol truth retained after one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetainedRetryProof {
    /// The provider explicitly established that the effect did not commit.
    EffectNotCommitted { receipt: RetryFingerprint },
    /// The exact operation key remains effective under the reviewed
    /// idempotency contract, including after a committed first attempt.
    IdempotencyKeyRetained { receipt: RetryFingerprint },
    /// The retained transaction protocol knows its exact disposition.
    TransactionRetained {
        transaction_contract: KindId,
        transaction_fingerprint: RetryFingerprint,
        disposition: RetainedTransactionDisposition,
        receipt: RetryFingerprint,
    },
    /// The exact reviewed compensation completed before another attempt.
    CompensationCommitted {
        compensation_contract: KindId,
        compensation_fingerprint: RetryFingerprint,
        receipt: RetryFingerprint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedRetryEvidence {
    pub operation: RetryOperationIdentity,
    pub replay_law: ReplayBehavior,
    pub attempt: RetryAttemptProvenance,
    pub supporting_sign: SignIdentity,
    pub proof: RetainedRetryProof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryEvidenceRefusal {
    InvalidCapacity,
    CapacityExhausted,
    DuplicateEvidence,
    InvalidIdentity,
    InvalidFingerprint,
    InvalidAttemptProvenance,
    KindMismatch,
    KindRevisionMismatch,
    ImplementationMismatch,
    OperationMismatch,
    ReplayLawMismatch,
    DispositionDoesNotAuthorizeRetry,
}

/// Finite append-only retention. Capacity exhaustion preserves all existing
/// evidence and refuses the new record; it never evicts or overwrites truth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryEvidenceLedger {
    capacity: usize,
    entries: Vec<RetainedRetryEvidence>,
}

impl RetryEvidenceLedger {
    pub fn new(capacity: usize) -> Result<Self, RetryEvidenceRefusal> {
        if capacity == 0 || capacity > MAXIMUM_RETAINED_RETRY_EVIDENCE {
            return Err(RetryEvidenceRefusal::InvalidCapacity);
        }
        Ok(Self {
            capacity,
            entries: Vec::with_capacity(capacity),
        })
    }

    pub fn admit(&mut self, evidence: RetainedRetryEvidence) -> Result<(), RetryEvidenceRefusal> {
        validate_retained_retry_evidence(&evidence)?;
        if self.entries.iter().any(|existing| existing == &evidence) {
            return Err(RetryEvidenceRefusal::DuplicateEvidence);
        }
        if self.entries.len() == self.capacity {
            return Err(RetryEvidenceRefusal::CapacityExhausted);
        }
        self.entries.push(evidence);
        Ok(())
    }

    pub fn evidence_for<'a>(
        &'a self,
        operation: &'a RetryOperationIdentity,
    ) -> impl Iterator<Item = &'a RetainedRetryEvidence> + 'a {
        self.entries
            .iter()
            .filter(move |evidence| &evidence.operation == operation)
    }

    pub fn entries(&self) -> &[RetainedRetryEvidence] {
        &self.entries
    }
}

pub(crate) fn validate_retained_retry_evidence(
    evidence: &RetainedRetryEvidence,
) -> Result<(), RetryEvidenceRefusal> {
    let operation = &evidence.operation;
    if operation.kind_id.as_str().is_empty()
        || operation.kind_contract_revision.as_str().is_empty()
        || operation.implementation_id.as_str().is_empty()
        || operation.operation_key_kind.as_str().is_empty()
    {
        return Err(RetryEvidenceRefusal::InvalidIdentity);
    }
    if !operation.operation_key_fingerprint.is_valid()
        || !proof_fingerprints_are_valid(&evidence.proof)
    {
        return Err(RetryEvidenceRefusal::InvalidFingerprint);
    }
    let attempt = &evidence.attempt;
    let active = &attempt.active_play;
    if active.active_play_id.as_str().is_empty()
        || active.plan_id.as_str().is_empty()
        || active.host_id.as_str().is_empty()
        || active.boot_id.as_str().is_empty()
        || active.play_sequence == 0
        || attempt.placement_id.as_str().is_empty()
        || attempt.host_call_contract_id.as_str().is_empty()
        || attempt.request_sequence == 0
        || evidence.supporting_sign.sign_id.as_str().is_empty()
        || evidence.supporting_sign.host_id != active.host_id
        || evidence.supporting_sign.boot_id != active.boot_id
        || evidence.supporting_sign.active_play_id.as_ref() != Some(&active.active_play_id)
        || evidence.supporting_sign.sequence == 0
    {
        return Err(RetryEvidenceRefusal::InvalidAttemptProvenance);
    }
    Ok(())
}

fn proof_fingerprints_are_valid(proof: &RetainedRetryProof) -> bool {
    match proof {
        RetainedRetryProof::EffectNotCommitted { receipt }
        | RetainedRetryProof::IdempotencyKeyRetained { receipt } => receipt.is_valid(),
        RetainedRetryProof::TransactionRetained {
            transaction_contract,
            transaction_fingerprint,
            receipt,
            ..
        } => {
            !transaction_contract.as_str().is_empty()
                && transaction_fingerprint.is_valid()
                && receipt.is_valid()
        }
        RetainedRetryProof::CompensationCommitted {
            compensation_contract,
            compensation_fingerprint,
            receipt,
        } => {
            !compensation_contract.as_str().is_empty()
                && compensation_fingerprint.is_valid()
                && receipt.is_valid()
        }
    }
}
