//! Bounded pinned TLS exchanges for the existing finite owner return protocol.
use crate::durable_host::routed_join::MaskSelection;
use conduit_body::{
    PortableAdmissionReceipt, PortableInvitation, RendezvousCandidate, RoutedAdmissionRequest,
    RoutedAdmissionResponse, ROUTED_ADMISSION_REQUEST_SCHEMA,
};
use conduit_face::{
    Face, MaskShow, OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse, RemoteOwnerMaskRouteSeal,
    OWNER_FACE_REQUEST_SCHEMA, OWNER_FACE_RESPONSE_SCHEMA,
};
use conduit_std_host::secure_websocket::SecureWebSocketClientLine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[cfg(any(test, all(target_os = "linux", feature = "native-display")))]
#[path = "member_carrier_client.rs"]
pub(super) mod member_carrier_client;

#[path = "admitted_snapshot.rs"]
mod admitted_snapshot;
use admitted_snapshot::AdmittedSnapshot;

const MESSAGE: usize = 64 * 1024;
const ADMISSION: usize = 512 * 1024;
#[derive(Deserialize)]
struct Grant {
    schema: String,
    token: [u8; 32],
    credential_id: String,
    body_id: String,
    part_id: String,
    host_id: String,
    boot_id: String,
    remaining_millis: u32,
    maximum_actions: u8,
}

pub(super) struct OwnerRoute {
    candidate: RendezvousCandidate,
    request: OwnerFaceSnapshotRequest,
    grant: Grant,
    deadline: Instant,
    sequence: u8,
    mask: MaskSelection,
    admitted: AdmittedSnapshot,
}

pub(super) struct Snapshot {
    pub face: Face,
    pub route: RemoteOwnerMaskRouteSeal,
    pub interactions_admitted: bool,
    pub document_bytes: usize,
}

impl OwnerRoute {
    pub(super) fn admit(
        host: &mut super::PresentationHost,
        invitation: PortableInvitation,
        mask: MaskSelection,
    ) -> Result<(Self, Snapshot), String> {
        Self::admit_initialized(
            &mut host.runtime,
            &mut host.status,
            &host.state,
            invitation,
            mask,
        )
    }

    pub(super) fn admit_initialized(
        runtime: &mut crate::durable_host_control::DurableHostRuntime,
        status: &mut super::RuntimeStatus,
        state: &std::path::Path,
        invitation: PortableInvitation,
        mask: MaskSelection,
    ) -> Result<(Self, Snapshot), String> {
        let installation = super::super::read_installation(&state.join("installation.json"))?;
        if installation.joined_body_state.is_some() {
            let encounter = load_member_encounter(state)?;
            return Self::return_initialized(runtime, status, state, encounter, mask);
        }
        let encounter_started = Instant::now();
        let rendezvous = invitation
            .rendezvous
            .as_ref()
            .ok_or("presentation needs a routed invitation")?;
        rendezvous
            .validate(super::super::current_time_millis()?)
            .map_err(|error| format!("presentation rendezvous: {error:?}"))?;
        let candidate = rendezvous
            .candidates
            .first()
            .ok_or("presentation invitation has no route")?
            .clone();
        if candidate.line_family != conduit_body::RendezvousLineFamily::AuthenticatedTlsStream {
            return Err("presentation requires an authenticated TLS route".into());
        }
        let request = super::prepare_initialized_join(runtime, status, state, invitation)?;
        let mut line = connect(
            &candidate,
            Duration::from_millis(u64::from(candidate.attempt_timeout_millis)),
        )?;
        send(
            &mut line,
            &RoutedAdmissionRequest {
                schema: ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
                invitation_id: request.invitation_id.as_str().into(),
                request: request.clone(),
            },
            ADMISSION,
        )?;
        let receipt: PortableAdmissionReceipt =
            match receive::<RoutedAdmissionResponse>(&mut line, ADMISSION)? {
                RoutedAdmissionResponse::Admitted { schema, receipt }
                    if conduit_body::supported_routed_admission_response_schema(&schema) =>
                {
                    *receipt
                }
                RoutedAdmissionResponse::Refused { code, .. } => {
                    return Err(format!("presentation admission refused:{code}"));
                }
                _ => return Err("presentation admission response schema".into()),
            };
        receipt
            .validate_against(&request)
            .map_err(|error| error.to_string())?;
        super::super::membership::complete_body_join_document(receipt.clone(), state, true)?;
        let credential = receipt.credential;
        let request = OwnerFaceSnapshotRequest {
            schema: OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: credential.credential_id.as_str().into(),
            body_id: credential.body_id,
            part_id: credential.part_id,
            host_id: credential.host_id,
            boot_id: credential.boot_id,
            last_seen_revision: None,
            last_seen_identity: None,
        };
        send(
            &mut line,
            &json!({"schema":"conduit.body/native-owner-face-request@1", "request":request, "mask":mask}),
            MESSAGE,
        )?;
        let snapshot = snapshot(document(&mut line)?)?;
        let admitted = AdmittedSnapshot::bind(&snapshot, &request)?;
        let grant: Grant = receive(&mut line, MESSAGE)?;
        validate_grant(&grant, &request)?;
        // Remaining lifetime was sampled by the owner before route preparation
        // and delivery. Anchor conservatively before the exchange, rather than
        // extending its witness by time spent preparing/transmitting the Face.
        let deadline = encounter_started + Duration::from_millis(u64::from(grant.remaining_millis));
        Ok((
            Self {
                candidate,
                request,
                grant,
                deadline,
                sequence: 1,
                mask,
                admitted,
            },
            snapshot,
        ))
    }

