//! One exact Show of one Face through one planned ordinary Mask Form.

use alloc::{format, string::String};
use conduit_core::{ActivePlayIdentity, FormIdentity, PlanId, SignId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    FaceInteraction, Manifestation, ManifestationError, ManifestationFailure,
    ManifestationLifecycle, PlannedMaskForm, Presentation, PresentationContentId, Show,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MaskShowId(String);

impl MaskShowId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskShow {
    pub show_id: MaskShowId,
    pub mask_form: FormIdentity,
    /// The application/tutorial Form that produced the Face Presentation.
    pub presentation_form: FormIdentity,
    /// The application Plan is deliberately distinct from the Mask Plan.
    pub presentation_plan_id: PlanId,
    pub presentation_id: PresentationContentId,
    pub presentation_revision: u64,
    pub planned_mask: PlannedMaskForm,
    pub show: Show,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskInteractionCorrelation {
    pub show_id: MaskShowId,
    pub mask_form: FormIdentity,
    pub presentation_id: PresentationContentId,
    pub presentation_revision: u64,
    pub interaction: FaceInteraction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskShowError {
    StalePresentation,
    MissingPresentationBasis,
    StalePlan,
    InvalidManifestation(ManifestationError),
    StaleShowIdentity,
    StaleInteraction,
}

impl MaskShow {
    #[allow(clippy::too_many_arguments)]
    pub fn prepared(
        planned_mask: &PlannedMaskForm,
        presentation: &Presentation,
        active_play: ActivePlayIdentity,
        front_subject: String,
        target_subject: String,
        sign_id: SignId,
    ) -> Result<Self, MaskShowError> {
        let (presentation_form, presentation_plan_id) = validate_basis(presentation)?;
        let show = Manifestation::prepared_at_mask_form_boundary(
            presentation,
            &planned_mask.plan,
            active_play,
            planned_mask.show_placement().placement_id.clone(),
            front_subject,
            target_subject,
            sign_id,
        )
        .map_err(MaskShowError::InvalidManifestation)?;
        let show_id = bind_show(planned_mask, presentation, &show);
        Ok(Self {
            show_id,
            mask_form: planned_mask.mask.form_identity.clone(),
            presentation_form,
            presentation_plan_id,
            presentation_id: presentation.identity.clone(),
            presentation_revision: presentation.revision,
            planned_mask: planned_mask.clone(),
            show,
        })
    }

    pub fn transition(
        &self,
        lifecycle: ManifestationLifecycle,
        sign_id: SignId,
    ) -> Result<Self, MaskShowError> {
        let mut next = self.clone();
        next.show = self
            .show
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
        next.show = self
            .show
            .fail(failure, sign_id)
            .map_err(MaskShowError::InvalidManifestation)?;
        Ok(next)
    }

    pub fn validate(&self, presentation: &Presentation) -> Result<(), MaskShowError> {
        let (presentation_form, presentation_plan_id) = validate_basis(presentation)?;
        if self.mask_form != self.planned_mask.mask.form_identity
            || self.presentation_form != presentation_form
            || self.presentation_plan_id != presentation_plan_id
            || self.presentation_id != presentation.identity
            || self.presentation_revision != presentation.revision
            || self.show.placement_id != self.planned_mask.show_placement().placement_id
        {
            return Err(MaskShowError::StalePlan);
        }
        self.show
            .validate_against_mask_form(presentation, &self.planned_mask.plan)
            .map_err(MaskShowError::InvalidManifestation)?;
        if self.show_id != bind_show(&self.planned_mask, presentation, &self.show) {
            return Err(MaskShowError::StaleShowIdentity);
        }
        Ok(())
    }

    pub fn correlate_interaction(
        &self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, MaskShowError> {
        if interaction.face_id != self.presentation_id.as_str()
            || interaction.face_revision != self.presentation_revision
            || interaction.show_id != self.show_id.as_str()
        {
            return Err(MaskShowError::StaleInteraction);
        }
        Ok(MaskInteractionCorrelation {
            show_id: self.show_id.clone(),
            mask_form: self.mask_form.clone(),
            presentation_id: self.presentation_id.clone(),
            presentation_revision: self.presentation_revision,
            interaction,
        })
    }
}

fn validate_basis(presentation: &Presentation) -> Result<(FormIdentity, PlanId), MaskShowError> {
    presentation
        .validate()
        .map_err(|_| MaskShowError::StalePresentation)?;
    let basis = &presentation.basis;
    let (Some(source_document_id), Some(checked_form_id), Some(expanded_form_id), Some(plan_id)) = (
        basis.source_document_id.clone(),
        basis.checked_form_id.clone(),
        basis.expanded_form_id.clone(),
        basis.plan_id.clone(),
    ) else {
        return Err(MaskShowError::MissingPresentationBasis);
    };
    Ok((
        FormIdentity {
            source_document_id,
            checked_form_id,
            expanded_form_id,
        },
        plan_id,
    ))
}

fn bind_show(
    planned_mask: &PlannedMaskForm,
    presentation: &Presentation,
    show: &Show,
) -> MaskShowId {
    let canonical = format!(
        "conduit.presentation/mask-form-show@1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        planned_mask.mask.form_identity.checked_form_id.as_str(),
        planned_mask.mask.form_identity.expanded_form_id.as_str(),
        presentation
            .basis
            .checked_form_id
            .as_ref()
            .expect("validated Mask Presentation retains its application Form")
            .as_str(),
        presentation
            .basis
            .plan_id
            .as_ref()
            .expect("validated Mask Presentation retains its application Plan")
            .as_str(),
        presentation.identity.as_str(),
        presentation.revision,
        planned_mask.plan.plan_id.as_str(),
        show.manifestation_id.as_str(),
    );
    let digest = Sha256::digest(canonical.as_bytes());
    let mut identity = String::from("show/");
    for byte in digest {
        identity.push_str(&format!("{byte:02x}"));
    }
    MaskShowId(identity)
}
