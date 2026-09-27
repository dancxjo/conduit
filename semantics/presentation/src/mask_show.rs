//! One exact finite Show of one Presentation through one planned Mask.

use alloc::{format, string::String};
use conduit_core::{ActivePlayIdentity, Plan, SignId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    Manifestation, ManifestationError, ManifestationFailure, ManifestationLifecycle,
    MaskBoundaryRole, MaskSpecification, MaskSpecificationId, PlannedMask, Presentation,
    PresentationContentId, PresentationInteraction,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MaskShowId(String);

impl MaskShowId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskShow {
    pub show_id: MaskShowId,
    pub specification_id: MaskSpecificationId,
    pub specification_revision: u64,
    pub presentation_id: PresentationContentId,
    pub presentation_revision: u64,
    pub planned_mask: PlannedMask,
    pub manifestation: Manifestation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskInteractionCorrelation {
    pub show_id: MaskShowId,
    pub specification_id: MaskSpecificationId,
    pub presentation_id: PresentationContentId,
    pub presentation_revision: u64,
    pub interaction: PresentationInteraction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskShowError {
    StaleSpecification,
    StalePlan,
    MissingShowBoundary,
    MissingTerminalStage,
    WrongTerminalPlacement,
    InvalidManifestation(ManifestationError),
    StaleShowIdentity,
    StaleInteraction,
}

impl MaskShow {
    #[allow(clippy::too_many_arguments)]
    pub fn prepared(
        specification: &MaskSpecification,
        planned_mask: &PlannedMask,
        presentation: &Presentation,
        plan: &Plan,
        active_play: ActivePlayIdentity,
        front_subject: String,
        target_subject: String,
        sign_id: SignId,
    ) -> Result<Self, MaskShowError> {
        validate_basis(specification, planned_mask, plan)?;
        let terminal = terminal_placement(specification, planned_mask)?;
        let manifestation = Manifestation::prepared_at_mask_terminal(
            presentation,
            plan,
            active_play,
            terminal,
            front_subject,
            target_subject,
            sign_id,
        )
        .map_err(MaskShowError::InvalidManifestation)?;
        let show_id = bind_show(specification, presentation, planned_mask, &manifestation);
        Ok(Self {
            show_id,
            specification_id: specification.specification_id.clone(),
            specification_revision: specification.revision,
            presentation_id: presentation.identity.clone(),
            presentation_revision: presentation.revision,
            planned_mask: planned_mask.clone(),
            manifestation,
        })
    }

    pub fn transition(
        &self,
        lifecycle: ManifestationLifecycle,
        sign_id: SignId,
    ) -> Result<Self, MaskShowError> {
        let mut next = self.clone();
        next.manifestation = self
            .manifestation
            .transition(lifecycle, sign_id)
            .map_err(MaskShowError::InvalidManifestation)?;
        Ok(next)
    }

    pub fn fail(
        &self,
        failure: ManifestationFailure,
        sign_id: SignId,
    ) -> Result<Self, MaskShowError> {
        let mut next = self.clone();
        next.manifestation = self
            .manifestation
            .fail(failure, sign_id)
            .map_err(MaskShowError::InvalidManifestation)?;
        Ok(next)
    }

    pub fn validate(
        &self,
        specification: &MaskSpecification,
        presentation: &Presentation,
        plan: &Plan,
    ) -> Result<(), MaskShowError> {
        validate_basis(specification, &self.planned_mask, plan)?;
        let terminal = terminal_placement(specification, &self.planned_mask)?;
        if self.specification_id != specification.specification_id
            || self.specification_revision != specification.revision
            || self.presentation_id != presentation.identity
            || self.presentation_revision != presentation.revision
            || self.manifestation.placement_id != *terminal
        {
            return Err(MaskShowError::WrongTerminalPlacement);
        }
        self.manifestation
            .validate_against_mask_terminal(presentation, plan)
            .map_err(MaskShowError::InvalidManifestation)?;
        if self.show_id
            != bind_show(
                specification,
                presentation,
                &self.planned_mask,
                &self.manifestation,
            )
        {
            return Err(MaskShowError::StaleShowIdentity);
        }
        Ok(())
    }

    /// Correlate Mask-local input with the exact public Show it returned
    /// through. This grants no application authority; the ordinary Face
    /// interaction path still accepts or refuses the semantic action.
    pub fn correlate_interaction(
        &self,
        interaction: PresentationInteraction,
    ) -> Result<MaskInteractionCorrelation, MaskShowError> {
        if interaction.presentation_id != self.presentation_id.as_str()
            || interaction.presentation_revision != self.presentation_revision
            || interaction.manifestation_id != self.manifestation.manifestation_id.as_str()
        {
            return Err(MaskShowError::StaleInteraction);
        }
        Ok(MaskInteractionCorrelation {
            show_id: self.show_id.clone(),
            specification_id: self.specification_id.clone(),
            presentation_id: self.presentation_id.clone(),
            presentation_revision: self.presentation_revision,
            interaction,
        })
    }
}

fn validate_basis(
    specification: &MaskSpecification,
    planned_mask: &PlannedMask,
    plan: &Plan,
) -> Result<(), MaskShowError> {
    if planned_mask.specification_id != specification.specification_id
        || planned_mask.specification_revision != specification.revision
    {
        return Err(MaskShowError::StaleSpecification);
    }
    if planned_mask.plan_id != plan.plan_id {
        return Err(MaskShowError::StalePlan);
    }
    Ok(())
}

fn terminal_placement<'a>(
    specification: &MaskSpecification,
    planned_mask: &'a PlannedMask,
) -> Result<&'a conduit_core::PlacementId, MaskShowError> {
    let boundary = specification
        .boundaries
        .iter()
        .find(|boundary| boundary.role == MaskBoundaryRole::ShowOutput)
        .ok_or(MaskShowError::MissingShowBoundary)?;
    planned_mask
        .stages
        .iter()
        .find(|stage| stage.stage_id == boundary.stage_id)
        .map(|stage| &stage.placement_id)
        .ok_or(MaskShowError::MissingTerminalStage)
}

fn bind_show(
    specification: &MaskSpecification,
    presentation: &Presentation,
    planned_mask: &PlannedMask,
    manifestation: &Manifestation,
) -> MaskShowId {
    let canonical = format!(
        "conduit.presentation/mask-show@1\n{}\n{}\n{}\n{}\n{}\n{}\n",
        specification.specification_id.as_str(),
        specification.revision,
        presentation.identity.as_str(),
        presentation.revision,
        planned_mask.plan_id.as_str(),
        manifestation.manifestation_id.as_str(),
    );
    let digest = Sha256::digest(canonical.as_bytes());
    let mut identity = String::from("show/");
    for byte in digest {
        identity.push_str(&format!("{byte:02x}"));
    }
    MaskShowId(identity)
}
