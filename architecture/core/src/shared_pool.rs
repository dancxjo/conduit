mod evidence;
pub use evidence::*;

use crate::{
    AdmittedLine, ArtifactId, AuthorityGrantId, BootId, CapabilityId, CheckedFront, ConnectionId,
    HostId, ImplementationId, OfferGeneration, PlacementId, PlanId, PortId, ResourceBinding,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

impl From<String> for SharedPoolId {
    fn from(value: String) -> Self {
        Self(value)
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PoolOperationId(String);

impl PoolOperationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PoolOperationId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolMemberSessionDirection {
    Input,
    Output,
}

/// Bind one dynamic operation/port session to its immutable plan-owned pool.
/// The operation remains runtime identity; this digest does not add a Cord or
/// authorize a Line outside the pool realization envelope.
pub fn pool_member_session_connection_id(
    plan_id: &PlanId,
    pool_id: &SharedPoolId,
    operation_id: &PoolOperationId,
    direction: PoolMemberSessionDirection,
    port_id: &PortId,
) -> ConnectionId {
    let mut hash = Sha256::new();
    for value in [
        "pool-member-session@1",
        plan_id.as_str(),
        pool_id.as_str(),
        operation_id.as_str(),
        match direction {
            PoolMemberSessionDirection::Input => "input",
            PoolMemberSessionDirection::Output => "output",
        },
        port_id.as_str(),
    ] {
        hash.update((value.len() as u32).to_le_bytes());
        hash.update(value.as_bytes());
    }
    let digest: [u8; 32] = hash.finalize().into();
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(hex_digit(byte >> 4));
        encoded.push(hex_digit(byte & 0x0f));
    }
    ConnectionId::from(encoded)
}

fn hex_digit(value: u8) -> char {
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolMemberLimits {
    pub queue_item_capacity: u16,
    pub queue_byte_capacity: u32,
    pub sign_item_capacity: u16,
    pub sign_byte_capacity: u32,
}

impl PoolMemberLimits {
    pub fn is_finite_and_nonzero(self) -> bool {
        self.queue_item_capacity > 0
            && self.queue_byte_capacity > 0
            && self.sign_item_capacity > 0
            && self.sign_byte_capacity > 0
    }
}

/// One exact boot-scoped capability/resource envelope into which a dynamic
/// member may be admitted. Runtime membership cannot invent another host,
/// capability, resource binding, or authority grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolRealizationEnvelope {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub member_capacity: u16,
    pub resources: Vec<ResourceBinding>,
    /// Exact directional Lines permitted for dynamic member sessions.
    #[serde(default)]
    pub admitted_lines: Vec<AdmittedLine>,
}

/// Runtime policy sealed by the plan for choosing among its exact envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SharedPoolSelectionPolicy {
    MoreUnreservedThenLessUtilizedThenPlanOrder,
}

/// Immutable Plan truth for one bounded shared dynamic population.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedSharedPool {
    pub pool_id: SharedPoolId,
    pub declaration_id: PoolDeclarationId,
    pub member_front: CheckedFront,
    pub maximum_members: u16,
    pub member_limits: PoolMemberLimits,
    /// Cross-Host members must have exact request/result Lines sealed below.
    #[serde(default)]
    pub member_sessions_required: bool,
    pub realization_envelope: Vec<PoolRealizationEnvelope>,
    pub selection_policy: SharedPoolSelectionPolicy,
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
    InvalidMemberLine,
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
                || realization.implementation_id.as_str().is_empty()
                || realization.artifact_id.as_str().is_empty()
                || realization.member_capacity == 0
                || self.realization_envelope[..index].iter().any(|prior| {
                    prior.host_id == realization.host_id
                        && prior.boot_id == realization.boot_id
                        && prior.offer_generation == realization.offer_generation
                        && prior.capability_id == realization.capability_id
                })
            {
                return Err(PlannedSharedPoolError::DuplicateRealization);
            }
            for (line_index, line) in realization.admitted_lines.iter().enumerate() {
                let binding = &line.binding;
                if line.line_id.as_str().is_empty()
                    || binding.binding_id.as_str().is_empty()
                    || binding.base.as_str().is_empty()
                    || binding.base_instance_id.as_str().is_empty()
                    || binding.source.endpoint_id.as_str().is_empty()
                    || binding.sink.endpoint_id.as_str().is_empty()
                    || binding.source == binding.sink
                    || binding.limits.maximum_in_flight_items
                        < self.member_limits.queue_item_capacity
                    || binding.limits.maximum_payload_bytes < self.member_limits.queue_byte_capacity
                    || binding.limits.maximum_buffered_bytes < binding.limits.maximum_payload_bytes
                    || binding.limits.maximum_buffered_bytes
                        < self.member_limits.queue_byte_capacity
                    || binding.limits.maximum_frame_bytes < binding.limits.maximum_payload_bytes
                    || !((binding.source.host_id == realization.host_id
                        && binding.source.boot_id == realization.boot_id)
                        || (binding.sink.host_id == realization.host_id
                            && binding.sink.boot_id == realization.boot_id))
                    || realization.admitted_lines[..line_index]
                        .iter()
                        .any(|prior| {
                            prior.line_id == line.line_id
                                || prior.binding.binding_id == binding.binding_id
                        })
                {
                    return Err(PlannedSharedPoolError::InvalidMemberLine);
                }
            }
        }
        if self
            .realization_envelope
            .iter()
            .try_fold(0u16, |total, realization| {
                total.checked_add(realization.member_capacity)
            })
            .is_none_or(|total| total < self.maximum_members)
        {
            return Err(PlannedSharedPoolError::EmptyRealizationEnvelope);
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
        realization: &PoolRealizationEnvelope,
        front: &CheckedFront,
    ) -> bool {
        front == &self.member_front && self.realization_envelope.contains(realization)
    }
}
