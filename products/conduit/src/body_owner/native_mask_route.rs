//! Owner selection of a ConduitOS Mask from one admitted native Part and its
//! current surface offer. The caller supplies real directional Line offers;
//! membership and matching identifiers cannot create them.

use super::Owner;
use conduit_body::{
    MembershipCredential, PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::LineOffer;
use conduit_presentation::{MaskShow, OwnerFaceSnapshotRequest, RemoteOwnerMaskRouteSeal};

pub(super) struct NativeMaskRoute {
    credential: MembershipCredential,
    host_offer: conduit_core::HostAdvertisement,
    face_line: LineOffer,
    return_line: LineOffer,
    seal: RemoteOwnerMaskRouteSeal,
    acknowledged_show: Option<MaskShow>,
    expires_at_millis: u64,
}

impl Owner {
    pub(crate) fn seal_native_mask_route(
        &mut self,
        receipt: &PortableAdmissionReceipt,
        face_line: &LineOffer,
        return_line: &LineOffer,
        expires_at_millis: u64,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        if super::super::super::current_time_millis()? >= expires_at_millis {
            return Err("native-mask-return-expired".into());
        }
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
        let seal = RemoteOwnerMaskRouteSeal::seal_current(
            &self.session,
            &face,
            self.host.advertisement(),
            native_offer,
            &planned,
            face_line,
            return_line,
        )
        .map_err(|error| format!("native-mask-route-refused:{error:?}"))?;
        self.pending_native_mask = Some(NativeMaskRoute {
            credential: credential.clone(),
            host_offer: native_offer.clone(),
            face_line: face_line.clone(),
            return_line: return_line.clone(),
            seal: seal.clone(),
            acknowledged_show: None,
            expires_at_millis,
        });
        Ok(seal)
    }

    pub(crate) fn acknowledge_native_mask_show(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.validate_native_mask_route_show(request, show)?;
        let route = self
            .pending_native_mask
            .as_mut()
            .ok_or("native-mask-route-not-selected")?;
        route.acknowledged_show = Some(show.clone());
        Ok(())
    }

    pub(crate) fn validate_native_mask_show(
        &self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.validate_native_mask_route_show(request, show)?;
        let route = self
            .pending_native_mask
            .as_ref()
            .ok_or("native-mask-route-not-selected")?;
        if route.acknowledged_show.as_ref() != Some(show) {
            return Err("native-mask-show-not-acknowledged".into());
        }
        Ok(())
    }

    fn validate_native_mask_route_show(
        &self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        let route = self
            .pending_native_mask
            .as_ref()
            .ok_or("native-mask-route-not-selected")?;
        if super::super::super::current_time_millis()? >= route.expires_at_millis {
            return Err("native-mask-return-expired".into());
        }
        let credential = &route.credential;
        if !request.has_exact_basis()
            || request.credential_id != credential.credential_id.as_str()
            || request.body_id != credential.body_id
            || request.part_id != credential.part_id
            || request.host_id != credential.host_id
            || request.boot_id != credential.boot_id
        {
            return Err("native-mask-route-credential-mismatch".into());
        }
        let face = self.face_snapshot(request)?;
        route
            .seal
            .validate_available_show(
                &self.session,
                &face,
                self.host.advertisement(),
                &route.host_offer,
                &route.face_line,
                &route.return_line,
                show,
            )
            .map_err(|error| format!("native-mask-show-refused:{error:?}"))
    }
}
