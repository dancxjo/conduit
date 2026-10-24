//! Expected stale submissions retire their old authority and recover visibly.
use super::*;

impl FaceWorkspace {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn accept_submission(
        &mut self,
        door: &mut FrontDoor,
        journey: &mut ProductJourney,
        display: &mut impl PixelTarget,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        make: &MakeRecord,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<CompositionReceipt, &'static str> {
        // submit consumes the interaction session, but its exact source scene
        // remains available until a replacement has actually been presented.
        let source = self
            .mask
            .scene()
            .ok_or("workspace-face-scene-absent")?
            .face()
            .clone();
        let before = journey.projection();
        let reason = match self.apply_interaction(
            door,
            journey,
            display,
            identities,
            offer,
            make,
            show,
            interaction,
        ) {
            Ok(receipt) => return Ok(receipt),
            Err(
                reason @ ("workspace-face-stale-face"
                | "workspace-face-stale-show"
                | "workspace-face-owner-changed"
                | "workspace-face-show-not-selected"),
            ) if journey.projection() == before => reason,
            Err(reason) => return Err(reason),
        };
        // These are pre-request admission failures. Do not replay, rebase, or
        // treat any domain failure or changed owner projection as zero effects.
        door.observe_product(journey)
            .map_err(|error| error.as_str())?;
        let current = door.face().map_err(|error| error.as_str())?;
        super::super::native_action_evidence::refused_interaction(
            &source,
            show,
            interaction,
            &current,
            &before,
            reason,
        )?;
        door.action_refused(reason)
            .map_err(|error| error.as_str())?;
        self.present_current(door, display)
    }

    pub(super) fn restore_refusal_notice(
        &mut self,
        receipt: CompositionReceipt,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, &'static str> {
        let notice = self.mask.scene().and_then(|scene| {
            scene.face().properties.iter().find_map(|property| {
                if property.name == "action-refusal"
                    && let conduit_face::FacePropertyValue::Text(text) = &property.value
                {
                    Some(text.clone())
                } else {
                    None
                }
            })
        });
        if let Some(notice) = notice {
            self.mask
                .show_local_notice(&notice, display)
                .map_err(|error| error.as_str())
        } else {
            Ok(receipt)
        }
    }
}
