//! One bounded owner Face and return-grant refresh before native Show acknowledgement.

use super::*;

const REFRESH_SCHEMA: &str = "conduit.body/native-owner-return-refresh@1";
const REFRESH_RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-refresh-response@1";

#[derive(Serialize)]
struct Refresh {
    schema: &'static str,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshResponse {
    schema: String,
    face: OwnerFaceSnapshotResponse,
    grant: Option<NativeOwnerReturnGrant>,
}

impl NativeOwnerReturnRoute {
    /// Refresh the exact owner Face before acknowledging a Show. An uncertain
    /// response retires this route; it cannot turn an old Show into authority.
    pub fn refresh(
        &mut self,
        identities: BootIdentities,
        prior: &NativeGuestFace,
    ) -> Result<NativeGuestFace, &'static str> {
        if !self.available() || self.acknowledged_show.is_some() || self.refreshes >= 2 {
            return Err("native-owner-refresh-unavailable");
        }
        self.active = false;
        self.refreshes += 1;
        let deadline = CandidateDeadline::admit(MAX_ACTION_MILLIS)
            .ok_or("native-owner-refresh-clock-unavailable")?;
        let mut endpoint = self.endpoint;
        endpoint.local_port = endpoint
            .local_port
            .checked_add(9 + u16::from(self.refreshes))
            .ok_or("native-owner-refresh-port-bound")?;
        let seeds = crate::native_boot_join::fresh_seeds()?;
        let device = self
            .device
            .take()
            .ok_or("native-owner-refresh-device-unavailable")?;
        if device.identity().boot_id != identities.boot {
            return Err("native-owner-refresh-stale-boot");
        }
        let mut request = owner_request(&self.receipt);
        request.last_seen_revision = Some(prior.presentation().revision);
        request.last_seen_identity = Some(prior.presentation().identity.clone());
        let length = serde_json_core::to_slice(
            &Refresh {
                schema: REFRESH_SCHEMA,
                token: self.grant.token,
                sequence: 0,
                request,
            },
            &mut self.action_storage,
        )
        .map_err(|_| "native-owner-refresh-pressure")?;
        if length == 0 || length > MAX_RETURN_ACTION_BYTES {
            return Err("native-owner-refresh-pressure");
        }
        let (response, _, device) = virtio_tls::with_websocket_deadline_retain_device(
            device,
            seeds.tcp,
            seeds.tls,
            seeds.websocket,
            endpoint,
            &self.server_identity,
            &self.certificate_der,
            MAXIMUM_POLLS,
            Some(deadline),
            |line| {
                exchange_refresh(
                    line,
                    &self.action_storage[..length],
                    &mut self.frame_storage,
                )
            },
        )
        .map_err(|_| "native-owner-refresh-outcome-unknown")?;
        self.device = Some(device);
        let fresh = validated_refresh(&self.receipt, prior, response)?;
        self.grant = fresh.1;
        self.lifetime = CandidateDeadline::admit(self.grant.remaining_millis)
            .ok_or("native-owner-refresh-clock-unavailable")?;
        self.active = true;
        Ok(fresh.0)
    }
}

fn exchange_refresh(
    line: &mut dyn BinaryWebSocketIo,
    encoded: &[u8],
    frame: &mut [u8],
) -> Result<RefreshResponse, &'static str> {
    send_chunks(line, encoded, frame)?;
    native_owner_document::receive(line, MAX_RETURN_ACTION_BYTES)
        .map_err(|_| "native-owner-refresh-response-invalid")
}

