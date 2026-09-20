use crate::{
    AdmittedLine, ArtifactId, AuthorityGrantId, BootId, CapabilityId, CheckedFront, ConnectionId,
    ControlLoopEvent, HostId, ImplementationId, OfferGeneration, PlacementId, Plan, PlanId,
    PlanningRequestAuthority, PlayUnsatisfiedReason, PortId, ResourceBinding, ResourceObservation,
    SignId,
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
pub enum PoolSelectionDisposition {
    Selected,
    CapacityRefused,
    ObservationRefused,
    ProviderLost,
    EnvelopeExhausted,
}

/// Bounded evidence for one new-operation selection or one in-flight loss.
/// The selected realization is an index into the immutable Plan envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolSelectionEvidence {
    pub plan_id: PlanId,
    pub pool_id: SharedPoolId,
    pub operation_id: PoolOperationId,
    pub selected_realization: Option<u16>,
    pub observation_sign_ids: Vec<SignId>,
    pub disposition: PoolSelectionDisposition,
    pub sign_id: SignId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSelectionEvidenceError {
    EmptyIdentity,
    InvalidPlan,
    PoolOutsidePlan,
    RealizationOutsideEnvelope,
    InvalidDisposition,
    MissingObservation,
    RequesterOutsidePlan,
}

impl PoolSelectionEvidence {
    pub fn validate(&self, plan: &Plan) -> Result<(), PoolSelectionEvidenceError> {
        if self.operation_id.as_str().is_empty()
            || self.sign_id.as_str().is_empty()
            || self
                .observation_sign_ids
                .iter()
                .any(|id| id.as_str().is_empty())
        {
            return Err(PoolSelectionEvidenceError::EmptyIdentity);
        }
        if !crate::verify_plan(plan) || self.plan_id != plan.plan_id {
            return Err(PoolSelectionEvidenceError::InvalidPlan);
        }
        let pool = plan
            .fragments
            .first()
            .and_then(|fragment| {
                fragment
                    .shared_pools
                    .iter()
                    .find(|pool| pool.pool_id == self.pool_id)
            })
            .ok_or(PoolSelectionEvidenceError::PoolOutsidePlan)?;
        if self
            .selected_realization
            .is_some_and(|index| usize::from(index) >= pool.realization_envelope.len())
        {
            return Err(PoolSelectionEvidenceError::RealizationOutsideEnvelope);
        }
        let selected_disposition = matches!(
            self.disposition,
            PoolSelectionDisposition::Selected | PoolSelectionDisposition::ProviderLost
        );
        if selected_disposition != self.selected_realization.is_some() {
            return Err(PoolSelectionEvidenceError::InvalidDisposition);
        }
        if selected_disposition && self.observation_sign_ids.is_empty() {
            return Err(PoolSelectionEvidenceError::MissingObservation);
        }
        Ok(())
    }

    /// Convert one terminal finite-envelope search into the ordinary control
    /// loop transition that requests a replacement Plan. Capacity pressure and
    /// provider loss cannot silently replan or replay an operation.
    pub fn exhaustion_replan_events(
        &self,
        plan: &Plan,
        requester_host_id: HostId,
        requester_boot_id: BootId,
        authority: PlanningRequestAuthority,
        request_sign_id: SignId,
    ) -> Result<[ControlLoopEvent; 2], PoolSelectionEvidenceError> {
        self.validate(plan)?;
        if self.disposition != PoolSelectionDisposition::EnvelopeExhausted
            || self.selected_realization.is_some()
        {
            return Err(PoolSelectionEvidenceError::InvalidDisposition);
        }
        if !plan.fragments.iter().any(|fragment| {
            fragment.host_id == requester_host_id && fragment.boot_id == requester_boot_id
        }) {
            return Err(PoolSelectionEvidenceError::RequesterOutsidePlan);
        }
        let events = [
            ControlLoopEvent::PlayBecameUnsatisfied {
                plan_id: plan.plan_id.clone(),
                reason: PlayUnsatisfiedReason::NoAdmittedPoolRealizationReady,
                sign_id: self.sign_id.clone(),
            },
            ControlLoopEvent::PlanningRequested {
                prior_plan_id: plan.plan_id.clone(),
                requester_host_id,
                requester_boot_id,
                authority,
                request_sign_id,
            },
        ];
        debug_assert!(events.iter().all(|event| event.validate().is_ok()));
        Ok(events)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolRealizationHealth {
    Ready,
    Unavailable,
}

/// Current provider and resource truth for one exact sealed realization.
/// This is observation, never stable offer or Plan truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolRealizationObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub health: PoolRealizationHealth,
    pub sign_id: SignId,
    pub resources: Vec<ResourceObservation>,
}

impl PoolRealizationObservation {
    pub fn is_current_for(&self, realization: &PoolRealizationEnvelope) -> bool {
        self.host_id == realization.host_id
            && self.boot_id == realization.boot_id
            && self.offer_generation == realization.offer_generation
            && self.capability_id == realization.capability_id
            && self.implementation_id == realization.implementation_id
            && self.artifact_id == realization.artifact_id
            && !self.sign_id.as_str().is_empty()
            && realization.resources.iter().all(|binding| {
                let mut matches = self.resources.iter().filter(|observation| {
                    observation.host_id == self.host_id
                        && observation.boot_id == self.boot_id
                        && observation.offer_generation == self.offer_generation
                        && observation.pool_id == binding.pool_id
                        && observation.class_id == binding.class_id
                });
                matches.next().is_some() && matches.next().is_none()
            })
    }
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