    pub(super) fn return_initialized(
        runtime: &mut crate::durable_host_control::DurableHostRuntime,
        status: &mut super::RuntimeStatus,
        state: &std::path::Path,
        encounter: crate::durable_host::routed_join::MemberEncounter,
        mask: MaskSelection,
    ) -> Result<(Self, Snapshot), String> {
        use crate::durable_host::routed_join::MEMBER_ROUTE_SCHEMA;
        let started = Instant::now();
        let installation = super::super::read_installation(&state.join("installation.json"))?;
        let prior = super::super::membership::load_joined_credential(state, &installation)?;
        let actual = runtime.truth().advertisement;
        if encounter.schema != MEMBER_ROUTE_SCHEMA
            || encounter.body_id != prior.body_id
            || encounter.part_id != prior.part_id
            || encounter.host_id != prior.host_id
            || actual.host_id.as_str() != status.host_id
            || actual.boot_id.as_str() != status.boot_id
        {
            return Err("member encounter differs from retained member and actual Boot".into());
        }
        let candidate = encounter.candidate;
        if candidate.line_family != conduit_body::RendezvousLineFamily::AuthenticatedTlsStream
            || candidate.expires_at_millis <= super::super::current_time_millis()?
            || !(1..=30_000).contains(&candidate.attempt_timeout_millis)
            || !(1..=2).contains(&candidate.maximum_attempts)
        {
            return Err("member encounter has no current bounded TLS route".into());
        }
        let (mut line, credential) = bounded_signed_return(state, &prior, &actual, &candidate)?;
        super::super::membership::retain_returned_membership(state, &prior, &credential, &actual)?;
        status.body_id = Some(credential.body_id.as_str().into());
        status.offer_generation = actual.offer_generation.0;
        super::super::write_json_atomic(&state.join("runtime.json"), status)?;
        let request = OwnerFaceSnapshotRequest {
            schema: OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: credential.credential_id.as_str().into(),
            body_id: credential.body_id,
            part_id: credential.part_id,
            host_id: credential.host_id,
            boot_id: credential.boot_id,
            last_seen_revision: None,
            last_seen_identity: None,
        };
        send(
            &mut line,
            &json!({"schema":"conduit.body/native-owner-face-request@1",
            "request":request,"mask":mask}),
            MESSAGE,
        )?;
        let snapshot = snapshot(document(&mut line)?)?;
        let admitted = AdmittedSnapshot::bind(&snapshot, &request)?;
        let grant: Grant = receive(&mut line, MESSAGE)?;
        validate_grant(&grant, &request)?;
        let deadline = started + Duration::from_millis(u64::from(grant.remaining_millis));
        Ok((
            Self {
                candidate,
                request,
                grant,
                deadline,
                sequence: 1,
                mask,
                admitted,
            },
            snapshot,
        ))
    }

