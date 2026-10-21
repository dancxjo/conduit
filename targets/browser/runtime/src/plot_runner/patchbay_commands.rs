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
