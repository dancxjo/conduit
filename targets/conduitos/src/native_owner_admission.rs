//! One routed native Body admission exchange over an authenticated owner carrier.
//!
//! This is pre-Play preparation. A verified receipt is evidence of the owner's
//! admission decision; installing membership and presenting a shared Body are
//! separate product transitions.

use alloc::{string::String, vec};
use conduit_body::{
    AdmissionDocumentRefusal, PortableAdmissionReceipt, RENDEZVOUS_DESCRIPTOR_PROTOCOL,
    ROUTED_ADMISSION_REQUEST_SCHEMA, ROUTED_ADMISSION_RESPONSE_SCHEMA, RendezvousCandidate,
    RendezvousLineFamily, RoutedAdmissionRequest, RoutedAdmissionResponse,
    SpawnRendezvousDescriptor,
};
use conduit_core::HostAdvertisement;
use conduit_presentation::{
    MAX_OWNER_FACE_RESPONSE_BYTES, OWNER_FACE_REQUEST_SCHEMA, OwnerFaceSnapshotRequest,
    OwnerFaceSnapshotResponse,
};
use serde::Deserialize;

use crate::{
    arch::{CandidateDeadline, VirtioNetReady},
    bounded_websocket::{BinaryWebSocketIo, MAXIMUM_BINARY_MESSAGE_BYTES, WebSocketError},
    native_guest_face::{GuestFaceRefusal, NativeGuestFace},
    native_owner_document::{self, DocumentRefusal},
    spore_provision::RouteCertificate,
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls::{self, VirtioTlsError, VirtioWebSocketRunError},
    wss_candidate_support::{certificate_matches, literal_ipv4_locator, locator_matches},
};

pub struct OwnerRouteSeeds {
    pub tcp: u64,
    pub tls: [u8; 32],
    pub websocket: [u8; 32],
}

/// Admission remains installed even if the subsequent read-only Face request
/// refuses or the owner closes the Line. A Face refusal cannot erase a Part.
pub struct NativeOwnerAdmissionExchange {
    pub receipt: PortableAdmissionReceipt,
    pub face: Result<NativeGuestFace, NativeOwnerFaceExchangeRefusal>,
    pub return_grant: Option<NativeOwnerReturnGrant>,
}

/// Distinct from the spent invitation. The owner issues this finite bearer
/// only on the already pinned and authenticated admission Line.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeOwnerReturnGrant {
    pub schema: String,
    pub token: [u8; 32],
    pub credential_id: String,
    pub body_id: String,
    pub part_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub remaining_millis: u32,
    pub maximum_actions: u8,
}

