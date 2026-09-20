//! Fixed-storage ConduitOS realization of one user-operated relay candidate.
//!
//! Relay attachment and routing remain outside the end-to-end protected
//! session. This adapter validates one shared candidate, binds its WSS
//! identity to the pinned certificate and exact TCP endpoint, attaches once,
//! and exposes only the protected session to protocol code.

use conduit_protected_line::{
    CarrierFailure, ProtectedFrameCarrier, RELAY_SERVICE_IMPLEMENTATION_ID,
    RelayCandidateDescriptor, RelayCandidateError, RelayEndpointRole, RelayEnvelope, Role,
    decode_relay_envelope,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    arch::VirtioNetReady,
    bounded_websocket::{BinaryWebSocketIo, MAXIMUM_BINARY_MESSAGE_BYTES, WebSocketError},
    cryptographic_entropy::{CryptographicEntropyBase, CryptographicEntropySource},
    protected_line_support::{
        ConduitOsProtectedLineIo, ConduitOsProtectedLineRefusal, establish_conduitos_protected_line,
    },
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls::{self, VirtioTlsError, VirtioWebSocketRunError},
};

const ATTACH_SCHEMA: &str = "conduit.relay/attach@1";
const OUTCOME_SCHEMA: &str = "conduit.relay/outcome@1";
const CLOSE_CONTROL: &[u8] = br#"{"schema":"conduit.relay/control@1","kind":"close"}"#;
const MAXIMUM_CONTROL_BYTES: usize = 1024;
const RELAY_ENVELOPE_OVERHEAD_BYTES: usize = 11;

