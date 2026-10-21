//! Retained owner window for one exact acknowledged Show and prepared transport.
use super::{dispatch_to_owner, PatchbayIngressDispatch, PreparedPatchbayIngress};
use conduit_body::BodyId;
use conduit_face::{Face, FaceContributionBasis, ManifestationLifecycle, MaskShow};
use patchbay_application::PatchbayApplicationPort;

/// The installing owner moves its current Face/Show here after acknowledgement.
/// This retains presentation/admission evidence, never a second domain model.
/// Replace/retire the old turn before installing its successor. Binding alone
/// does not install this turn in a Body Play or account for parent resources.
pub struct PatchbayOwnerTurn {
    body: BodyId,
    basis: FaceContributionBasis,
    face: Face,
    show: MaskShow,
    ingress: PreparedPatchbayIngress,
    receipt: Option<PatchbayIngressDispatch>,
    retired: bool,
}

impl PatchbayOwnerTurn {
    pub fn bind(
        body: BodyId,
        basis: FaceContributionBasis,
        face: Face,
        show: MaskShow,
        ingress: PreparedPatchbayIngress,
    ) -> Result<Self, String> {
        face.validate()
            .map_err(|error| format!("Patchbay owner Face: {error:?}"))?;
        show.validate(&face)
            .map_err(|error| format!("Patchbay owner Show: {error:?}"))?;
        if !patchbay_application::patchbay_face_contribution_matches(&face, &basis)
            || face.basis.body_id.as_ref() != Some(&body)
            || show.show.lifecycle != ManifestationLifecycle::Available
            || basis
                .required_interaction_context
                .as_ref()
                .is_some_and(|context| *context != face.interaction_context.identity)
            || !ingress.is_ready()
        {
            return Err(
                "Patchbay owner turn requires current acknowledged evidence and unused ingress"
                    .into(),
            );
        }
        Ok(Self {
            body,
            basis,
            face,
            show,
            ingress,
            receipt: None,
            retired: false,
        })
    }

    pub fn face(&self) -> &Face {
        &self.face
    }
    pub fn show(&self) -> &MaskShow {
        &self.show
    }
    pub fn can_submit(&self) -> bool {
        !self.retired && self.ingress.is_ready()
    }
    pub fn receipt(&self) -> Option<&PatchbayIngressDispatch> {
        self.receipt.as_ref()
    }

    /// Store the full typed outcome before returning to the installing owner.
    /// Even ignored return values cannot drop a successful Edit/Mask request or
    /// overwrite it with another action. Admission failures before dispatch do
    /// not pretend that the owner callback ran.
    pub fn submit(
        &mut self,
        owner: &mut PatchbayApplicationPort,
        bytes: &[u8],
    ) -> Result<&PatchbayIngressDispatch, String> {
        if !self.can_submit() || self.receipt.is_some() {
            return Err("Patchbay owner turn is consumed or retired".into());
        }
        self.ingress.admit(bytes)?;
        let receipt = dispatch_to_owner(
            &mut self.ingress,
            owner,
            &self.body,
            &self.basis,
            &self.face,
            &self.show,
        )?;
        self.receipt = Some(receipt);
        Ok(self
            .receipt
            .as_ref()
            .expect("stored owner dispatch receipt"))
    }

    /// Transfer result ownership once. This does not reopen the consumed turn.
    #[must_use = "retain or handle the owner request and transport outcome"]
    pub fn take_receipt(&mut self) -> Option<PatchbayIngressDispatch> {
        self.receipt.take()
    }

    /// Revoke interaction on replacement/retirement, retaining any prior result.
    pub fn retire(&mut self) -> Result<(), String> {
        self.retired = true;
        self.ingress.retire()
    }
}