impl NativeOwnerReturnGrant {
    fn matches_receipt(&self, receipt: &PortableAdmissionReceipt) -> bool {
        let credential = &receipt.credential;
        self.schema == "conduit.body/native-owner-return-grant@1"
            && self.token != [0; 32]
            && (1_000..=60_000).contains(&self.remaining_millis)
            && (1..=4).contains(&self.maximum_actions)
            && self.credential_id == credential.credential_id.as_str()
            && self.body_id == credential.body_id.as_str()
            && self.part_id == credential.part_id.as_str()
            && self.host_id == credential.host_id.as_str()
            && self.boot_id == credential.boot_id.as_str()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum NativeOwnerFaceExchangeRefusal {
    Send(WebSocketError),
    Receive(WebSocketError),
    Encoding,
    FramePressure,
    GrantBasis,
    Face(GuestFaceRefusal),
}

impl NativeOwnerFaceExchangeRefusal {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Send(_) => "native-owner-face-send-refused",
            Self::Receive(_) => "native-owner-face-receive-refused",
            Self::Encoding => "native-owner-face-response-encoding-invalid",
            Self::FramePressure => "native-owner-face-frame-pressure",
            Self::GrantBasis => "native-owner-return-grant-invalid",
            Self::Face(refusal) => refusal.as_str(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum NativeOwnerAdmissionRefusal {
    RouteIdentity,
    CandidateMissing,
    UnsupportedLine,
    EndpointBinding,
    CertificateBinding,
    ClockUnavailable,
    RequestBasis,
    RequestPressure,
    Send(WebSocketError),
    Receive(WebSocketError),
    ResponseEncoding,
    ResponseSchema,
    OwnerRefused(String),
    Receipt(AdmissionDocumentRefusal),
    Transport(VirtioTlsError),
}

impl NativeOwnerAdmissionRefusal {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RouteIdentity => "owner-route-identity-invalid",
            Self::CandidateMissing => "owner-route-candidate-missing",
            Self::UnsupportedLine => "owner-route-line-unsupported",
            Self::EndpointBinding => "owner-route-endpoint-invalid",
            Self::CertificateBinding => "owner-route-certificate-invalid",
            Self::ClockUnavailable => "owner-route-deadline-clock-unavailable",
            Self::RequestBasis => "owner-admission-request-invalid",
            Self::RequestPressure => "owner-admission-request-pressure",
            Self::Send(_) => "owner-admission-send-refused",
            Self::Receive(_) => "owner-admission-receive-refused",
            Self::ResponseEncoding => "owner-admission-response-invalid",
            Self::ResponseSchema => "owner-admission-response-schema-invalid",
            Self::OwnerRefused(_) => "owner-admission-refused",
            Self::Receipt(_) => "owner-admission-receipt-invalid",
            Self::Transport(error) => error.as_str(),
        }
    }
}

/// Consume one candidate in one bounded attempt. Neither candidate discovery
/// nor the invitation signature authenticates the owner: the pinned TLS leaf
/// and server name must match the exact provisioned route before exchange.
#[allow(clippy::too_many_arguments)]
pub fn exchange_over_candidate(
    device: VirtioNetReady,
    seeds: OwnerRouteSeeds,
    endpoint: VirtioTcpEndpoint,
    maximum_polls: u32,
    route: &SpawnRendezvousDescriptor,
    certificate: &RouteCertificate,
    request: &RoutedAdmissionRequest,
    current: &HostAdvertisement,
) -> Result<(NativeOwnerAdmissionExchange, u32, VirtioNetReady), NativeOwnerAdmissionRefusal> {
    let encoded = prepare_request(request, current)?;
    let candidate = validate_candidate(route, certificate, request, endpoint, maximum_polls)?;
    let deadline = CandidateDeadline::admit(candidate.attempt_timeout_millis)
        .ok_or(NativeOwnerAdmissionRefusal::ClockUnavailable)?;
    virtio_tls::with_websocket_deadline_retain_device(
        device,
        seeds.tcp,
        seeds.tls,
        seeds.websocket,
        endpoint,
        &candidate.authentication.server_identity,
        &certificate.certificate_der,
        maximum_polls,
        Some(deadline),
        |line| {
            let receipt = exchange_prepared(line, &encoded, request)?;
            let face_and_grant = exchange_face(line, &receipt);
            let (face, return_grant) = match face_and_grant {
                Ok((face, grant)) => (Ok(face), grant),
                Err(error) => (Err(error), None),
            };
            Ok(NativeOwnerAdmissionExchange {
                receipt,
                face,
                return_grant,
            })
        },
    )
    .map_err(|error| match error {
        VirtioWebSocketRunError::Transport(error) => NativeOwnerAdmissionRefusal::Transport(error),
        VirtioWebSocketRunError::Operation(error) => error,
    })
}

fn exchange_face(
    line: &mut dyn BinaryWebSocketIo,
    receipt: &PortableAdmissionReceipt,
) -> Result<(NativeGuestFace, Option<NativeOwnerReturnGrant>), NativeOwnerFaceExchangeRefusal> {
    let credential = &receipt.credential;
    let request = OwnerFaceSnapshotRequest {
        schema: OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: credential.credential_id.as_str().into(),
        body_id: credential.body_id.clone(),
        part_id: credential.part_id.clone(),
        host_id: credential.host_id.clone(),
        boot_id: credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    if !request.has_exact_basis() {
        return Err(NativeOwnerFaceExchangeRefusal::Encoding);
    }
    let encoded = serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.body/native-owner-face-request@1", "request":request,
    }))
    .map_err(|_| NativeOwnerFaceExchangeRefusal::Encoding)?;
    if encoded.len() > MAX_OWNER_FACE_RESPONSE_BYTES {
        return Err(NativeOwnerFaceExchangeRefusal::FramePressure);
    }
    line.send_binary(&encoded)
        .map_err(NativeOwnerFaceExchangeRefusal::Send)?;
    let response: OwnerFaceSnapshotResponse = native_owner_document::receive(line, 64 * 1024)
        .map_err(|error| match error {
            DocumentRefusal::Receive(error) => NativeOwnerFaceExchangeRefusal::Receive(error),
            DocumentRefusal::Bound => NativeOwnerFaceExchangeRefusal::FramePressure,
            DocumentRefusal::Decode => NativeOwnerFaceExchangeRefusal::Encoding,
        })?;
    let admitted = matches!(
        &response,
        OwnerFaceSnapshotResponse::Snapshot {
            interactions_admitted: true,
            ..
        }
    );
    let face = NativeGuestFace::from_owner_response(receipt, response)
        .map_err(NativeOwnerFaceExchangeRefusal::Face)?;
    let grant = if admitted {
        let mut response_bytes = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
        let length = line
            .receive_binary(&mut response_bytes)
            .map_err(NativeOwnerFaceExchangeRefusal::Receive)?;
        let grant: NativeOwnerReturnGrant = serde_json::from_slice(&response_bytes[..length])
            .map_err(|_| NativeOwnerFaceExchangeRefusal::Encoding)?;
        if !grant.matches_receipt(receipt) {
            return Err(NativeOwnerFaceExchangeRefusal::GrantBasis);
        }
        Some(grant)
    } else {
        None
    };
    Ok((face, grant))
}

