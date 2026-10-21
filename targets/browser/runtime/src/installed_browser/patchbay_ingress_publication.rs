//! A prepared browser Mask and its exact canonical owner turn.
use super::PatchbayIngressOwner;
use super::{PatchbayIngressDispatch, PatchbayOwnerTurn, PreparedPatchbayIngress};
pub use crate::workspace_mask::{
    BrowserMaskAcknowledgement, BrowserMaskInteraction, BrowserMaskObservation, BrowserMaskRuntime,
};
use conduit_face::{FaceContributionBasis, FaceInteraction};

/// The caller prepares/charges both Plays and publishes the Mask's real effect.
/// This owns the presentation evidence and transport, never the domain model.
pub struct PatchbayPublication {
    mask: BrowserMaskRuntime,
    ingress: Option<PreparedPatchbayIngress>,
    basis: FaceContributionBasis,
    turn: Option<PatchbayOwnerTurn>,
    retired: bool,
}

impl PatchbayPublication {
    pub fn prepare(
        mask: BrowserMaskRuntime,
        ingress: PreparedPatchbayIngress,
        basis: FaceContributionBasis,
    ) -> Result<Self, String> {
        let observed = mask.observation();
        if !ingress.is_ready()
            || observed.mask_show.show.lifecycle != conduit_face::ManifestationLifecycle::Prepared
            || observed.face.basis.body_id.as_ref() != Some(&observed.body_plan.body_id)
            || observed.body_plan.plan_id != basis.plan_id
            || basis
                .required_interaction_context
                .as_ref()
                .is_some_and(|context| *context != observed.face.interaction_context.identity)
            || !patchbay_application::patchbay_face_contribution_matches(&observed.face, &basis)
        {
            return Err(
                "Patchbay publication requires a prepared Mask, owner basis and unused ingress"
                    .into(),
            );
        }
        Ok(Self {
            mask,
            ingress: Some(ingress),
            basis,
            turn: None,
            retired: false,
        })
    }

    pub fn observation(&self) -> BrowserMaskObservation {
        self.mask.observation()
    }
    pub fn receipt(&self) -> Option<&PatchbayIngressDispatch> {
        self.turn.as_ref().and_then(|turn| turn.receipt())
    }
    pub fn can_submit(&self) -> bool {
        !self.retired && self.turn.as_ref().is_some_and(|turn| turn.can_submit())
    }

    /// The actual Host reports the full exact effect identity; never synthesize
    /// acknowledgement merely because a Face or Plan is available.
    pub fn acknowledge(
        &mut self,
        acknowledgement: &BrowserMaskAcknowledgement,
    ) -> Result<(), String> {
        if self.retired || self.turn.is_some() {
            return Err("Patchbay publication is retired or already acknowledged".into());
        }
        self.mask.acknowledge(acknowledgement)?;
        let observed = self.mask.observation();
        self.turn = Some(PatchbayOwnerTurn::bind(
            observed.body_plan.body_id,
            self.basis.clone(),
            observed.face,
            observed.mask_show,
            self.ingress
                .take()
                .ok_or("Patchbay publication has no prepared ingress")?,
        )?);
        Ok(())
    }

    pub fn interact(
        &mut self,
        owner: &mut impl PatchbayIngressOwner,
        proposed: &BrowserMaskInteraction,
    ) -> Result<&PatchbayIngressDispatch, String> {
        if !self.can_submit() {
            return Err("Patchbay publication has no live acknowledged owner turn".into());
        }
        let current = self.mask.observation();
        // Refuse invalid owner arguments/revision before consuming the Mask's
        // one closing interaction Fore. The emitted bytes are rechecked below.
        let checked = FaceInteraction::new(
            &current.face,
            &current.mask_show,
            &proposed.action_id,
            &proposed.target,
            proposed.arguments.clone(),
            proposed.sequence,
        )
        .map_err(|error| format!("Patchbay interaction: {error:?}"))?;
        let body = current
            .face
            .basis
            .body_id
            .as_ref()
            .ok_or("Patchbay Face has no Body")?;
        owner
            .port()?
            .admit_face_command(
                body,
                self.basis.clone(),
                &current.face,
                &current.mask_show,
                &checked,
            )
            .map_err(|error| format!("Patchbay owner admission: {error:?}"))?;
        let emitted = self.mask.interact(proposed)?;
        self.turn
            .as_mut()
            .ok_or("Patchbay owner turn absent")?
            .submit(owner, &emitted.correlation.interaction.encode())
    }

    #[must_use = "retain or handle the typed owner request and transport outcome"]
    pub fn take_receipt(&mut self) -> Option<PatchbayIngressDispatch> {
        self.turn.as_mut().and_then(|turn| turn.take_receipt())
    }

    /// Revoke the return route, retaining any result. This does not claim that
    /// the Host has erased the pixels or cancelled an already returned request.
    pub fn retire(&mut self) -> Result<(), String> {
        self.retired = true;
        if let Some(turn) = &mut self.turn {
            turn.retire()?;
        }
        if let Some(ingress) = &mut self.ingress {
            ingress.retire()?;
        }
        Ok(())
    }
}