    fn connection(&self) -> Result<SecureWebSocketClientLine, String> {
        let left = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or("presentation return expired")?;
        connect(
            &self.candidate,
            left.min(Duration::from_millis(u64::from(
                self.candidate.attempt_timeout_millis,
            ))),
        )
    }

    pub(super) fn acknowledge(&self, show: &MaskShow) -> Result<(), String> {
        match self.acknowledge_status(show)? {
            ShowAcceptance::Accepted => Ok(()),
            ShowAcceptance::Stale => Err("presentation Show is stale".into()),
        }
    }

    pub(super) fn acknowledge_status(&self, show: &MaskShow) -> Result<ShowAcceptance, String> {
        self.admitted.validate_show(show)?;
        let mut line = self.connection()?;
        send_return(
            &mut line,
            &json!({"schema":"conduit.body/native-owner-show-ack@1", "token":self.grant.token,
            "sequence":self.sequence, "request":self.request, "show":show}),
            MESSAGE,
        )?;
        let response: Value = receive(&mut line, MESSAGE)?;
        decode_show_acceptance(&response)
    }

    /// Explicitly withdraw this exact attachment before its finite grant ends.
    /// A failed or lost acknowledgment is not proof that withdrawal committed.
    pub(super) fn leave(&self) -> Result<(), String> {
        let mut line = self.connection()?;
        send_return(
            &mut line,
            &json!({
                "schema":"conduit.body/native-owner-member-leave@1",
                "token":self.grant.token, "request":self.request,
            }),
            MESSAGE,
        )?;
        let response: Value = receive(&mut line, MESSAGE)?;
        if response["schema"] != "conduit.body/native-owner-member-leave-response@1"
            || response["accepted"] != true
        {
            return Err(format!(
                "member leave not acknowledged:{}",
                response["code"]
            ));
        }
        Ok(())
    }

    pub(super) fn refresh(&mut self) -> Result<Snapshot, String> {
        self.select(None)
    }

    pub(super) fn select_spoken(
        &mut self,
        selection: &conduit_std_host::direct_spoken_template_mask::RemoteSpokenTemplateSelection,
    ) -> Result<Snapshot, String> {
        self.select(Some(selection))
    }

    fn select(
        &mut self,
        selection: Option<
            &conduit_std_host::direct_spoken_template_mask::RemoteSpokenTemplateSelection,
        >,
    ) -> Result<Snapshot, String> {
        let mut line = self.connection()?;
        let mut request = json!({"schema":"conduit.body/native-owner-return-refresh@1", "token":self.grant.token,
            "sequence":0, "request":self.request});
        if let Some(selection) = selection {
            request["spoken_selection"] =
                serde_json::to_value(selection).map_err(|error| error.to_string())?;
        } else {
            request["mask"] = json!(self.mask);
        }
        send_return(&mut line, &request, MESSAGE)?;
        let response = document(&mut line)?;
        if response["schema"] != "conduit.body/native-owner-return-refresh-response@1" {
            return Err("presentation refresh response schema".into());
        }
        let mut selected = snapshot(response["face"].clone())?;
        selected.document_bytes = serde_json::to_vec(&response)
            .map_err(|error| error.to_string())?
            .len();
        let grant: Grant =
            serde_json::from_value(response["grant"].clone()).map_err(|error| error.to_string())?;
        validate_grant(&grant, &self.request)?;
        self.deadline = self
            .deadline
            .min(Instant::now() + Duration::from_millis(u64::from(grant.remaining_millis)));
        self.admitted = AdmittedSnapshot::bind(&selected, &self.request)?;
        self.grant = grant;
        self.sequence = 1;
        Ok(selected)
    }

