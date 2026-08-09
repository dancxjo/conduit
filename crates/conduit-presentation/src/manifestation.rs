//! Exact portable result of one planned renderer realization.

use alloc::string::String;
use conduit_core::{
    verify_plan, ActivePlayId, EvidenceId, PlacementId, Plan, PlanId, PlannedOperation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Presentation, PresentationContentId, RENDERER_KIND};

pub const MAX_MANIFESTATION_TARGET_BYTES: usize = 256;
pub const MAX_MANIFESTATION_EVIDENCE: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ManifestationId(String);

impl ManifestationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestationLifecycle {
    Prepared,
    Available,
    Replaced,
    Closed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifestation {
    pub manifestation_id: ManifestationId,
    pub presentation_id: PresentationContentId,
    pub presentation_revision: u64,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub placement_id: PlacementId,
    pub target_subject: String,
    pub lifecycle: ManifestationLifecycle,
    pub evidence_ids: alloc::vec::Vec<EvidenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestationError {
    InvalidPresentation,
    InvalidPlan,
    MissingRendererPlacement,
    WrongRendererContract,
    InvalidTarget,
    InvalidTransition,
    StaleIdentity,
}

impl core::fmt::Display for ManifestationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid Manifestation: {self:?}")
    }
}

impl Manifestation {
    pub fn prepared(
        presentation: &Presentation,
        plan: &Plan,
        active_play_id: ActivePlayId,
        placement_id: PlacementId,
        target_subject: String,
        evidence_id: EvidenceId,
    ) -> Result<Self, ManifestationError> {
        presentation
            .validate()
            .map_err(|_| ManifestationError::InvalidPresentation)?;
        let placement = renderer_placement(plan, &placement_id)?;
        validate_target(&target_subject)?;
        let manifestation_id = bind_manifestation(
            &presentation.identity,
            &plan.plan_id,
            &active_play_id,
            &placement.placement_id,
            &target_subject,
        );
        Ok(Self {
            manifestation_id,
            presentation_id: presentation.identity.clone(),
            presentation_revision: presentation.revision,
            plan_id: plan.plan_id.clone(),
            active_play_id,
            placement_id: placement.placement_id.clone(),
            target_subject,
            lifecycle: ManifestationLifecycle::Prepared,
            evidence_ids: alloc::vec![evidence_id],
        })
    }

    pub fn transition(
        &self,
        lifecycle: ManifestationLifecycle,
        evidence_id: EvidenceId,
    ) -> Result<Self, ManifestationError> {
        let accepted = matches!(
            (self.lifecycle, lifecycle),
            (
                ManifestationLifecycle::Prepared,
                ManifestationLifecycle::Available
            ) | (
                ManifestationLifecycle::Prepared,
                ManifestationLifecycle::Failed
            ) | (
                ManifestationLifecycle::Available,
                ManifestationLifecycle::Replaced
            ) | (
                ManifestationLifecycle::Available,
                ManifestationLifecycle::Closed
            ) | (
                ManifestationLifecycle::Available,
                ManifestationLifecycle::Failed
            )
        );
        if !accepted
            || self.evidence_ids.len() >= MAX_MANIFESTATION_EVIDENCE
            || self.evidence_ids.contains(&evidence_id)
        {
            return Err(ManifestationError::InvalidTransition);
        }
        let mut next = self.clone();
        next.lifecycle = lifecycle;
        next.evidence_ids.push(evidence_id);
        Ok(next)
    }

    pub fn validate_against<'a>(
        &self,
        presentation: &Presentation,
        plan: &'a Plan,
    ) -> Result<&'a PlannedOperation, ManifestationError> {
        presentation
            .validate()
            .map_err(|_| ManifestationError::InvalidPresentation)?;
        let placement = renderer_placement(plan, &self.placement_id)?;
        if self.presentation_id != presentation.identity
            || self.presentation_revision != presentation.revision
            || self.plan_id != plan.plan_id
            || self.manifestation_id
                != bind_manifestation(
                    &presentation.identity,
                    &plan.plan_id,
                    &self.active_play_id,
                    &self.placement_id,
                    &self.target_subject,
                )
        {
            return Err(ManifestationError::StaleIdentity);
        }
        validate_target(&self.target_subject)?;
        if self.evidence_ids.is_empty()
            || self.evidence_ids.len() > MAX_MANIFESTATION_EVIDENCE
            || self.evidence_ids.iter().any(|evidence| {
                evidence.as_str().is_empty()
                    || evidence.as_str().len() > crate::MAX_PRESENTATION_ID_BYTES
            })
            || self
                .evidence_ids
                .iter()
                .enumerate()
                .any(|(index, evidence)| self.evidence_ids[..index].contains(evidence))
        {
            return Err(ManifestationError::InvalidTransition);
        }
        Ok(placement)
    }
}

fn renderer_placement<'a>(
    plan: &'a Plan,
    placement_id: &PlacementId,
) -> Result<&'a PlannedOperation, ManifestationError> {
    if !verify_plan(plan) {
        return Err(ManifestationError::InvalidPlan);
    }
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| &placement.placement_id == placement_id)
        .ok_or(ManifestationError::MissingRendererPlacement)?;
    if placement.kind_id.as_str() != RENDERER_KIND
        || placement.inputs != crate::renderer_inputs()
        || placement.outputs != crate::renderer_outputs()
    {
        return Err(ManifestationError::WrongRendererContract);
    }
    Ok(placement)
}

fn validate_target(value: &str) -> Result<(), ManifestationError> {
    if value.is_empty() || value.len() > MAX_MANIFESTATION_TARGET_BYTES {
        Err(ManifestationError::InvalidTarget)
    } else {
        Ok(())
    }
}

fn bind_manifestation(
    presentation: &PresentationContentId,
    plan: &PlanId,
    active_play: &ActivePlayId,
    placement: &PlacementId,
    target_subject: &str,
) -> ManifestationId {
    let mut digest = Sha256::new();
    for value in [
        "conduit.presentation/manifestation@1",
        presentation.as_str(),
        plan.as_str(),
        active_play.as_str(),
        placement.as_str(),
        target_subject,
    ] {
        digest.update((value.len() as u32).to_le_bytes());
        digest.update(value.as_bytes());
    }
    let bytes: [u8; 32] = digest.finalize().into();
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    ManifestationId(output)
}