fn validate_candidate<'a>(
    route: &'a SpawnRendezvousDescriptor,
    certificate: &RouteCertificate,
    request: &RoutedAdmissionRequest,
    endpoint: VirtioTcpEndpoint,
    maximum_polls: u32,
) -> Result<&'a RendezvousCandidate, NativeOwnerAdmissionRefusal> {
    if route.protocol != RENDEZVOUS_DESCRIPTOR_PROTOCOL
        || route.body_id != request.request.body_id.as_str()
        || route.invitation_id != request.invitation_id
    {
        return Err(NativeOwnerAdmissionRefusal::RouteIdentity);
    }
    let candidate = route
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == certificate.candidate_id)
        .ok_or(NativeOwnerAdmissionRefusal::CandidateMissing)?;
    if candidate.line_family != RendezvousLineFamily::AuthenticatedTlsStream {
        return Err(NativeOwnerAdmissionRefusal::UnsupportedLine);
    }
    let numeric_endpoint = literal_ipv4_locator(&candidate.reachability);
    if !locator_matches(
        &candidate.reachability,
        &candidate.authentication.server_identity,
        endpoint.remote_port,
    ) && numeric_endpoint != Some((endpoint.remote_address, endpoint.remote_port))
    {
        return Err(NativeOwnerAdmissionRefusal::EndpointBinding);
    }
    if !certificate_matches(
        candidate.authentication.transport_binding_sha256,
        &certificate.certificate_der,
    ) {
        return Err(NativeOwnerAdmissionRefusal::CertificateBinding);
    }
    if candidate.maximum_attempts == 0
        || candidate.attempt_timeout_millis == 0
        || maximum_polls == 0
    {
        return Err(NativeOwnerAdmissionRefusal::EndpointBinding);
    }
    Ok(candidate)
}

/// Exercise exact owner wire forms over a scripted authenticated carrier.
#[cfg(test)]
fn exchange_document(
    line: &mut dyn BinaryWebSocketIo,
    request: &RoutedAdmissionRequest,
    current: &HostAdvertisement,
) -> Result<PortableAdmissionReceipt, NativeOwnerAdmissionRefusal> {
    let encoded = prepare_request(request, current)?;
    exchange_prepared(line, &encoded, request)
}

fn prepare_request(
    request: &RoutedAdmissionRequest,
    current: &HostAdvertisement,
) -> Result<alloc::vec::Vec<u8>, NativeOwnerAdmissionRefusal> {
    if request.schema != ROUTED_ADMISSION_REQUEST_SCHEMA
        || request.invitation_id != request.request.invitation_id.as_str()
        || request.request.host_advertisement != *current
        || request.request.validate().is_err()
    {
        return Err(NativeOwnerAdmissionRefusal::RequestBasis);
    }
    let encoded =
        serde_json::to_vec(request).map_err(|_| NativeOwnerAdmissionRefusal::RequestBasis)?;
    if encoded.is_empty() || encoded.len() > MAXIMUM_BINARY_MESSAGE_BYTES {
        return Err(NativeOwnerAdmissionRefusal::RequestPressure);
    }
    Ok(encoded)
}

fn exchange_prepared(
    line: &mut dyn BinaryWebSocketIo,
    encoded: &[u8],
    request: &RoutedAdmissionRequest,
) -> Result<PortableAdmissionReceipt, NativeOwnerAdmissionRefusal> {
    line.send_binary(encoded)
        .map_err(NativeOwnerAdmissionRefusal::Send)?;
    let mut response = vec![0; MAXIMUM_BINARY_MESSAGE_BYTES];
    let received = line
        .receive_binary(&mut response)
        .map_err(NativeOwnerAdmissionRefusal::Receive)?;
    let outcome: RoutedAdmissionResponse = serde_json::from_slice(&response[..received])
        .map_err(|_| NativeOwnerAdmissionRefusal::ResponseEncoding)?;
    match outcome {
        RoutedAdmissionResponse::Admitted { schema, receipt }
            if schema == ROUTED_ADMISSION_RESPONSE_SCHEMA =>
        {
            receipt
                .validate_against(&request.request)
                .map_err(NativeOwnerAdmissionRefusal::Receipt)?;
            Ok(*receipt)
        }
        RoutedAdmissionResponse::Refused { schema, code }
            if schema == ROUTED_ADMISSION_RESPONSE_SCHEMA && !code.is_empty() =>
        {
            Err(NativeOwnerAdmissionRefusal::OwnerRefused(code))
        }
        _ => Err(NativeOwnerAdmissionRefusal::ResponseSchema),
    }
}

#[cfg(test)]
#[path = "native_owner_admission_tests.rs"]
mod tests;