    pub(super) fn action(
        &mut self,
        show: &MaskShow,
        interaction: conduit_face::FaceInteraction,
    ) -> Result<(Value, Option<Snapshot>), String> {
        if self.sequence > self.grant.maximum_actions {
            return Err("presentation action budget exhausted".into());
        }
        self.admitted.validate_show(show)?;
        let mut line = self.connection()?;
        send_return(
            &mut line,
            &json!({"schema":"conduit.body/native-owner-return-action@1", "token":self.grant.token,
            "sequence":self.sequence, "request":self.request, "show":show, "interaction":interaction}),
            MESSAGE,
        )?;
        let response = document(&mut line)?;
        if response["schema"] != "conduit.body/native-owner-return-response@1" {
            return Err("presentation action response schema".into());
        }
        self.sequence += 1;
        let next = bind_action_snapshot(&response, &self.request, &mut self.admitted)?;
        Ok((
            json!({"accepted":response["accepted"],"code":response["code"]}),
            next,
        ))
    }
}

// Called only after this encounter's pinned owner exchange and response schema
// check. Rebind before exposing the new snapshot, just as refresh does. A
// malformed or foreign replacement leaves the prior local scope unchanged.
fn bind_action_snapshot(
    response: &Value,
    request: &OwnerFaceSnapshotRequest,
    admitted: &mut AdmittedSnapshot,
) -> Result<Option<Snapshot>, String> {
    if response["face"].is_null() || response["face"]["route"].is_null() {
        return Ok(None);
    }
    let mut next = snapshot(response["face"].clone())?;
    next.document_bytes = serde_json::to_vec(response)
        .map_err(|e| e.to_string())?
        .len();
    let replacement = AdmittedSnapshot::bind(&next, request)?;
    *admitted = replacement;
    Ok(Some(next))
}

fn validate_grant(grant: &Grant, request: &OwnerFaceSnapshotRequest) -> Result<(), String> {
    if grant.schema != "conduit.body/native-owner-return-grant@1"
        || grant.remaining_millis == 0
        || grant.remaining_millis > 60_000
        || grant.maximum_actions == 0
        || grant.maximum_actions > 4
        || grant.credential_id != request.credential_id
        || grant.body_id != request.body_id.as_str()
        || grant.part_id != request.part_id.as_str()
        || grant.host_id != request.host_id.as_str()
        || grant.boot_id != request.boot_id.as_str()
    {
        return Err("presentation grant basis or finite bounds invalid".into());
    }
    Ok(())
}

