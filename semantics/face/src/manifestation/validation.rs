//! Canonical Face/placement validation shared by bare and checked Mask paths.
use super::*;
use crate::mask_show::ValidatedFace;

impl Manifestation {
    pub fn validate_against<'a>(
        &self,
        face: &Face,
        plan: &'a Plan,
    ) -> Result<&'a PlannedGear, ManifestationError> {
        face.validate()
            .map_err(|_| ManifestationError::InvalidFace)?;
        let placement = renderer_placement(plan, &self.placement_id)?;
        self.validate_at_placement(face, plan, placement)?;
        Ok(placement)
    }

    pub(crate) fn validate_against_mask_plot<'a>(
        &self,
        face: &Face,
        plan: &'a Plan,
    ) -> Result<&'a PlannedGear, ManifestationError> {
        let checked = ValidatedFace::new(face).map_err(|_| ManifestationError::InvalidFace)?;
        self.validate_against_validated_mask_plot(&checked, plan)
    }

    pub(crate) fn validate_against_validated_mask_plot<'a>(
        &self,
        face: &ValidatedFace<'_>,
        plan: &'a Plan,
    ) -> Result<&'a PlannedGear, ManifestationError> {
        let face = face.face();
        let placement = planned_placement(plan, &self.placement_id)?;
        self.validate_at_placement(face, plan, placement)?;
        Ok(placement)
    }

    fn validate_at_placement(
        &self,
        face: &Face,
        plan: &Plan,
        placement: &PlannedGear,
    ) -> Result<(), ManifestationError> {
        if self.body_id != face.basis.body_id
            || self.wake_id != face.basis.wake_id
            || self.face_id != face.identity
            || self.face_revision != face.revision
            || self.plan_id != plan.plan_id
            || self.presenter_capability_id != placement.capability_id
            || self.presenter_implementation_id != placement.implementation_id
            || self.presenter_artifact_id != placement.artifact_id
            || self.host_id != placement.host_id
            || self.boot_id != placement.boot_id
            || self.offer_generation != placement.offer_generation
            || !active_play_matches(
                self.active_play_id.as_str(),
                plan.plan_id.as_str(),
                placement.host_id.as_str(),
                placement.boot_id.as_str(),
                self.play_sequence,
            )
            || self.manifestation_id
                != bind_manifestation(
                    &face.identity,
                    &plan.plan_id,
                    &self.active_play_id,
                    placement,
                    &self.front_subject,
                    &self.target_subject,
                )
        {
            return Err(ManifestationError::StaleIdentity);
        }
        validate_target(&self.front_subject)?;
        validate_front_subject(face, &self.front_subject)?;
        validate_target(&self.target_subject)?;
        if (self.lifecycle == ManifestationLifecycle::Failed) != self.failure.is_some()
            || !self.valid_signs()
        {
            return Err(ManifestationError::InvalidTransition);
        }
        Ok(())
    }
}

// The existing public digest avoids constructing and discarding an owned
// ActivePlayIdentity (including clones of Plan/Host/Boot) during validation.
fn active_play_matches(identity: &str, plan: &str, host: &str, boot: &str, sequence: u64) -> bool {
    let digest = conduit_core::active_play_digest(plan, host, boot, sequence);
    let identity = identity.as_bytes();
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    identity.len() == 64
        && digest.iter().enumerate().all(|(index, byte)| {
            identity[index * 2] == DIGITS[(byte >> 4) as usize]
                && identity[index * 2 + 1] == DIGITS[(byte & 15) as usize]
        })
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