#[derive(Debug, PartialEq, Eq)]
pub enum ConduitOsRelayRefusal<E> {
    Candidate(RelayCandidateError),
    EndpointBinding,
    CertificateBinding,
    Transport(VirtioTlsError),
    Attachment(RelayAttachmentRefusal),
    Protected(ConduitOsProtectedLineRefusal),
    Operation(E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayAttachmentRefusal {
    Encoding,
    TransportLost,
    Protocol,
    Pressure,
    Waiting,
}

/// Run one protected Line session through one exact relay candidate.
///
/// The caller supplies the already-admitted physical endpoint because the
/// candidate's locator is identity, not DNS or ambient discovery authority.
#[allow(clippy::too_many_arguments)]
pub fn with_conduitos_protected_relay<T, E, S, const REQUESTS: u32>(
    device: VirtioNetReady,
    tcp_seed: u64,
    tls_seed: [u8; 32],
    websocket_seed: [u8; 32],
    endpoint: VirtioTcpEndpoint,
    pinned_certificate_der: &[u8],
    maximum_polls: u32,
    candidate: &RelayCandidateDescriptor,
    now_millis: u64,
    entropy: &mut CryptographicEntropyBase<S, REQUESTS>,
    operation: impl FnOnce(&mut dyn ConduitOsProtectedLineIo) -> Result<T, E>,
) -> Result<(T, u32), ConduitOsRelayRefusal<E>>
where
    S: CryptographicEntropySource,
{
    candidate
        .validate(now_millis)
        .map_err(ConduitOsRelayRefusal::Candidate)?;
    validate_transport_binding(candidate, endpoint, pinned_certificate_der)?;
    let attempt_polls = maximum_polls.min(candidate.bounds.attempt_timeout_millis);
    if attempt_polls == 0 {
        return Err(ConduitOsRelayRefusal::EndpointBinding);
    }

    virtio_tls::with_websocket(
        device,
        tcp_seed,
        tls_seed,
        websocket_seed,
        endpoint,
        &candidate.identity.relay_server_identity,
        pinned_certificate_der,
        attempt_polls,
        |websocket| {
            let receive_budget = attempt_polls
                .min(candidate.bounds.protected_session.handshake_timeout_millis)
                .min(candidate.bounds.protected_session.idle_timeout_millis);
            let carrier = attach(websocket, candidate, receive_budget)
                .map_err(RelayOperationRefusal::Attachment)?;
            let role = match candidate.identity.role {
                RelayEndpointRole::First => Role::Initiator,
                RelayEndpointRole::Second => Role::Responder,
            };
            let mut protected = establish_conduitos_protected_line(
                carrier,
                role,
                &candidate.identity.session_binding,
                candidate.bounds.protected_session,
                candidate.copy_protected_session_psk_for_attempt(),
                entropy,
            )
            .map_err(RelayOperationRefusal::Protected)?;
            match operation(&mut protected) {
                Ok(value) => {
                    protected.close().map_err(|error| {
                        RelayOperationRefusal::Protected(ConduitOsProtectedLineRefusal::Session(
                            error,
                        ))
                    })?;
                    Ok(value)
                }
                Err(error) => {
                    let _ = protected.close();
                    Err(RelayOperationRefusal::Operation(error))
                }
            }
        },
    )
    .map_err(map_run_error)
}

fn validate_transport_binding<E>(
    candidate: &RelayCandidateDescriptor,
    endpoint: VirtioTcpEndpoint,
    pinned_certificate_der: &[u8],
) -> Result<(), ConduitOsRelayRefusal<E>> {
    if !locator_matches(
        &candidate.identity.relay_locator,
        &candidate.identity.relay_server_identity,
        endpoint.remote_port,
    ) {
        return Err(ConduitOsRelayRefusal::EndpointBinding);
    }
    let digest: [u8; 32] = Sha256::digest(pinned_certificate_der).into();
    if digest != candidate.identity.certificate_binding_sha256 {
        return Err(ConduitOsRelayRefusal::CertificateBinding);
    }
    let outer_bytes = RELAY_ENVELOPE_OVERHEAD_BYTES
        .checked_add(candidate.identity.route_id.len())
        .and_then(|length| {
            length.checked_add(candidate.bounds.maximum_protected_frame_bytes as usize)
        })
        .ok_or(ConduitOsRelayRefusal::EndpointBinding)?;
    if outer_bytes > MAXIMUM_BINARY_MESSAGE_BYTES {
        return Err(ConduitOsRelayRefusal::Candidate(
            RelayCandidateError::InvalidBounds,
        ));
    }
    Ok(())
}

fn locator_matches(locator: &str, server_identity: &str, port: u16) -> bool {
    let Some(rest) = locator.strip_prefix("wss://") else {
        return false;
    };
    let Some((authority, path)) = rest.split_once('/') else {
        return false;
    };
    if path != "conduit" {
        return false;
    }
    if port == 443 && authority == server_identity {
        return true;
    }
    authority
        .strip_prefix(server_identity)
        .and_then(|suffix| suffix.strip_prefix(':'))
        .and_then(|value| value.parse::<u16>().ok())
        == Some(port)
}

enum RelayOperationRefusal<E> {
    Attachment(RelayAttachmentRefusal),
    Protected(ConduitOsProtectedLineRefusal),
    Operation(E),
}

fn map_run_error<E>(
    error: VirtioWebSocketRunError<RelayOperationRefusal<E>>,
) -> ConduitOsRelayRefusal<E> {
    match error {
        VirtioWebSocketRunError::Transport(error) => ConduitOsRelayRefusal::Transport(error),
        VirtioWebSocketRunError::Operation(RelayOperationRefusal::Attachment(error)) => {
            ConduitOsRelayRefusal::Attachment(error)
        }
        VirtioWebSocketRunError::Operation(RelayOperationRefusal::Protected(error)) => {
            ConduitOsRelayRefusal::Protected(error)
        }
        VirtioWebSocketRunError::Operation(RelayOperationRefusal::Operation(error)) => {
            ConduitOsRelayRefusal::Operation(error)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum AttachmentRole {
    First,
    Second,
}

#[derive(Serialize)]
struct Attachment<'a> {
    schema: &'static str,
    route_id: &'a str,
    role: AttachmentRole,
    endpoint_binding: &'a str,
    capability: &'a [u8; 32],
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum OutcomeStatus {
    WaitingForPeer,
    Paired,
    Pressure,
    Closed,
    Lost,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome<'a> {
    schema: &'a str,
    implementation_id: &'a str,
    route_id: &'a str,
    status: OutcomeStatus,
    code: Option<&'a str>,
}

struct RelayWebSocketCarrier<'a, 'b> {
    websocket: &'a mut dyn BinaryWebSocketIo,
    route_id: &'b str,
    maximum_protected_frame_bytes: usize,
    maximum_receive_millis: u32,
    outer: [u8; MAXIMUM_BINARY_MESSAGE_BYTES],
}

fn attach<'a, 'b>(
    websocket: &'a mut dyn BinaryWebSocketIo,
    candidate: &'b RelayCandidateDescriptor,
    maximum_receive_millis: u32,
) -> Result<RelayWebSocketCarrier<'a, 'b>, RelayAttachmentRefusal> {
    let mut outer = [0_u8; MAXIMUM_BINARY_MESSAGE_BYTES];
    let mut capability = candidate.copy_relay_capability_for_attempt();
    let attachment = Attachment {
        schema: ATTACH_SCHEMA,
        route_id: &candidate.identity.route_id,
        role: match candidate.identity.role {
            RelayEndpointRole::First => AttachmentRole::First,
            RelayEndpointRole::Second => AttachmentRole::Second,
        },
        endpoint_binding: &candidate.identity.endpoint_binding,
        capability: &capability,
    };
    let encoded = serde_json_core::to_slice(&attachment, &mut outer[..MAXIMUM_CONTROL_BYTES])
        .map_err(|_| RelayAttachmentRefusal::Encoding);
    let result = match encoded {
        Ok(length) => websocket
            .send_binary(&outer[..length])
            .map_err(map_attachment_websocket),
        Err(error) => Err(error),
    };
    outer[..MAXIMUM_CONTROL_BYTES].fill(0);
    capability.fill(0);
    result?;

    let mut waiting = false;
    for _ in 0..2 {
        let length = websocket
            .receive_binary(&mut outer)
            .map_err(map_attachment_websocket)?;
        match decode_outcome(&outer[..length], &candidate.identity.route_id)? {
            OutcomeStatus::WaitingForPeer => waiting = true,
            OutcomeStatus::Paired => {
                outer[..length].fill(0);
                return Ok(RelayWebSocketCarrier {
                    websocket,
                    route_id: &candidate.identity.route_id,
                    maximum_protected_frame_bytes: candidate.bounds.maximum_protected_frame_bytes
                        as usize,
                    maximum_receive_millis,
                    outer,
                });
            }
            OutcomeStatus::Pressure => return Err(RelayAttachmentRefusal::Pressure),
            OutcomeStatus::Closed | OutcomeStatus::Lost => {
                return Err(RelayAttachmentRefusal::TransportLost);
            }
        }
        outer[..length].fill(0);
    }
    if waiting {
        Err(RelayAttachmentRefusal::Waiting)
    } else {
        Err(RelayAttachmentRefusal::Protocol)
    }
}

fn decode_outcome(encoded: &[u8], route_id: &str) -> Result<OutcomeStatus, RelayAttachmentRefusal> {
    let (outcome, used) = serde_json_core::from_slice::<Outcome<'_>>(encoded)
        .map_err(|_| RelayAttachmentRefusal::Protocol)?;
    if used != encoded.len()
        || outcome.schema != OUTCOME_SCHEMA
        || outcome.implementation_id != RELAY_SERVICE_IMPLEMENTATION_ID
        || outcome.route_id != route_id
        || outcome.code.is_some_and(|code| code.len() > 128)
    {
        return Err(RelayAttachmentRefusal::Protocol);
    }
    Ok(outcome.status)
}

impl ProtectedFrameCarrier for RelayWebSocketCarrier<'_, '_> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        if frame.len() > self.maximum_protected_frame_bytes {
            return Err(CarrierFailure::Pressure);
        }
        let length = RelayEnvelope {
            route_id: self.route_id,
            protected_frame: frame,
        }
        .encode(&mut self.outer)
        .map_err(|_| CarrierFailure::Pressure)?;
        self.websocket
            .send_binary(&self.outer[..length])
            .map_err(map_carrier_websocket)
    }

    fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout_millis: u32,
    ) -> Result<usize, CarrierFailure> {
        if timeout_millis == 0 || self.maximum_receive_millis > timeout_millis {
            return Err(CarrierFailure::TimedOut);
        }
        let length = self
            .websocket
            .receive_binary(&mut self.outer)
            .map_err(map_carrier_websocket)?;
        if !self.outer[..length].starts_with(b"CNDR") {
            return match decode_outcome(&self.outer[..length], self.route_id) {
                Ok(OutcomeStatus::Pressure) => Err(CarrierFailure::Pressure),
                _ => Err(CarrierFailure::Lost),
            };
        }
        let envelope =
            decode_relay_envelope(&self.outer[..length], self.maximum_protected_frame_bytes)
                .map_err(|_| CarrierFailure::Lost)?;
        if envelope.route_id != self.route_id || envelope.protected_frame.len() > output.len() {
            return Err(CarrierFailure::Pressure);
        }
        output[..envelope.protected_frame.len()].copy_from_slice(envelope.protected_frame);
        Ok(envelope.protected_frame.len())
    }

    fn close(&mut self) -> Result<(), CarrierFailure> {
        self.websocket
            .send_binary(CLOSE_CONTROL)
            .map_err(map_carrier_websocket)
    }
}

fn map_attachment_websocket(error: WebSocketError) -> RelayAttachmentRefusal {
    match error {
        WebSocketError::RequestTooLarge | WebSocketError::ResponseTooLarge => {
            RelayAttachmentRefusal::Pressure
        }
        _ => RelayAttachmentRefusal::TransportLost,
    }
}

fn map_carrier_websocket(error: WebSocketError) -> CarrierFailure {
    match error {
        WebSocketError::RequestTooLarge | WebSocketError::ResponseTooLarge => {
            CarrierFailure::Pressure
        }
        _ => CarrierFailure::Lost,
    }
}

#[cfg(test)]
mod tests;