fn snapshot(value: Value) -> Result<Snapshot, String> {
    let document_bytes = serde_json::to_vec(&value)
        .map_err(|error| error.to_string())?
        .len();
    match serde_json::from_value(value).map_err(|error| format!("presentation Face:{error}"))? {
        OwnerFaceSnapshotResponse::Snapshot {
            schema,
            face,
            route: Some(route),
            interactions_admitted,
        } if schema == OWNER_FACE_RESPONSE_SCHEMA => Ok(Snapshot {
            face: *face,
            route: *route,
            interactions_admitted,
            document_bytes,
        }),
        OwnerFaceSnapshotResponse::Refused { code, .. } => {
            Err(format!("presentation Face refused:{code}"))
        }
        _ => Err("presentation has no admitted current Mask route".into()),
    }
}
fn connect(
    candidate: &RendezvousCandidate,
    timeout: Duration,
) -> Result<SecureWebSocketClientLine, String> {
    let address = crate::durable_host::routed_join::route_address(&candidate.reachability)?;
    SecureWebSocketClientLine::connect_pinned(
        address,
        &candidate.reachability,
        &candidate.authentication.server_identity,
        candidate.authentication.transport_binding_sha256,
        timeout,
        ADMISSION as u32,
    )
    .map_err(|error| format!("presentation TLS:{error:?}"))
}
fn send(
    line: &mut SecureWebSocketClientLine,
    value: &impl Serialize,
    bound: usize,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > bound {
        return Err("presentation message pressure".into());
    }
    line.send_binary(&bytes)
        .map_err(|error| format!("presentation send:{error:?}"))
}
fn receive<T: for<'de> Deserialize<'de>>(
    line: &mut SecureWebSocketClientLine,
    bound: usize,
) -> Result<T, String> {
    let mut frame = vec![0; bound];
    let length = line
        .receive_binary(&mut frame)
        .map_err(|error| format!("presentation receive:{error:?}"))?;
    serde_json::from_slice(&frame[..length])
        .map_err(|error| format!("presentation message:{error}"))
}
fn document(line: &mut SecureWebSocketClientLine) -> Result<Value, String> {
    let mut frame = vec![0; MESSAGE];
    let mut assembly = conduit_wire::BoundedDocumentAssembly::new(128 * 1024, 32);
    for index in 0..32 {
        let length = line
            .receive_binary(&mut frame)
            .map_err(|error| format!("presentation document:{error:?}"))?;
        let frame = &frame[..length];
        if index == 0 {
            if let Ok(value) = serde_json::from_slice(frame) {
                return Ok(value);
            }
        }
        if assembly
            .push(frame)
            .map_err(|error| format!("presentation chunks:{error:?}"))?
        {
            let (bytes, _) = assembly
                .completed()
                .ok_or("presentation chunks incomplete")?;
            return serde_json::from_slice(bytes).map_err(|error| error.to_string());
        }
    }
    Err("presentation document chunk pressure".into())
}