fn validated_refresh(
    receipt: &PortableAdmissionReceipt,
    prior: &NativeGuestFace,
    response: RefreshResponse,
) -> Result<(NativeGuestFace, NativeOwnerReturnGrant), &'static str> {
    if response.schema != REFRESH_RESPONSE_SCHEMA {
        return Err("native-owner-refresh-schema-invalid");
    }
    match &response.face {
        OwnerFaceSnapshotResponse::Refused { .. } => {
            return Err("native-owner-refresh-owner-refused");
        }
        OwnerFaceSnapshotResponse::Unchanged { .. } => {
            return Err("native-owner-refresh-unchanged");
        }
        OwnerFaceSnapshotResponse::Snapshot { .. } => {}
    }
    let grant = response.grant.ok_or("native-owner-refresh-grant-absent")?;
    if !grant.matches_receipt(receipt) {
        return Err("native-owner-refresh-grant-invalid");
    }
    let face = NativeGuestFace::from_owner_response(receipt, response.face)
        .map_err(|_| "native-owner-refresh-face-invalid")?;
    if !face.interactions_admitted()
        || face.presentation().revision < prior.presentation().revision
        || (face.presentation().revision == prior.presentation().revision
            && face.presentation().identity != prior.presentation().identity)
        || face.route().map(|route| &route.planned_mask)
            != prior.route().map(|route| &route.planned_mask)
    {
        return Err("native-owner-refresh-face-stale-or-route-changed");
    }
    Ok((face, grant))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use conduit_body::{Body, MembershipCredential, SPAWN_ADMISSION_RECEIPT_SCHEMA};
    use conduit_core::{OfferGeneration, PROTOCOL_VERSION, SignId};
    use conduit_presentation::{Face, FaceContext, FaceFocus, OWNER_FACE_RESPONSE_SCHEMA};

    #[test]
    fn refresh_requires_exact_credential_and_admitted_current_face() {
        let body = Body::born(
            "source/native-refresh".into(),
            "checked/native-refresh".into(),
            1,
            SignId::from("sign/native-refresh-born"),
        )
        .unwrap();
        let advertisement = conduit_core::HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: "host/native-refresh".into(),
            boot_id: "boot/native-refresh".into(),
            offer_generation: OfferGeneration(1),
            profile: "profile/native-refresh".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        let credential = MembershipCredential {
            credential_id: serde_json::from_str("\"credential/native-refresh\"").unwrap(),
            body_id: body.body_id.clone(),
            part_id: serde_json::from_str("\"part/native-refresh\"").unwrap(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            issued_at_millis: 1,
        };
        let receipt = PortableAdmissionReceipt {
            schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential: credential.clone(),
            host_advertisement: advertisement,
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
        };
        let presentation = Face::project(
            &body,
            None,
            7,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .presentation;
        let snapshot = || OwnerFaceSnapshotResponse::Snapshot {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            presentation: Box::new(presentation.clone()),
            interactions_admitted: false,
            route: None,
        };
        let prior = NativeGuestFace::from_owner_response(&receipt, snapshot()).unwrap();
        let grant = NativeOwnerReturnGrant {
            schema: "conduit.body/native-owner-return-grant@1".into(),
            token: [7; 32],
            credential_id: credential.credential_id.as_str().into(),
            body_id: credential.body_id.as_str().into(),
            part_id: credential.part_id.as_str().into(),
            host_id: credential.host_id.as_str().into(),
            boot_id: credential.boot_id.as_str().into(),
            remaining_millis: 60_000,
            maximum_actions: 4,
        };
        assert!(matches!(
            validated_refresh(
                &receipt,
                &prior,
                RefreshResponse {
                    schema: REFRESH_RESPONSE_SCHEMA.into(),
                    face: OwnerFaceSnapshotResponse::Refused {
                        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                        code: "selected-native-route-unavailable".into(),
                    },
                    grant: None,
                },
            ),
            Err("native-owner-refresh-owner-refused")
        ));
        assert!(matches!(
            validated_refresh(
                &receipt,
                &prior,
                RefreshResponse {
                    schema: REFRESH_RESPONSE_SCHEMA.into(),
                    face: snapshot(),
                    grant: None,
                },
            ),
            Err("native-owner-refresh-grant-absent")
        ));
        let mut foreign = grant.clone();
        foreign.boot_id = "boot/foreign".into();
        assert!(matches!(
            validated_refresh(
                &receipt,
                &prior,
                RefreshResponse {
                    schema: REFRESH_RESPONSE_SCHEMA.into(),
                    face: snapshot(),
                    grant: Some(foreign),
                },
            ),
            Err("native-owner-refresh-grant-invalid")
        ));
        assert!(matches!(
            validated_refresh(
                &receipt,
                &prior,
                RefreshResponse {
                    schema: REFRESH_RESPONSE_SCHEMA.into(),
                    face: snapshot(),
                    grant: Some(grant),
                },
            ),
            Err("native-owner-refresh-face-stale-or-route-changed")
        ));
    }
}
