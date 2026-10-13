//! One exact Show of one Face through one planned ordinary Mask Plot.

use alloc::{format, string::String};
use conduit_core::{ActivePlayIdentity, PlanId, PlotIdentity, SignId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    Face, FaceContentId, FaceInteraction, Manifestation, ManifestationError, ManifestationFailure,
    ManifestationLifecycle, PlannedMaskPlot, Show,
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
    pub mask_plot: PlotIdentity,
    /// Producing Plot, when this Face represents a planned execution.
    pub face_plot: Option<PlotIdentity>,
    /// A resting Body Face can have no current Plan. The Mask Plan remains exact.
    pub face_plan_id: Option<PlanId>,
    pub face_id: FaceContentId,
    pub face_revision: u64,
    pub planned_mask: PlannedMaskPlot,
    pub show: Show,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskInteractionCorrelation {
    pub show_id: MaskShowId,
    pub mask_plot: PlotIdentity,
    pub face_id: FaceContentId,
    pub face_revision: u64,
    pub interaction: FaceInteraction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskShowError {
    StaleFace,
    StalePlan,
    InvalidManifestation(ManifestationError),
    StaleShowIdentity,
    StaleInteraction,
}

impl MaskShow {
    #[allow(clippy::too_many_arguments)]
    pub fn prepared(
        planned_mask: &PlannedMaskPlot,
        face: &Face,
        active_play: ActivePlayIdentity,
        front_subject: String,
        target_subject: String,
        sign_id: SignId,
    ) -> Result<Self, MaskShowError> {
        let (face_plot, face_plan_id) = validate_basis(face)?;
        let show = Manifestation::prepared_at_mask_plot_boundary(
            face,
            &planned_mask.plan,
            active_play,
            planned_mask.show_placement().placement_id.clone(),
            front_subject,
            target_subject,
            sign_id,
        )
        .map_err(MaskShowError::InvalidManifestation)?;
        let show_id = bind_show(planned_mask, face, &show);
        Ok(Self {
            show_id,
            mask_plot: planned_mask.mask.plot_identity.clone(),
            face_plot,
            face_plan_id,
            face_id: face.identity.clone(),
            face_revision: face.revision,
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

    pub fn validate(&self, face: &Face) -> Result<(), MaskShowError> {
        let (face_plot, face_plan_id) = validate_basis(face)?;
        if self.mask_plot != self.planned_mask.mask.plot_identity
            || self.face_plot != face_plot
            || self.face_plan_id != face_plan_id
            || self.face_id != face.identity
            || self.face_revision != face.revision
            || self.show.placement_id != self.planned_mask.show_placement().placement_id
        {
            return Err(MaskShowError::StalePlan);
        }
        self.show
            .validate_against_mask_plot(face, &self.planned_mask.plan)
            .map_err(MaskShowError::InvalidManifestation)?;
        if self.show_id != bind_show(&self.planned_mask, face, &self.show) {
            return Err(MaskShowError::StaleShowIdentity);
        }
        Ok(())
    }

    pub fn correlate_interaction(
        &self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, MaskShowError> {
        if interaction.face_id != self.face_id.as_str()
            || interaction.face_revision != self.face_revision
            || interaction.show_id != self.show_id.as_str()
        {
            return Err(MaskShowError::StaleInteraction);
        }
        Ok(MaskInteractionCorrelation {
            show_id: self.show_id.clone(),
            mask_plot: self.mask_plot.clone(),
            face_id: self.face_id.clone(),
            face_revision: self.face_revision,
            interaction,
        })
    }
}

fn validate_basis(face: &Face) -> Result<(Option<PlotIdentity>, Option<PlanId>), MaskShowError> {
    face.validate().map_err(|_| MaskShowError::StaleFace)?;
    let basis = &face.basis;
    let plot = match (
        basis.source_document_id.clone(),
        basis.checked_plot_id.clone(),
        basis.expanded_plot_id.clone(),
    ) {
        (Some(source_document_id), Some(checked_plot_id), Some(expanded_plot_id)) => {
            Some(PlotIdentity {
                source_document_id,
                checked_plot_id,
                expanded_plot_id,
            })
        }
        _ => None,
    };
    Ok((plot, basis.plan_id.clone()))
}

fn bind_show(planned_mask: &PlannedMaskPlot, face: &Face, show: &Show) -> MaskShowId {
    let canonical = format!(
        "conduit.face/mask-plot-show@1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        planned_mask.mask.plot_identity.checked_plot_id.as_str(),
        planned_mask.mask.plot_identity.expanded_plot_id.as_str(),
        face.basis
            .checked_plot_id
            .as_ref()
            .map_or("<none>", |id| id.as_str()),
        face.basis
            .plan_id
            .as_ref()
            .map_or("<none>", |id| id.as_str()),
        face.identity.as_str(),
        face.revision,
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
