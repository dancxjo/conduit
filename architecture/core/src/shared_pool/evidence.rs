//! Runtime observation and retained selection custody for a sealed shared pool.
//! Evidence validates against ordinary Plan truth; it does not admit capacity,
//! change the envelope, replay an operation, or perform selection.
use super::{PoolOperationId, PoolRealizationEnvelope, SharedPoolId};
use crate::{
    terminal_cause_digest, ArtifactId, BootId, CapabilityId, ControlLoopEvent, GearId, HostId,
    ImplementationId, KindId, OfferGeneration, Plan, PlanId, PlanningRequestAuthority,
    PlayUnsatisfiedReason, PortId, ResourceObservation, SignId, TerminalCategory, TerminalInfo,
};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

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
    InvalidPlanningOutcome,
    InvalidTerminalCause,
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

    /// Resolve the semantic terminal consequence of fresh planning after this
    /// pool exhausted its immutable envelope. A successful replacement is
    /// realization recovery and therefore produces no abnormal semantic value;
    /// a final planning refusal produces one typed value correlated to the
    /// exhaustion, request, and refusal Signs.
    pub fn terminal_after_planning_outcome(
        &self,
        plan: &Plan,
        request_sign_id: &SignId,
        outcome: &ControlLoopEvent,
        origin_kind: &KindId,
        origin_gear: &GearId,
        origin_port: &PortId,
    ) -> Result<Option<TerminalInfo>, PoolSelectionEvidenceError> {
        self.validate(plan)?;
        if self.disposition != PoolSelectionDisposition::EnvelopeExhausted
            || self.selected_realization.is_some()
            || outcome.validate().is_err()
        {
            return Err(PoolSelectionEvidenceError::InvalidPlanningOutcome);
        }
        let refusal_sign_id = match outcome {
            ControlLoopEvent::PlanningSucceeded {
                prior_plan_id,
                request_sign_id: outcome_request_sign_id,
                ..
            } if prior_plan_id == &plan.plan_id && outcome_request_sign_id == request_sign_id => {
                return Ok(None)
            }
            ControlLoopEvent::PlanningRefused {
                prior_plan_id,
                request_sign_id: outcome_request_sign_id,
                sign_id,
                ..
            } if prior_plan_id == &plan.plan_id && outcome_request_sign_id == request_sign_id => {
                sign_id
            }
            _ => return Err(PoolSelectionEvidenceError::InvalidPlanningOutcome),
        };
        let cause_digest = terminal_cause_digest(
            origin_kind,
            origin_gear,
            origin_port,
            &[
                self.sign_id.clone(),
                request_sign_id.clone(),
                refusal_sign_id.clone(),
            ],
            None,
        )
        .map_err(|_| PoolSelectionEvidenceError::InvalidTerminalCause)?;
        Ok(Some(TerminalInfo::new(
            TerminalCategory::UnavailableRealization,
            cause_digest,
        )))
    }
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
