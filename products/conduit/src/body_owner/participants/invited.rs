//! Consume an exact portable invitation request at the running Body owner.
//! The caller must independently authorize the Host and supply a real carrier;
//! this operation performs membership only, not a Line, Plan, or remote Play.
use super::{state, Owner};
use conduit_body::{
    AdmissionManager, BodyState, PortableAdmissionReceipt, PortableInvitation,
    PortableSpawnAdmissionRequest, SPAWN_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::{HostId, PROTOCOL_VERSION};
use std::path::Path;

impl Owner {
    pub(crate) fn issue_invitation(
        &mut self,
        root: &Path,
        ttl_seconds: u64,
    ) -> Result<PortableInvitation, String> {
        if self.session.evidence().body.state != BodyState::Lulled
            || self.session.realization().is_some()
        {
            return Err("invitation requires a lulled Body without a proposal".into());
        }
        let mut manager = match &self.admissions {
            Some(manager) => manager.clone(),
            None => AdmissionManager::new(self.session.evidence().body_id.clone())
                .map_err(|error| format!("initialize Body admission: {error:?}"))?,
        };
        let invitation =
            super::super::super::invitation::issue_from_manager(&mut manager, ttl_seconds, None)?;
        state::retain(
            root,
            self.session.evidence(),
            self.last_execution.as_ref(),
            Some(&manager),
        )?;
        self.admissions = Some(manager);
        Ok(invitation)
    }

    pub(crate) fn admit_invited(
        &mut self,
        root: &Path,
        request: PortableSpawnAdmissionRequest,
        expected_host_id: &str,
    ) -> Result<PortableAdmissionReceipt, String> {
        if self.session.evidence().body.state != BodyState::Lulled
            || self.session.realization().is_some()
        {
            return Err("invited admission requires a lulled Body without a proposal".into());
        }
        if expected_host_id.is_empty() || expected_host_id.len() > 256 {
            return Err("invited admission requires an exact authorized Host".into());
        }
        request.validate().map_err(|error| error.to_string())?;
        let authority = self.host.advertisement();
        if request.body_id != self.session.evidence().body_id
            || request.host_advertisement.host_id != HostId::from(expected_host_id)
            || request.host_advertisement.host_id == authority.host_id
        {
            return Err("invited request differs from Body or authorized Host".into());
        }
        if request.host_advertisement.protocol_version != PROTOCOL_VERSION
            || request.host_advertisement.boot_id.as_str().is_empty()
            || request.host_advertisement.offer_generation.0 == 0
        {
            return Err("invited Host advertisement is invalid".into());
        }
        let proof = request
            .admission_proof()
            .map_err(|error| error.to_string())?;
        let mut manager = self
            .admissions
            .clone()
            .ok_or("owner has no retained invitation authority")?;
        let already_admitted = manager
            .receipts
            .iter()
            .any(|receipt| receipt.credential.host_id == request.host_advertisement.host_id);
        let mut session = self.session.clone();
        let credential = session
            .admit_invited_host(
                &mut manager,
                &request.host_advertisement,
                &proof,
                super::super::super::current_time_millis()?,
                &authority.host_id,
                &authority.boot_id,
            )
            .map_err(|error| format!("invited Body admission refused: {error:?}"))?;
        if already_admitted {
            return Err("invited Host already has an admitted Part; use return admission".into());
        }
        let receipt = PortableAdmissionReceipt {
            schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential,
            host_advertisement: request.host_advertisement.clone(),
            membership_admitted: true,
            current_offers_available: !request.host_advertisement.capabilities.is_empty(),
            plan_created: false,
            play_created: false,
        };
        receipt
            .validate_against(&request)
            .map_err(|error| error.to_string())?;
        // Persist both consumed invitation and biography before a receipt can
        // escape. The owner must never acknowledge a non-durable Part.
        state::retain(
            root,
            session.evidence(),
            self.last_execution.as_ref(),
            Some(&manager),
        )?;
        self.session = session;
        self.admissions = Some(manager);
        Ok(receipt)
    }
}
