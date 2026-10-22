//! A prepared browser Mask and its exact canonical owner turn.
use super::value_budget::PublicationValueLease;
use super::{PatchbayIngressDispatch, PatchbayOwnerTurn, PreparedPatchbayIngress};
use super::{PatchbayIngressOwner, PatchbayValueStorageBudget};
pub use crate::workspace_mask::{
    BrowserMaskAcknowledgement, BrowserMaskInteraction, BrowserMaskObservation, BrowserMaskRuntime,
};
use conduit_face::{FaceContributionBasis, FaceInteraction};

/// Own presentation evidence and the Mask's real effect, never the domain model.
/// Resident publication borrows its already-admitted root transport; detached
/// compatibility fixtures retain their separately charged one-turn ingress.
pub struct PatchbayPublication {
    mask: BrowserMaskRuntime,
    ingress: Option<PreparedPatchbayIngress>,
    basis: FaceContributionBasis,
    turn: Option<PatchbayOwnerTurn>,
    retired: bool,
    resident: bool,
    resident_consumed: bool,
    resident_receipt: Option<PatchbayIngressDispatch>,
    // Declared last: release accounting only after the actual Mask/ingress buffers.
    value_lease: PublicationValueLease,
}

impl PatchbayPublication {
    pub fn prepare(
        mask: BrowserMaskRuntime,
        ingress: PreparedPatchbayIngress,
        basis: FaceContributionBasis,
        value_budget: &PatchbayValueStorageBudget,
    ) -> Result<Self, String> {
        let observed = mask.observation();
        ingress.validate_local_show(&observed.mask_show)?;
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
        let backing = mask
            .reserved_value_storage_bytes()
            .checked_add(ingress.reserved_value_storage_bytes())
            .ok_or("publication value backing overflow")?;
        let value_lease = value_budget.admit(&basis.plan_id, &basis.active_play_id, backing)?;
        Ok(Self {
            mask,
            ingress: Some(ingress),
            basis,
            turn: None,
            retired: false,
            resident: false,
            resident_consumed: false,
            resident_receipt: None,
            value_lease,
        })
    }

    /// The owner transport is already admitted in the resident root Plan.
    /// This preparation charges only the new Mask, never another input kernel.
    pub(crate) fn prepare_resident(
        mask: BrowserMaskRuntime,
        basis: FaceContributionBasis,
        value_budget: &PatchbayValueStorageBudget,
    ) -> Result<Self, String> {
        let observed = mask.observation();
        if observed.mask_show.show.lifecycle != conduit_face::ManifestationLifecycle::Prepared
            || observed.face.basis.body_id.as_ref() != Some(&observed.body_plan.body_id)
            || observed.body_plan.plan_id != basis.plan_id
            || basis
                .required_interaction_context
                .as_ref()
                .is_some_and(|context| *context != observed.face.interaction_context.identity)
            || !patchbay_application::patchbay_face_contribution_matches(&observed.face, &basis)
        {
            return Err("resident Patchbay publication differs from its captured owner".into());
        }
        let value_lease = value_budget.admit(
            &basis.plan_id,
            &basis.active_play_id,
            mask.reserved_value_storage_bytes(),
        )?;
        Ok(Self {
            mask,
            ingress: None,
            basis,
            turn: None,
            retired: false,
            resident: true,
            resident_consumed: false,
            resident_receipt: None,
            value_lease,
        })
    }

    pub(crate) fn is_resident(&self) -> bool {
        self.resident
    }
    pub(crate) fn basis(&self) -> &FaceContributionBasis {
        &self.basis
    }

    /// Consume only this Show; root Fore closure belongs to Play retirement.
    pub(crate) fn emit_resident(
        &mut self,
        owner: &mut impl PatchbayIngressOwner,
        proposed: &BrowserMaskInteraction,
    ) -> Result<Vec<u8>, String> {
        if !self.resident || !self.can_submit() {
            return Err("resident Patchbay publication is unavailable".into());
        }
        let current = self.mask.observation();
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
        self.resident_consumed = true;
        Ok(emitted.correlation.interaction.encode())
    }

    pub(crate) fn record_resident(&mut self, receipt: PatchbayIngressDispatch) {
        self.resident_receipt = Some(receipt);
    }

    pub(crate) fn face_view(&self, admitted: bool) -> crate::owner_face_mask::FaceView {
        self.mask.face_view(admitted && self.can_submit())
    }

    pub fn observation(&self) -> BrowserMaskObservation {
        self.mask.observation()
    }
    pub fn receipt(&self) -> Option<&PatchbayIngressDispatch> {
        self.resident_receipt
            .as_ref()
            .or_else(|| self.turn.as_ref().and_then(|turn| turn.receipt()))
    }
    pub fn can_submit(&self) -> bool {
        !self.retired
            && self.value_lease.is_live()
            && if self.resident {
                !self.resident_consumed
                    && self.mask.observation().mask_show.show.lifecycle
                        == conduit_face::ManifestationLifecycle::Available
            } else {
                self.turn.as_ref().is_some_and(|turn| turn.can_submit())
            }
    }

    /// The actual Host reports the full exact effect identity; never synthesize
    /// acknowledgement merely because a Face or Plan is available.
    pub fn acknowledge(
        &mut self,
        acknowledgement: &BrowserMaskAcknowledgement,
    ) -> Result<(), String> {
        if self.retired || !self.value_lease.is_live() || self.turn.is_some() {
            return Err("Patchbay publication is retired or already acknowledged".into());
        }
        self.mask.acknowledge(acknowledgement)?;
        if self.resident {
            return Ok(());
        }
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
        if self.resident {
            return Err("resident Patchbay interaction requires its admitted root Fore".into());
        }
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
        self.resident_receipt
            .take()
            .or_else(|| self.turn.as_mut().and_then(|turn| turn.take_receipt()))
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
