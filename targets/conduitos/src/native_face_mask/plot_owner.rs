//! Explicit local Plot-owner publication through the ordinary checked Mask.
//! A Plot role alone is not producer custody. The trusted installed owner passes
//! its sealed Body Plan and actually issued Play, plus exact resident identity.
use super::*;
use alloc::string::String;
use conduit_body::{BodyPlan, BodyPlayIdentity, ResidentPlot};
use conduit_face::FaceRelationshipKind;

impl NativeFaceMask {
    /// Publish an ordinary resident Plot's Face with its actual Body execution
    /// basis. This does not issue/start a Body Play or grant make authority.
    /// The caller is the trusted execution owner and must supply its issued Play.
    #[allow(clippy::too_many_arguments)]
    pub fn present_plot_owner(
        &mut self,
        face: Face,
        body_plan: &BodyPlan,
        issued_play: &BodyPlayIdentity,
        plot: &ResidentPlot,
        observation_sequence: u64,
        mask_play_sequence: u64,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, NativeFaceMaskError> {
        let subject = validate_owner(self, &face, body_plan, issued_play, plot)?;
        self.present_with_input(
            face,
            observation_sequence,
            mask_play_sequence,
            display,
            true,
            Some(&subject),
        )
    }
}

fn validate_owner(
    mask: &NativeFaceMask,
    face: &Face,
    plan: &BodyPlan,
    play: &BodyPlayIdentity,
    plot: &ResidentPlot,
) -> Result<String, NativeFaceMaskError> {
    if plan.verify_seal().is_err() || !play.validate_for(plan) {
        return Err(NativeFaceMaskError::Plan);
    }
    let partition = plan
        .plots
        .iter()
        .find(|partition| &partition.plot == plot)
        .ok_or(NativeFaceMaskError::Plan)?;
    let source = &partition.plan;
    let basis = &face.basis;
    if basis.body_id.as_ref() != Some(&plan.body_id)
        || basis.wake_id.as_ref() != Some(&plan.wake_id)
        || basis.source_document_id.as_ref() != Some(&source.source_document_id)
        || basis.checked_plot_id.as_ref() != Some(&source.checked_plot_id)
        || basis.expanded_plot_id.as_ref() != Some(&source.expanded_plot_id)
        || basis.plan_id.as_ref() != Some(&source.plan_id)
        || basis.active_play_id.as_ref() != Some(&play.active_play_id)
    {
        return Err(NativeFaceMaskError::Face);
    }
    let local = mask
        .producer
        .plan()
        .fragments
        .first()
        .ok_or(NativeFaceMaskError::Plan)?;
    if !source.fragments.iter().any(|fragment| {
        fragment.host_id == local.host_id
            && fragment.boot_id == local.boot_id
            && fragment.offer_generation == local.offer_generation
    }) {
        return Err(NativeFaceMaskError::Plan);
    }
    // Select one actual root Plot subject only after execution provenance is
    // checked above. Neither a role nor descriptive properties grant authority.
    let mut subjects = face.subjects.iter().filter(|subject| {
        subject.role == FaceRole::Plot
            && !face.relationships.iter().any(|relation| {
                relation.kind == FaceRelationshipKind::Contains
                    && relation.target == subject.identity
            })
    });
    let subject = subjects.next().ok_or(NativeFaceMaskError::Face)?;
    if subjects.next().is_some() {
        return Err(NativeFaceMaskError::Face);
    }
    Ok(subject.identity.clone())
}

#[cfg(test)]
#[path = "plot_owner_tests.rs"]
mod tests;
