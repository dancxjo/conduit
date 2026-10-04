//! Owner selection of a ConduitOS Mask from one admitted native Part and its
//! current surface offer. The caller supplies real directional Line offers;
//! membership and matching identifiers cannot create them.

use super::Owner;
use conduit_body::{PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA};
use conduit_core::LineOffer;
use conduit_presentation::RemoteOwnerMaskRouteSeal;

impl Owner {
    pub(crate) fn seal_native_mask_route(
        &self,
        receipt: &PortableAdmissionReceipt,
        face_line: &LineOffer,
        return_line: &LineOffer,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let credential = &receipt.credential;
        let native_offer = &receipt.host_advertisement;
        let current = self.session.evidence().membership.parts.iter().any(|part| {
            part.part_id == credential.part_id
                && part.current.as_ref().is_some_and(|host| {
                    host.host_id == native_offer.host_id
                        && host.boot_id == native_offer.boot_id
                        && host.offer_generation == native_offer.offer_generation
                })
        });
        let admitted = self.admissions.as_ref().is_some_and(|manager| {
            manager
                .receipts
                .iter()
                .rev()
                .any(|receipt| receipt.credential == *credential)
        });
        if !current
            || !admitted
            || receipt.schema != SPAWN_ADMISSION_RECEIPT_SCHEMA
            || !receipt.membership_admitted
            || receipt.plan_created
            || receipt.play_created
            || credential.body_id != self.session.evidence().body_id
        {
            return Err("native-mask-part-unavailable".into());
        }
        let planned = conduit_conduitos_mask_offer::prepare_stage_from_offer(
            conduit_conduitos_mask_offer::Adapter::Native,
            native_offer,
        )
        .map_err(|error| format!("native-mask-plan-refused:{error:?}"))?
        .planned_mask;
        let face = self.local_face_snapshot()?;
        RemoteOwnerMaskRouteSeal::seal_current(
            &self.session,
            &face,
            self.host.advertisement(),
            native_offer,
            &planned,
            face_line,
            return_line,
        )
        .map_err(|error| format!("native-mask-route-refused:{error:?}"))
    }
}