// The existing native return uses a forty-byte total/offset/SHA-256 envelope;
// owner responses independently use conduit_wire's CBD1 bounded documents.
fn send_return(
    line: &mut SecureWebSocketClientLine,
    value: &impl Serialize,
    bound: usize,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > bound || bound > MESSAGE {
        return Err("presentation return message pressure".into());
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    const PAYLOAD: usize = 8192 - 14 - 40;
    for (index, chunk) in bytes.chunks(PAYLOAD).enumerate() {
        let mut frame = Vec::with_capacity(40 + chunk.len());
        frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        frame.extend_from_slice(&((index * PAYLOAD) as u32).to_le_bytes());
        frame.extend_from_slice(&digest);
        frame.extend_from_slice(chunk);
        line.send_binary(&frame)
            .map_err(|error| format!("presentation return:{error:?}"))?;
    }
    Ok(())
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
impl conduit_std_host::NativeMaskEncounter for OwnerRoute {
    fn validate_current(
        &self,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
        local_offer: &conduit_core::HostAdvertisement,
    ) -> Result<(), String> {
        if self.mask != MaskSelection::Native || self.deadline <= Instant::now() {
            return Err("native owner encounter is not current".into());
        }
        validate_grant(&self.grant, &self.request)?;
        if route.body_id != self.request.body_id
            || face.basis.body_id.as_ref() != Some(&self.request.body_id)
            || local_offer.host_id != self.request.host_id
            || local_offer.boot_id != self.request.boot_id
        {
            return Err("native owner encounter differs from this admitted member".into());
        }
        self.admitted.validate_native(face, route)?;
        route
            .validate_mask_host_offer(local_offer)
            .map_err(|_| "native owner route targets a different current offer".into())
    }
}

#[cfg(test)]
#[path = "action_snapshot_tests.rs"]
mod action_snapshot_tests;

pub(super) fn load_member_encounter(
    state: &std::path::Path,
) -> Result<crate::durable_host::routed_join::MemberEncounter, String> {
    serde_json::from_slice(&super::super::bounded_read(
        &state.join("body/member-encounter.json"),
        16 * 1024,
    )?)
    .map_err(|_| "invalid installed finite member encounter".into())
}

fn signed_return_exchange(
    state: &std::path::Path,
    prior: &conduit_body::MembershipCredential,
    actual: &conduit_core::HostAdvertisement,
    candidate: &RendezvousCandidate,
) -> Result<
    (
        SecureWebSocketClientLine,
        conduit_body::MembershipCredential,
    ),
    String,
> {
    use crate::durable_host::routed_join::{MemberReturnFrame, MEMBER_RETURN_SCHEMA};
    let deadline = Instant::now()
        + Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000)));
    let mut line = connect(
        candidate,
        Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000))),
    )?;
    line.replace_io_deadline(deadline)
        .map_err(|e| format!("member return I/O deadline:{e:?}"))?;
    send(
        &mut line,
        &MemberReturnFrame::Advertise {
            schema: MEMBER_RETURN_SCHEMA.into(),
            credential: prior.clone(),
            advertisement: Box::new(actual.clone()),
        },
        ADMISSION,
    )?;
    let challenge = match receive::<MemberReturnFrame>(&mut line, MESSAGE)? {
        MemberReturnFrame::Challenge { schema, challenge } if schema == MEMBER_RETURN_SCHEMA => {
            *challenge
        }
        MemberReturnFrame::Refused { code, .. } => {
            return Err(format!("member return refused: {code}"))
        }
        _ => return Err("member return challenge response schema".into()),
    };
    let proof = super::super::membership::sign_return_challenge(
        state,
        prior,
        actual,
        &challenge,
        super::super::current_time_millis()?,
    )?;
    send(
        &mut line,
        &MemberReturnFrame::Proof {
            schema: MEMBER_RETURN_SCHEMA.into(),
            proof: crate::durable_host::owner::MemberReturnProofDocument::encode(proof),
        },
        MESSAGE,
    )?;
    let credential = match receive::<MemberReturnFrame>(&mut line, ADMISSION)? {
        MemberReturnFrame::Admitted {
            schema,
            credential,
            advertisement,
        } if schema == MEMBER_RETURN_SCHEMA
            && *advertisement == *actual
            && credential.issued_at_millis >= challenge.issued_at_millis
            && credential.issued_at_millis < challenge.expires_at_millis =>
        {
            credential
        }
        MemberReturnFrame::Refused { code, .. } => {
            return Err(format!("member return refused: {code}"))
        }
        _ => return Err("member return response differs from authenticated challenge".into()),
    };
    Ok((line, credential))
}

#[cfg(test)]
#[path = "member_return_tests.rs"]
mod member_return_tests;

fn bounded_signed_return(
    state: &std::path::Path,
    prior: &conduit_body::MembershipCredential,
    actual: &conduit_core::HostAdvertisement,
    candidate: &RendezvousCandidate,
) -> Result<
    (
        SecureWebSocketClientLine,
        conduit_body::MembershipCredential,
    ),
    String,
> {
    let mut result = Err("member return had no authenticated exchange".to_string());
    for _ in 0..candidate.maximum_attempts.min(2) {
        result = signed_return_exchange(state, prior, actual, candidate);
        if result.is_ok() {
            break;
        }
    }
    result
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ShowAcceptance {
    Accepted,
    Stale,
}
fn decode_show_acceptance(response: &Value) -> Result<ShowAcceptance, String> {
    if response["schema"] != "conduit.body/native-owner-show-ack-response@1" {
        return Err("presentation Show response schema".into());
    }
    match (response["accepted"].as_bool(), response["code"].as_str()) {
        (Some(true), Some("accepted")) => Ok(ShowAcceptance::Accepted),
        (Some(false), Some("stale-face")) => Ok(ShowAcceptance::Stale),
        _ => Err(format!("presentation Show refused:{}", response["code"])),
    }
}

#[cfg(test)]
#[path = "show_ack_tests.rs"]
mod show_ack_tests;
