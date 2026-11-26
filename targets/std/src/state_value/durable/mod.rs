//! Bounded transactional semantics for a Body-lived State realization.
//!
//! This is deliberately separate from [`super::RetainedTypedState`]. That type
//! is consumed ownership passed between graceful Plays; it is not a crash
//! checkpoint. This module also makes no filesystem or hard-loss claim. A Host
//! can use this exact protocol only when its separately admitted implementation
//! can truthfully provide the required residence and synchronization.

use conduit_core::{KindId, StateId};
use sha2::{Digest, Sha256};

const PROTOCOL_VERSION: u8 = 1;
const RECOVERY_ABSENT: u8 = 0;
const RECOVERY_PRESENT: u8 = 1;
const RECEIPT_BYTES: usize = 1 + 8 + 32;
const RECOVERY_HEADER_BYTES: usize = 2 + 8 + 32;
pub const MAXIMUM_DURABLE_STATE_BYTES: u32 =
    conduit_std_offers::STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES;

/// Exact semantic owner of one durable State journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableStateBinding {
    pub body: String,
    pub state: StateId,
    pub value_kind: KindId,
    pub maximum_value_bytes: u32,
}

mod binding_profile;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Generation {
    number: u64,
    digest: [u8; 32],
    value: Vec<u8>,
}

/// Recovery truth remains more exact than a generic I/O error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDisposition {
    Absent,
    Recovered { generation: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableStateRefusal {
    InvalidBinding,
    ValueTooLarge,
    GenerationGap,
    ConflictingDigest,
    Corrupt,
    Incompatible,
    Lost,
    StaleRecovery,
    InvalidReceipt,
}

/// Deterministic Host-side transactional model. `candidate` represents bytes
/// that may have been written but have not acquired committed-generation truth.
pub struct DurableStateHost {
    binding: DurableStateBinding,
    committed: Option<Generation>,
    candidate: Option<Generation>,
    lost_generation: Option<u64>,
}

impl DurableStateHost {
    pub fn new(binding: DurableStateBinding) -> Result<Self, DurableStateRefusal> {
        binding.validate()?;
        Ok(Self {
            binding,
            committed: None,
            candidate: None,
            lost_generation: None,
        })
    }

    /// Stage a bounded candidate. It is intentionally invisible to recovery
    /// until [`commit_candidate`](Self::commit_candidate) publishes it.
    pub fn stage_candidate(
        &mut self,
        generation: u64,
        value: &[u8],
    ) -> Result<(), DurableStateRefusal> {
        if value.len() > self.binding.maximum_value_bytes as usize {
            return Err(DurableStateRefusal::ValueTooLarge);
        }
        let expected = self
            .committed
            .as_ref()
            .map_or(1, |current| current.number.saturating_add(1));
        let digest = digest(value);
        if let Some(current) = self
            .committed
            .as_ref()
            .filter(|item| item.number == generation)
        {
            return if current.digest == digest {
                Ok(())
            } else {
                Err(DurableStateRefusal::ConflictingDigest)
            };
        }
        if generation != expected {
            return Err(DurableStateRefusal::GenerationGap);
        }
        if let Some(candidate) = &self.candidate {
            return if candidate.number == generation && candidate.digest == digest {
                Ok(())
            } else if candidate.number == generation {
                Err(DurableStateRefusal::ConflictingDigest)
            } else {
                Err(DurableStateRefusal::GenerationGap)
            };
        }
        self.candidate = Some(Generation {
            number: generation,
            digest,
            value: value.to_vec(),
        });
        Ok(())
    }

    pub fn commit_candidate(&mut self) -> Result<[u8; RECEIPT_BYTES], DurableStateRefusal> {
        let candidate = self
            .candidate
            .take()
            .ok_or(DurableStateRefusal::GenerationGap)?;
        let receipt = encode_receipt(candidate.number, candidate.digest);
        self.committed = Some(candidate);
        self.lost_generation = None;
        Ok(receipt)
    }

    /// Exact Host Call semantics: the same binding, generation and digest is
    /// idempotent; the same generation with different bytes is a conflict.
    pub fn commit(
        &mut self,
        binding: &DurableStateBinding,
        generation: u64,
        value: &[u8],
    ) -> Result<[u8; RECEIPT_BYTES], DurableStateRefusal> {
        if binding != &self.binding {
            return Err(DurableStateRefusal::Incompatible);
        }
        let value_digest = digest(value);
        if let Some(current) = self
            .committed
            .as_ref()
            .filter(|item| item.number == generation)
        {
            return if current.digest == value_digest {
                Ok(encode_receipt(generation, value_digest))
            } else {
                Err(DurableStateRefusal::ConflictingDigest)
            };
        }
        self.stage_candidate(generation, value)?;
        self.commit_candidate()
    }

    /// Test/support hook for representing a separately observed loss of a
    /// generation that was known to have committed. This is not `Absent`.
    pub fn mark_committed_lost(&mut self) {
        self.lost_generation = self.committed.take().map(|item| item.number);
    }

    pub fn recover(
        &self,
        binding: &DurableStateBinding,
    ) -> Result<(RecoveryDisposition, Vec<u8>), DurableStateRefusal> {
        if binding != &self.binding {
            return Err(DurableStateRefusal::Incompatible);
        }
        if self.lost_generation.is_some() {
            return Err(DurableStateRefusal::Lost);
        }
        let Some(current) = &self.committed else {
            return Ok((RecoveryDisposition::Absent, encode_absent()));
        };
        if digest(&current.value) != current.digest {
            return Err(DurableStateRefusal::Corrupt);
        }
        let mut response = Vec::with_capacity(RECOVERY_HEADER_BYTES + current.value.len());
        response.extend_from_slice(&[PROTOCOL_VERSION, RECOVERY_PRESENT]);
        response.extend_from_slice(&current.number.to_be_bytes());
        response.extend_from_slice(&current.digest);
        response.extend_from_slice(&current.value);
        Ok((
            RecoveryDisposition::Recovered {
                generation: current.number,
            },
            response,
        ))
    }

    #[cfg(test)]
    fn corrupt_committed_value(&mut self) {
        self.committed.as_mut().expect("committed generation").value[0] ^= 0xff;
    }
}

fn digest(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

fn encode_receipt(generation: u64, digest: [u8; 32]) -> [u8; RECEIPT_BYTES] {
    let mut receipt = [0; RECEIPT_BYTES];
    receipt[0] = PROTOCOL_VERSION;
    receipt[1..9].copy_from_slice(&generation.to_be_bytes());
    receipt[9..].copy_from_slice(&digest);
    receipt
}

fn decode_receipt(bytes: &[u8]) -> Option<(u64, [u8; 32])> {
    if bytes.len() != RECEIPT_BYTES || bytes[0] != PROTOCOL_VERSION {
        return None;
    }
    Some((
        u64::from_be_bytes(bytes[1..9].try_into().ok()?),
        bytes[9..].try_into().ok()?,
    ))
}

fn encode_absent() -> Vec<u8> {
    vec![PROTOCOL_VERSION, RECOVERY_ABSENT]
}

mod back;
pub use back::DurableStateBack;
mod residence;
pub use residence::FileDurableStateResidence;
pub(crate) use residence::InstalledDurableStateHost;

#[cfg(test)]
mod tests;
