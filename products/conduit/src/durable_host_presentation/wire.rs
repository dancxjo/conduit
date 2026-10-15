//! Bounded pinned TLS exchanges for the existing finite owner return protocol.
use conduit_body::{
    PortableAdmissionReceipt, PortableInvitation, RendezvousCandidate, RoutedAdmissionRequest,
    RoutedAdmissionResponse, ROUTED_ADMISSION_REQUEST_SCHEMA, ROUTED_ADMISSION_RESPONSE_SCHEMA,
};
use conduit_face::{
    Face, MaskShow, OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse, RemoteOwnerMaskRouteSeal,
    OWNER_FACE_REQUEST_SCHEMA, OWNER_FACE_RESPONSE_SCHEMA,
};
use conduit_std_host::secure_websocket::SecureWebSocketClientLine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

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
    ) -> Result<(Self, Snapshot), String> {
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
        let request = host.prepare_join(invitation)?;
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
                    if schema == ROUTED_ADMISSION_RESPONSE_SCHEMA =>
                {
                    *receipt
                }
                RoutedAdmissionResponse::Refused { code, .. } => {
                    return Err(format!("presentation admission refused:{code}"))
                }
                _ => return Err("presentation admission response schema".into()),
            };
        receipt
            .validate_against(&request)
            .map_err(|error| error.to_string())?;
        host.retain_join(receipt.clone())?;
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
            &json!({"schema":"conduit.body/native-owner-face-request@1", "request":request, "mask":"terminal"}),
            MESSAGE,
        )?;
        let snapshot = snapshot(document(&mut line)?)?;
        let grant: Grant = receive(&mut line, MESSAGE)?;
        validate_grant(&grant, &request)?;
        let deadline = Instant::now() + Duration::from_millis(u64::from(grant.remaining_millis));
        Ok((
            Self {
                candidate,
                request,
                grant,
                deadline,
                sequence: 1,
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
        let mut line = self.connection()?;
        send_return(
            &mut line,
            &json!({"schema":"conduit.body/native-owner-show-ack@1", "token":self.grant.token,
            "sequence":self.sequence, "request":self.request, "show":show}),
            MESSAGE,
        )?;
        let response: Value = receive(&mut line, MESSAGE)?;
        if response["schema"] != "conduit.body/native-owner-show-ack-response@1"
            || response["accepted"] != true
        {
            return Err(format!("presentation Show refused:{}", response["code"]));
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
            request["mask"] = json!("terminal");
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
        let next = if response["face"].is_null() || response["face"]["route"].is_null() {
            None
        } else {
            Some(snapshot(response["face"].clone())?)
        };
        Ok((
            json!({"accepted":response["accepted"],"code":response["code"]}),
            next,
        ))
    }
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
