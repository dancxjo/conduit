//! Exact planned lifecycle for one renderer manifestation.

use alloc::string::String;
use conduit_core::{bind_active_play, ActivePlayId, PlacementId, Plan, SignId};

use crate::{
    Manifestation, ManifestationError, ManifestationFailure, ManifestationLifecycle, Presentation,
    RendererSelfInspection, RendererSelfInspectionError, RENDERER_KIND,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererExecution {
    pub presentation: Presentation,
    pub plan: Plan,
    pub active_play_id: ActivePlayId,
    pub placement_id: PlacementId,
    pub manifestation: Manifestation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererExecutionError {
    MissingPlacement,
    AmbiguousPlacement,
    Manifestation(ManifestationError),
    Inspection(RendererSelfInspectionError),
}

impl core::fmt::Display for RendererExecutionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "renderer execution failed: {self:?}")
    }
}

impl RendererExecution {
    pub fn prepare_planned(
        presentation: Presentation,
        plan: Plan,
        target_subject: String,
        sign_id: SignId,
    ) -> Result<Self, RendererExecutionError> {
        Self::prepare_planned_sequence(presentation, plan, target_subject, 0, sign_id)
    }

    pub fn prepare_planned_sequence(
        presentation: Presentation,
        plan: Plan,
        target_subject: String,
        play_sequence: u64,
        sign_id: SignId,
    ) -> Result<Self, RendererExecutionError> {
        presentation.validate().map_err(|_| {
            RendererExecutionError::Manifestation(ManifestationError::InvalidPresentation)
        })?;
        let mut placements = plan.fragments.iter().flat_map(|fragment| {
            fragment
                .placements
                .iter()
                .filter(|placement| placement.kind_id.as_str() == RENDERER_KIND)
                .map(move |placement| (fragment, placement))
        });
        let (fragment, placement) = placements
            .next()
            .ok_or(RendererExecutionError::MissingPlacement)?;
        if placements.next().is_some() {
            return Err(RendererExecutionError::AmbiguousPlacement);
        }
        let placement_id = placement.placement_id.clone();
        let active_play = bind_active_play(
            &plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let active_play_id = active_play.active_play_id.clone();
        let manifestation = Manifestation::prepared(
            &presentation,
            &plan,
            active_play,
            placement_id.clone(),
            presentation
                .subjects
                .first()
                .map(|subject| subject.identity.clone())
                .ok_or(RendererExecutionError::MissingPlacement)?,
            target_subject,
            sign_id,
        )
        .map_err(RendererExecutionError::Manifestation)?;
        Ok(Self {
            presentation,
            plan,
            active_play_id,
            placement_id,
            manifestation,
        })
    }

    pub fn mark_available(&mut self, sign_id: SignId) -> Result<(), RendererExecutionError> {
        if self.manifestation.lifecycle == ManifestationLifecycle::Available {
            return Ok(());
        }
        self.manifestation = self
            .manifestation
            .transition(ManifestationLifecycle::Available, sign_id)
            .map_err(RendererExecutionError::Manifestation)?;
        Ok(())
    }

    pub fn mark_failed(
        &mut self,
        failure: ManifestationFailure,
        sign_id: SignId,
    ) -> Result<(), RendererExecutionError> {
        self.manifestation = self
            .manifestation
            .fail(failure, sign_id)
            .map_err(RendererExecutionError::Manifestation)?;
        Ok(())
    }

    pub fn mark_closed(&mut self, sign_id: SignId) -> Result<(), RendererExecutionError> {
        self.manifestation = self
            .manifestation
            .transition(ManifestationLifecycle::Closed, sign_id)
            .map_err(RendererExecutionError::Manifestation)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), RendererExecutionError> {
        self.manifestation
            .validate_against(&self.presentation, &self.plan)
            .map_err(RendererExecutionError::Manifestation)?;
        if self.manifestation.active_play_id != self.active_play_id
            || self.manifestation.placement_id != self.placement_id
        {
            return Err(RendererExecutionError::Manifestation(
                ManifestationError::StaleIdentity,
            ));
        }
        Ok(())
    }

    pub fn self_inspection(&self) -> Result<RendererSelfInspection, RendererExecutionError> {
        self.validate()?;
        RendererSelfInspection::new(
            &self.presentation,
            self.plan.clone(),
            self.manifestation.clone(),
        )
        .map_err(RendererExecutionError::Inspection)
    }
}
