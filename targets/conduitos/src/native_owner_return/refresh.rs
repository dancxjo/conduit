//! One bounded owner Face and return-grant refresh before native Show acknowledgement.

use super::*;

const MAX_FACE_REFRESHES: u8 = 4;
const REFRESH_SCHEMA: &str = "conduit.body/native-owner-return-refresh@1";
const REFRESH_RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-refresh-response@1";

#[derive(Serialize)]
struct Refresh {
    schema: &'static str,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
}

#[derive(Serialize)]
struct ComponentSelection<'a> {
    schema: &'static str,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: &'a MaskShow,
    look: conduit_face::ComponentMaskLook,
    recipe: &'static str,
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
        self.refresh_with_selection(identities, prior, None)
    }

    /// Replace appearance through the owner while preserving the exact Face.
    /// The returned Face has no local Show acknowledgement yet.
    pub fn select_component(
        &mut self,
        identities: BootIdentities,
        prior: &NativeGuestFace,
        show: &MaskShow,
        look: conduit_face::ComponentMaskLook,
        recipe: conduit_face::TodoComponentRecipe,
    ) -> Result<NativeGuestFace, &'static str> {
        self.refresh_with_selection(identities, prior, Some((show, look, recipe)))
    }

    fn refresh_with_selection(
        &mut self,
        identities: BootIdentities,
        prior: &NativeGuestFace,
        selection: Option<(
            &MaskShow,
            conduit_face::ComponentMaskLook,
            conduit_face::TodoComponentRecipe,
        )>,
    ) -> Result<NativeGuestFace, &'static str> {
        let show_valid = match selection {
            Some((show, _, _)) => self.acknowledged_show.as_ref() == Some(show),
            None => self.acknowledged_show.is_none(),
        };
        if !self.available() || !show_valid || self.refreshes >= MAX_FACE_REFRESHES {
            return Err("native-owner-refresh-unavailable");
        }
        self.active = false;
        self.refreshes += 1;
        let deadline = CandidateDeadline::admit(MAX_ACTION_MILLIS)
            .ok_or("native-owner-refresh-clock-unavailable")?;
        let endpoint = next_return_endpoint(&mut self.endpoint)?;
        let seeds = crate::native_boot_join::fresh_seeds()?;
        let device = self
            .device
            .take()
            .ok_or("native-owner-refresh-device-unavailable")?;
        if device.identity().boot_id != identities.boot {
            return Err("native-owner-refresh-stale-boot");
        }
        let mut request = owner_request(&self.receipt);
        request.last_seen_revision = Some(prior.face().revision);
        request.last_seen_identity = Some(prior.face().identity.clone());
        let length = if let Some((show, look, recipe)) = selection {
            let recipe = match recipe {
                conduit_face::TodoComponentRecipe::Flat => "flat",
                conduit_face::TodoComponentRecipe::Grouped => "grouped",
                conduit_face::TodoComponentRecipe::RequiredItem => "required-item",
            };
            serde_json_core::to_slice(
                &ComponentSelection {
                    schema: "conduit.body/native-owner-component-selection@1",
                    token: self.grant.token,
                    sequence: 0,
                    request,
                    show,
                    look,
                    recipe,
                },
                &mut self.action_storage,
            )
        } else {
            serde_json_core::to_slice(
                &Refresh {
                    schema: REFRESH_SCHEMA,
                    token: self.grant.token,
                    sequence: 0,
                    request,
                },
                &mut self.action_storage,
            )
        }
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
                    conduit_conduitos_mask_offer::NativeMaskStorageProfile::owner_document_profile(
                        &self.receipt.host_advertisement,
                    ),
                )
            },
        )
        .map_err(|_| "native-owner-refresh-outcome-unknown")?;
        self.device = Some(device);
        let fresh = if let Some((_, look, recipe)) = selection {
            validated_component_selection(&self.receipt, prior, response, look, recipe)?
        } else {
            validated_refresh(&self.receipt, prior, response)?
        };
        if fresh.1.token == self.grant.token {
            return Err("native-owner-refresh-bearer-not-replaced");
        }
        self.acknowledged_show = None;
        self.grant = fresh.1;
        self.next_sequence = 1;
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
    profile: conduit_conduitos_mask_offer::NativeMaskStorageProfile,
) -> Result<RefreshResponse, &'static str> {
    send_chunks(line, encoded, frame)?;
    native_owner_document::receive_for_profile(line, profile)
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
        || face.face().revision < prior.face().revision
        || (face.face().revision == prior.face().revision
            && face.face().identity != prior.face().identity)
        || face.route().map(|route| &route.planned_mask)
            != prior.route().map(|route| &route.planned_mask)
    {
        return Err("native-owner-refresh-face-stale-or-route-changed");
    }
    Ok((face, grant))
}

fn validated_component_selection(
    receipt: &PortableAdmissionReceipt,
    prior: &NativeGuestFace,
    response: RefreshResponse,
    look: conduit_face::ComponentMaskLook,
    recipe: conduit_face::TodoComponentRecipe,
) -> Result<(NativeGuestFace, NativeOwnerReturnGrant), &'static str> {
    if response.schema != REFRESH_RESPONSE_SCHEMA {
        return Err("native-owner-refresh-schema-invalid");
    }
    let grant = response.grant.ok_or("native-owner-refresh-grant-absent")?;
    if !grant.matches_receipt(receipt) {
        return Err("native-owner-refresh-grant-invalid");
    }
    let face = NativeGuestFace::from_owner_response(receipt, response.face)
        .map_err(|_| "native-owner-refresh-face-invalid")?;
    let (expected, _) = conduit_conduitos_mask_offer::prepare_component_stage_from_offer(
        &receipt.host_advertisement,
        look,
        recipe,
    )
    .map_err(|_| "native-owner-component-plan-invalid")?;
    if !face.interactions_admitted()
        || face.face() != prior.face()
        || face.route().map(|route| &route.planned_mask) != Some(&expected.planned_mask)
    {
        return Err("native-owner-component-face-or-plan-changed");
    }
    Ok((face, grant))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use conduit_body::{Body, MembershipCredential, SPAWN_ADMISSION_RECEIPT_SCHEMA};
    use conduit_core::{OfferGeneration, PROTOCOL_VERSION, SignId};
    use conduit_face::{BodyFace, FaceContext, FaceFocus, OWNER_FACE_RESPONSE_SCHEMA};

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
        let face = BodyFace::project(
            &body,
            None,
            7,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .face;
        let snapshot = || OwnerFaceSnapshotResponse::Snapshot {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            face: Box::new(face.clone()),
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
