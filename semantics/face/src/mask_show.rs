//! One exact Show of one Face through one planned ordinary Mask Plot.

use alloc::string::String;
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

/// Proof tied to one immutable borrow; callers cannot construct an unchecked
/// token or mutate its Face while canonical Show validation consumes it.
pub(crate) struct ValidatedFace<'a>(&'a Face);

impl<'a> ValidatedFace<'a> {
    pub(crate) fn from_constructed(face: &'a crate::CheckedFace) -> Self {
        Self(face.as_face())
    }

    pub(crate) fn new(face: &'a Face) -> Result<Self, crate::FaceError> {
        face.validate()?;
        Ok(Self(face))
    }

    pub(crate) fn face(&self) -> &'a Face {
        self.0
    }
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
        let checked = ValidatedFace::new(face).map_err(|_| MaskShowError::StaleFace)?;
        self.validate_validated(&checked)
    }

    /// Reuse validation only through an immutable, borrow-bound private proof.
    /// Public callers always enter through `validate` above.
    pub(crate) fn validate_validated(
        &self,
        checked: &ValidatedFace<'_>,
    ) -> Result<(), MaskShowError> {
        let face = checked.face();
        // Compare borrowed provenance; the constructor alone needs owned IDs.
        let face_plot = match (
            face.basis.source_document_id.as_ref(),
            face.basis.checked_plot_id.as_ref(),
            face.basis.expanded_plot_id.as_ref(),
        ) {
            (Some(source), Some(checked), Some(expanded)) => Some((source, checked, expanded)),
            _ => None,
        };
        let shown_plot = self.face_plot.as_ref().map(|plot| {
            (
                &plot.source_document_id,
                &plot.checked_plot_id,
                &plot.expanded_plot_id,
            )
        });
        if self.mask_plot != self.planned_mask.mask.plot_identity
            || shown_plot != face_plot
            || self.face_plan_id.as_ref() != face.basis.plan_id.as_ref()
            || self.face_id != face.identity
            || self.face_revision != face.revision
            || self.show.placement_id != self.planned_mask.show_placement().placement_id
        {
            return Err(MaskShowError::StalePlan);
        }
        self.show
            .validate_against_validated_mask_plot(checked, &self.planned_mask.plan)
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

pub(crate) fn validate_basis(
    face: &Face,
) -> Result<(Option<PlotIdentity>, Option<PlanId>), MaskShowError> {
    let checked = ValidatedFace::new(face).map_err(|_| MaskShowError::StaleFace)?;
    Ok(validated_basis(&checked))
}

fn validated_basis(face: &ValidatedFace<'_>) -> (Option<PlotIdentity>, Option<PlanId>) {
    let basis = &face.face().basis;
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
    (plot, basis.plan_id.clone())
}

fn bind_show(planned_mask: &PlannedMaskPlot, face: &Face, show: &Show) -> MaskShowId {
    bind_show_identity(
        &planned_mask.mask.plot_identity,
        &planned_mask.plan.plan_id,
        face,
        show,
    )
}
pub(crate) fn bind_show_identity(
    mask: &PlotIdentity,
    plan: &PlanId,
    face: &Face,
    show: &Show,
) -> MaskShowId {
    let bytes = show_digest(
        [
            mask.checked_plot_id.as_str(),
            mask.expanded_plot_id.as_str(),
            face.basis
                .checked_plot_id
                .as_ref()
                .map_or("<none>", |id| id.as_str()),
            face.basis
                .plan_id
                .as_ref()
                .map_or("<none>", |id| id.as_str()),
            face.identity.as_str(),
            plan.as_str(),
            show.manifestation_id.as_str(),
        ],
        face.revision,
    );
    show_identity(bytes)
}

fn show_identity(bytes: [u8; 32]) -> MaskShowId {
    let mut identity = String::with_capacity(69);
    identity.push_str("show/");
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        identity.push(DIGITS[(byte >> 4) as usize] as char);
        identity.push(DIGITS[(byte & 15) as usize] as char);
    }
    MaskShowId(identity)
}

/// Preserve the original newline-delimited UTF-8, including optional sentinels
/// and the revision's unsigned decimal spelling. This is not a new wire schema.
fn show_digest(fields: [&str; 7], revision: u64) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"conduit.face/mask-plot-show@1\n");
    for field in &fields[..5] {
        digest.update(field.as_bytes());
        digest.update(b"\n");
    }
    let mut decimal = [0u8; 20];
    let mut cursor = decimal.len();
    let mut remaining = revision;
    loop {
        cursor -= 1;
        decimal[cursor] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    digest.update(&decimal[cursor..]);
    digest.update(b"\n");
    for field in &fields[5..] {
        digest.update(field.as_bytes());
        digest.update(b"\n");
    }
    digest.finalize().into()
}

#[cfg(test)]
#[path = "mask_show/tests.rs"]
mod tests;
