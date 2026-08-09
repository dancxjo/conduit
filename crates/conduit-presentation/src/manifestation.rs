//! Exact portable result of one planned renderer realization.

use alloc::string::String;
use conduit_body::{BodyId, SeedId, WakeId};
use conduit_core::{verify_plan, ActivePlayId, ClueId, PlacementId, Plan, PlanId, PlannedGear};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Presentation, PresentationContentId, RENDERER_KIND};

pub const MAX_MANIFESTATION_TARGET_BYTES: usize = 256;
pub const MAX_MANIFESTATION_CLUES: usize = 3;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestationFailure {
    AdapterUnavailable,
    OutputRejected,
    DeliveryFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestationClue {
    pub clue_id: ClueId,
    pub manifestation_id: ManifestationId,
    pub presentation_id: PresentationContentId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub placement_id: PlacementId,
    pub lifecycle: ManifestationLifecycle,
    pub failure: Option<ManifestationFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifestation {
    pub manifestation_id: ManifestationId,
    pub seed_id: SeedId,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub presentation_id: PresentationContentId,
    pub presentation_revision: u64,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub placement_id: PlacementId,
    pub target_subject: String,
    pub lifecycle: ManifestationLifecycle,
    pub failure: Option<ManifestationFailure>,
    pub clues: alloc::vec::Vec<ManifestationClue>,
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
        clue_id: ClueId,
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
        let mut manifestation = Self {
            manifestation_id,
            seed_id: presentation.basis.seed_id.clone(),
            body_id: presentation.basis.body_id.clone(),
            wake_id: presentation.basis.wake_id.clone(),
            presentation_id: presentation.identity.clone(),
            presentation_revision: presentation.revision,
            plan_id: plan.plan_id.clone(),
            active_play_id,
            placement_id: placement.placement_id.clone(),
            target_subject,
            lifecycle: ManifestationLifecycle::Prepared,
            failure: None,
            clues: alloc::vec::Vec::new(),
        };
        manifestation.push_clue(clue_id);
        Ok(manifestation)
    }

    pub fn transition(
        &self,
        lifecycle: ManifestationLifecycle,
        clue_id: ClueId,
    ) -> Result<Self, ManifestationError> {
        let accepted = matches!(
            (self.lifecycle, lifecycle),
            (
                ManifestationLifecycle::Prepared,
                ManifestationLifecycle::Available
            ) | (
                ManifestationLifecycle::Available,
                ManifestationLifecycle::Replaced
            ) | (
                ManifestationLifecycle::Available,
                ManifestationLifecycle::Closed
            )
        );
        if !accepted
            || self.clues.len() >= MAX_MANIFESTATION_CLUES
            || self.clues.iter().any(|clue| clue.clue_id == clue_id)
        {
            return Err(ManifestationError::InvalidTransition);
        }
        let mut next = self.clone();
        next.lifecycle = lifecycle;
        next.failure = None;
        next.push_clue(clue_id);
        Ok(next)
    }

    pub fn fail(
        &self,
        failure: ManifestationFailure,
        clue_id: ClueId,
    ) -> Result<Self, ManifestationError> {
        if !matches!(
            self.lifecycle,
            ManifestationLifecycle::Prepared | ManifestationLifecycle::Available
        ) || self.clues.len() >= MAX_MANIFESTATION_CLUES
            || self.clues.iter().any(|clue| clue.clue_id == clue_id)
        {
            return Err(ManifestationError::InvalidTransition);
        }
        let mut next = self.clone();
        next.lifecycle = ManifestationLifecycle::Failed;
        next.failure = Some(failure);
        next.push_clue(clue_id);
        Ok(next)
    }

    pub fn validate_against<'a>(
        &self,
        presentation: &Presentation,
        plan: &'a Plan,
    ) -> Result<&'a PlannedGear, ManifestationError> {
        presentation
            .validate()
            .map_err(|_| ManifestationError::InvalidPresentation)?;
        let placement = renderer_placement(plan, &self.placement_id)?;
        if self.seed_id != presentation.basis.seed_id
            || self.body_id != presentation.basis.body_id
            || self.wake_id != presentation.basis.wake_id
            || self.presentation_id != presentation.identity
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
        if (self.lifecycle == ManifestationLifecycle::Failed) != self.failure.is_some()
            || !self.valid_clues()
        {
            return Err(ManifestationError::InvalidTransition);
        }
        Ok(placement)
    }

    fn push_clue(&mut self, clue_id: ClueId) {
        self.clues.push(ManifestationClue {
            clue_id,
            manifestation_id: self.manifestation_id.clone(),
            presentation_id: self.presentation_id.clone(),
            plan_id: self.plan_id.clone(),
            active_play_id: self.active_play_id.clone(),
            placement_id: self.placement_id.clone(),
            lifecycle: self.lifecycle,
            failure: self.failure,
        });
    }

    fn valid_clues(&self) -> bool {
        if self.clues.is_empty() || self.clues.len() > MAX_MANIFESTATION_CLUES {
            return false;
        }
        let mut prior = None;
        for (index, clue) in self.clues.iter().enumerate() {
            if clue.clue_id.as_str().is_empty()
                || clue.clue_id.as_str().len() > crate::MAX_PRESENTATION_ID_BYTES
                || self.clues[..index]
                    .iter()
                    .any(|earlier| earlier.clue_id == clue.clue_id)
                || clue.manifestation_id != self.manifestation_id
                || clue.presentation_id != self.presentation_id
                || clue.plan_id != self.plan_id
                || clue.active_play_id != self.active_play_id
                || clue.placement_id != self.placement_id
                || (clue.lifecycle == ManifestationLifecycle::Failed) != clue.failure.is_some()
                || !valid_lifecycle_step(prior, clue.lifecycle)
            {
                return false;
            }
            prior = Some(clue.lifecycle);
        }
        self.clues
            .last()
            .is_some_and(|clue| clue.lifecycle == self.lifecycle && clue.failure == self.failure)
    }
}

fn valid_lifecycle_step(
    prior: Option<ManifestationLifecycle>,
    next: ManifestationLifecycle,
) -> bool {
    matches!(
        (prior, next),
        (None, ManifestationLifecycle::Prepared)
            | (
                Some(ManifestationLifecycle::Prepared),
                ManifestationLifecycle::Available | ManifestationLifecycle::Failed
            )
            | (
                Some(ManifestationLifecycle::Available),
                ManifestationLifecycle::Replaced
                    | ManifestationLifecycle::Closed
                    | ManifestationLifecycle::Failed
            )
    )
}

fn renderer_placement<'a>(
    plan: &'a Plan,
    placement_id: &PlacementId,
) -> Result<&'a PlannedGear, ManifestationError> {
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
