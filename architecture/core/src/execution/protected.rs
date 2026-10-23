//! Finite offered/selected protected execution policy, separate from enforcement.
use crate::{ExecutionProfileId, ResourceBinding, RUNTIME_MEMORY_RESOURCE_CLASS};
use serde::{Deserialize, Serialize};

/// Counts observed source interrupts while protected work executes. Missed or
/// coalesced interrupts are not reconstructed. This is neither calibrated CPU
/// time nor instruction fuel, and does not convert cooperative Step fuel.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProtectedEntryAccounting {
    SourceInterruptsV1,
}
impl ProtectedEntryAccounting {
    pub const fn profile_id(self) -> &'static str {
        match self {
            Self::SourceInterruptsV1 => "conduit.execution/source-interrupt-count@1",
        }
    }
}

/// One logical request keeps its cumulative count across slices, gate returns
/// and resumes. Only its existing request owner can end that operation; returning
/// to Root or retrying transport cannot renew the count.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtectedEntryBudget {
    pub accounting: ProtectedEntryAccounting,
    pub quantum_interrupts: u32,
    pub total_interrupts: u32,
}
impl ProtectedEntryBudget {
    pub const fn is_valid(&self) -> bool {
        self.quantum_interrupts > 0
            && self.total_interrupts > 0
            && self.quantum_interrupts <= self.total_interrupts
    }
}

/// Exact enforcement offer on an existing compute resource pool. Its containing
/// advertisement and Base ownership supply Host/Boot/generation scope. Merely
/// selecting or serializing this descriptor does not install a target mechanism.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtectedPreemptionOffer {
    pub execution_profile_id: ExecutionProfileId,
    pub policy: ProtectedEntryBudget,
    pub context_bytes: u32,
    pub context_alignment: u32,
}
impl ProtectedPreemptionOffer {
    pub fn is_valid(&self) -> bool {
        !self.execution_profile_id.as_str().is_empty()
            && self.policy.is_valid()
            && valid_extent(self.context_bytes, self.context_alignment)
    }
}

/// Complete additional continuation backing, separate from worker partition
/// storage and selected Back resources. Target admission checks actual backing
/// and alignment before entry; the Plan commits their finite required extent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContinuationStorage {
    pub binding: ResourceBinding,
    pub bytes: u32,
    pub alignment: u32,
}
impl ExecutionContinuationStorage {
    pub fn is_valid(&self) -> bool {
        valid_extent(self.bytes, self.alignment)
            && !self.binding.pool_id.as_str().is_empty()
            && self.binding.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS
            && self.binding.units >= self.bytes
            && self.binding.compute.is_none()
            && self.binding.protected.is_none()
            && self.binding.content.is_none()
    }
}

fn valid_extent(bytes: u32, alignment: u32) -> bool {
    bytes > 0 && alignment.is_power_of_two() && bytes.is_multiple_of(alignment)
}
