//! Explicit Host acknowledgement and typed input for the prepared resident route.
use super::TourSession;
use crate::patchbay_ingress::{BrowserMaskAcknowledgement, BrowserMaskInteraction};

impl TourSession {
    pub(super) fn patchbay_acknowledge(
        &mut self,
        acknowledgement: BrowserMaskAcknowledgement,
    ) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("Patchbay Play is retiring".into());
        }
        self.patchbay_publication
            .as_mut()
            .ok_or("no prepared canonical Patchbay publication")?
            .publication
            .acknowledge(&acknowledgement)?;
        self.patchbay_observation()
    }

    pub(super) fn patchbay_interact(
        &mut self,
        input: BrowserMaskInteraction,
    ) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("Patchbay Play is retiring".into());
        }
        let context = self
            .patchbay_face
            .as_ref()
            .ok_or("no current Patchbay owner")?;
        let prepared = self
            .patchbay_publication
            .as_mut()
            .ok_or("no prepared canonical Patchbay publication")?;
        super::engine::application::interact_publication(
            &mut self.scheduler,
            context.owner_node,
            &mut prepared.publication,
            &input,
        )?;
        self.patchbay_observation()
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicationTarget {
    pub body_plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub show_id: String,
    pub face_id: String,
    pub face_revision: u64,
}

impl TourSession {
    /// Explicit preparation of a new closing turn, never a replay of an effect.
    pub(super) fn patchbay_refresh(
        &mut self,
        target: PublicationTarget,
    ) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("Patchbay Play is retiring".into());
        }
        let context = self
            .patchbay_face
            .as_ref()
            .ok_or("no current Patchbay owner")?;
        let old = self
            .patchbay_publication
            .as_ref()
            .ok_or("no current Patchbay publication")?;
        if target.body_plan_id != context.plan.plan_id
            || target.active_play_id != self.active_play_id
            || target.show_id != old.effect.show_id
            || target.face_id != old.effect.face_id
            || target.face_revision != old.effect.face_revision
        {
            return Err("Patchbay replacement target is stale or foreign".into());
        }
        let sequence = old
            .sequence
            .checked_add(1)
            .ok_or("Patchbay publication sequence exhausted")?;
        // Both allocations remain charged until retirement and actual buffer drop.
        let mut next = super::engine::application::prepare_publication(
            &self.scheduler,
            context,
            &self.host_id,
            &self.boot_id,
            &self.active_play_id,
            sequence,
        )?
        .ok_or("captured Patchbay Mask route is unavailable")?;
        let old = self.patchbay_publication.as_mut().expect("checked above");
        old.publication.retire()?;
        next.previous_dispatch = if let Some(receipt) = old.publication.take_receipt() {
            Some(super::engine::application::RetainedPatchbayDispatch {
                show_id: old.effect.show_id.clone(),
                face_id: old.effect.face_id.clone(),
                face_revision: old.effect.face_revision,
                receipt,
            })
        } else {
            old.previous_dispatch.take()
        };
        self.patchbay_publication = Some(next);
        self.patchbay_observation()
    }
}
