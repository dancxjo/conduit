//! One bounded owner Face and return-grant refresh before native Show acknowledgement.

use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

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

#[derive(Clone, Copy)]
enum PresentationSelection<'a> {
    Todo(
        &'a MaskShow,
        conduit_face::ComponentMaskLook,
        conduit_face::TodoComponentRecipe,
    ),
    History(&'a MaskShow, &'a conduit_face::LookInfo, &'a [u8]),
}

#[derive(Serialize)]
struct HistorySelection<'a> {
    schema: &'static str,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: &'a MaskShow,
    look: &'a [u8],
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
        self.refresh_with_selection(
            identities,
            prior,
            Some(PresentationSelection::Todo(show, look, recipe)),
        )
    }

    /// Select a complete caller-owned Look. Its checked canonical bytes are
    /// transport data; the returned routed Plan is reproduced independently.
    pub fn select_history(
        &mut self,
        identities: BootIdentities,
        prior: &NativeGuestFace,
        show: &MaskShow,
        look: &conduit_face::LookInfo,
    ) -> Result<NativeGuestFace, &'static str> {
        let bytes = look
            .clone()
            .encode()
            .map_err(|_| "native-owner-history-look-invalid")?;
        if bytes.len() > conduit_face::MAX_LOOK_BYTES {
            return Err("native-owner-history-look-pressure");
        }
        self.refresh_with_selection(
            identities,
            prior,
            Some(PresentationSelection::History(show, look, &bytes)),
        )
    }

    fn refresh_with_selection(
        &mut self,
        identities: BootIdentities,
        prior: &NativeGuestFace,
        selection: Option<PresentationSelection<'_>>,
    ) -> Result<NativeGuestFace, &'static str> {
        let show_valid = match selection {
            Some(
                PresentationSelection::Todo(show, _, _)
                | PresentationSelection::History(show, _, _),
            ) => self.acknowledged_show.as_ref() == Some(show),
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
        let length = if let Some(PresentationSelection::Todo(show, look, recipe)) = selection {
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
        } else if let Some(PresentationSelection::History(show, _, look)) = selection {
            serde_json_core::to_slice(
                &HistorySelection {
                    schema: "conduit.body/native-owner-history-selection@1",
                    token: self.grant.token,
                    sequence: 0,
                    request,
                    show,
                    look,
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
        let fresh = match selection {
            Some(PresentationSelection::Todo(_, look, recipe)) => {
                validated_component_selection(&self.receipt, prior, response, look, recipe)?
            }
            Some(PresentationSelection::History(_, look, _)) => {
                validated_history_selection(&self.receipt, prior, response, look)?
            }
            None => validated_refresh(&self.receipt, prior, response)?,
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
    let face = face
        .retain_history_look_from(prior)
        .map_err(|_| "native-owner-history-plan-invalid")?;
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

fn validated_history_selection(
    receipt: &PortableAdmissionReceipt,
    prior: &NativeGuestFace,
    response: RefreshResponse,
    look: &conduit_face::LookInfo,
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
    if !face.interactions_admitted() || face.face() != prior.face() {
        return Err("native-owner-history-face-changed");
    }
    let face = face
        .with_history_look(receipt, look)
        .map_err(|_| "native-owner-history-plan-invalid")?;
    Ok((face, grant))
}

#[cfg(test)]
#[path = "refresh/tests.rs"]
mod tests;

#[cfg(all(test, feature = "native-compositor-core"))]
#[path = "refresh/history_tests.rs"]
mod history_tests;
