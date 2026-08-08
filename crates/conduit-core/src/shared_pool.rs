use crate::{
    AuthorityGrantId, BootId, CapabilityId, CheckedFace, HostId, PlacementId, ResourceBinding,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SharedPoolId(String);

impl SharedPoolId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SharedPoolId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PoolDeclarationId(String);

impl PoolDeclarationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PoolDeclarationId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for PoolDeclarationId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolMemberLimits {
    pub queue_item_capacity: u16,
    pub queue_byte_capacity: u32,
    pub evidence_item_capacity: u16,
    pub evidence_byte_capacity: u32,
}

impl PoolMemberLimits {
    pub fn is_finite_and_nonzero(self) -> bool {
        self.queue_item_capacity > 0
            && self.queue_byte_capacity > 0
            && self.evidence_item_capacity > 0
            && self.evidence_byte_capacity > 0
    }
}

/// One exact boot-scoped capability/resource envelope into which a dynamic
/// member may be admitted. Runtime membership cannot invent another host,
/// capability, resource binding, or authority grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolRealizationEnvelope {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub resources: Vec<ResourceBinding>,
}

/// Immutable Plan truth for one bounded shared dynamic population.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedSharedPool {
    pub pool_id: SharedPoolId,
    pub declaration_id: PoolDeclarationId,
    pub member_face: CheckedFace,
    pub maximum_members: u16,
    pub member_limits: PoolMemberLimits,
    pub realization_envelope: Vec<PoolRealizationEnvelope>,
    pub admission_authority: AuthorityGrantId,
    /// Explicit placements that receive this exact pool reference. Name
    /// equality alone never grants access.
    pub consumers: Vec<PlacementId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedSharedPoolError {
    EmptyIdentity,
    EmptyCapacity,
    InvalidMemberLimits,
    EmptyRealizationEnvelope,
    DuplicateRealization,
    MissingConsumer,
    DuplicateConsumer,
}

impl PlannedSharedPool {
    pub fn validate(&self) -> Result<(), PlannedSharedPoolError> {
        if self.pool_id.as_str().is_empty()
            || self.declaration_id.as_str().is_empty()
            || self.admission_authority.as_str().is_empty()
        {
            return Err(PlannedSharedPoolError::EmptyIdentity);
        }
        if self.maximum_members == 0 {
            return Err(PlannedSharedPoolError::EmptyCapacity);
        }
        if !self.member_limits.is_finite_and_nonzero() {
            return Err(PlannedSharedPoolError::InvalidMemberLimits);
        }
        if self.realization_envelope.is_empty() {
            return Err(PlannedSharedPoolError::EmptyRealizationEnvelope);
        }
        for (index, realization) in self.realization_envelope.iter().enumerate() {
            if realization.host_id.as_str().is_empty()
                || realization.boot_id.as_str().is_empty()
                || realization.capability_id.as_str().is_empty()
                || self.realization_envelope[..index].iter().any(|prior| {
                    prior.host_id == realization.host_id
                        && prior.boot_id == realization.boot_id
                        && prior.capability_id == realization.capability_id
                })
            {
                return Err(PlannedSharedPoolError::DuplicateRealization);
            }
        }
        if self.consumers.is_empty() {
            return Err(PlannedSharedPoolError::MissingConsumer);
        }
        for (index, consumer) in self.consumers.iter().enumerate() {
            if consumer.as_str().is_empty() || self.consumers[..index].contains(consumer) {
                return Err(PlannedSharedPoolError::DuplicateConsumer);
            }
        }
        Ok(())
    }

    pub fn permits_realization(
        &self,
        host_id: &HostId,
        boot_id: &BootId,
        capability_id: &CapabilityId,
        face: &CheckedFace,
    ) -> bool {
        face == &self.member_face
            && self.realization_envelope.iter().any(|allowed| {
                &allowed.host_id == host_id
                    && &allowed.boot_id == boot_id
                    && &allowed.capability_id == capability_id
            })
    }
}
