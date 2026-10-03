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

use crate::{
    arch::VirtioNetReady,
    bounded_websocket::{BinaryWebSocketIo, MAXIMUM_BINARY_MESSAGE_BYTES, WebSocketError},
    spore_provision::RouteCertificate,
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls::{self, VirtioTlsError, VirtioWebSocketRunError},
    wss_candidate_support::{certificate_matches, locator_matches},
};

pub struct OwnerRouteSeeds {
    pub tcp: u64,
    pub tls: [u8; 32],
    pub websocket: [u8; 32],
}

#[derive(Debug, PartialEq, Eq)]
pub enum NativeOwnerAdmissionRefusal {
    RouteIdentity,
    CandidateMissing,
    UnsupportedLine,
    EndpointBinding,
    CertificateBinding,
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
) -> Result<(PortableAdmissionReceipt, u32), NativeOwnerAdmissionRefusal> {
    let encoded = prepare_request(request, current)?;
    let (candidate, attempt_polls) =
        validate_candidate(route, certificate, request, endpoint, maximum_polls)?;
    virtio_tls::with_websocket(
        device,
        seeds.tcp,
        seeds.tls,
        seeds.websocket,
        endpoint,
        &candidate.authentication.server_identity,
        &certificate.certificate_der,
        attempt_polls,
        |line| exchange_prepared(line, &encoded, request),
    )
    .map_err(|error| match error {
        VirtioWebSocketRunError::Transport(error) => NativeOwnerAdmissionRefusal::Transport(error),
        VirtioWebSocketRunError::Operation(error) => error,
    })
}

fn validate_candidate<'a>(
    route: &'a SpawnRendezvousDescriptor,
    certificate: &RouteCertificate,
    request: &RoutedAdmissionRequest,
    endpoint: VirtioTcpEndpoint,
    maximum_polls: u32,
) -> Result<(&'a RendezvousCandidate, u32), NativeOwnerAdmissionRefusal> {
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
    if !locator_matches(
        &candidate.reachability,
        &candidate.authentication.server_identity,
        endpoint.remote_port,
    ) {
        return Err(NativeOwnerAdmissionRefusal::EndpointBinding);
    }
    if !certificate_matches(
        candidate.authentication.transport_binding_sha256,
        &certificate.certificate_der,
    ) {
        return Err(NativeOwnerAdmissionRefusal::CertificateBinding);
    }
    let attempt_polls = maximum_polls.min(candidate.attempt_timeout_millis);
    if candidate.maximum_attempts == 0 || attempt_polls == 0 {
        return Err(NativeOwnerAdmissionRefusal::EndpointBinding);
    }
    Ok((candidate, attempt_polls))
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
