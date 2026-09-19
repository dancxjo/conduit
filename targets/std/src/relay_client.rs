//! Hosted outbound client for one finite user-operated relay candidate.

use crate::secure_websocket::{SecureWebSocketClientLine, SecureWebSocketError};
use conduit_protected_line::{
    decode_relay_envelope, CarrierFailure, ProtectedFrameCarrier, RelayEndpointRole, RelayEnvelope,
    RELAY_SERVICE_IMPLEMENTATION_ID,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

const ATTACH_SCHEMA: &str = "conduit.relay/attach@1";
const CONTROL_SCHEMA: &str = "conduit.relay/control@1";
const OUTCOME_SCHEMA: &str = "conduit.relay/outcome@1";
const MAXIMUM_CONTROL_BYTES: usize = 4 * 1024;

pub struct RelayClientDescriptor {
    pub address: SocketAddr,
    pub public_url: String,
    pub server_identity: String,
    pub certificate_binding_sha256: [u8; 32],
    pub route_id: String,
    pub role: RelayEndpointRole,
    pub endpoint_binding: String,
    pub capability: [u8; 32],
    pub maximum_protected_frame_bytes: u32,
    pub timeout_millis: u32,
}

impl core::fmt::Debug for RelayClientDescriptor {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RelayClientDescriptor")
            .field("address", &self.address)
            .field("public_url", &self.public_url)
            .field("server_identity", &self.server_identity)
            .field(
                "certificate_binding_sha256",
                &self.certificate_binding_sha256,
            )
            .field("route_id", &self.route_id)
            .field("role", &self.role)
            .field("endpoint_binding", &self.endpoint_binding)
            .field("capability", &"[REDACTED]")
            .field(
                "maximum_protected_frame_bytes",
                &self.maximum_protected_frame_bytes,
            )
            .field("timeout_millis", &self.timeout_millis)
            .finish()
    }
}

impl Drop for RelayClientDescriptor {
    fn drop(&mut self) {
        self.capability.fill(0);
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

#[derive(Deserialize)]
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
struct Outcome {
    schema: String,
    implementation_id: String,
    route_id: String,
    status: OutcomeStatus,
    code: Option<String>,
}

#[derive(Serialize)]
struct CloseControl {
    schema: &'static str,
    kind: &'static str,
}

pub struct HostedRelayCarrier {
    line: SecureWebSocketClientLine,
    route_id: String,
    outer: Vec<u8>,
    maximum_protected_frame_bytes: usize,
}

impl HostedRelayCarrier {
    pub fn connect(mut descriptor: RelayClientDescriptor) -> Result<Self, RelayClientError> {
        descriptor.validate()?;
        let maximum_protected_frame_bytes = descriptor.maximum_protected_frame_bytes as usize;
        let maximum_outer_bytes = maximum_protected_frame_bytes
            .checked_add(256)
            .ok_or(RelayClientError::InvalidDescriptor)?
            .max(MAXIMUM_CONTROL_BYTES);
        let timeout = Duration::from_millis(u64::from(descriptor.timeout_millis));
        let mut line = SecureWebSocketClientLine::connect_pinned(
            descriptor.address,
            &descriptor.public_url,
            &descriptor.server_identity,
            descriptor.certificate_binding_sha256,
            timeout,
            u32::try_from(maximum_outer_bytes).map_err(|_| RelayClientError::InvalidDescriptor)?,
        )
        .map_err(map_connect_error)?;
        let attachment = Attachment {
            schema: ATTACH_SCHEMA,
            route_id: &descriptor.route_id,
            role: match descriptor.role {
                RelayEndpointRole::First => AttachmentRole::First,
                RelayEndpointRole::Second => AttachmentRole::Second,
            },
            endpoint_binding: &descriptor.endpoint_binding,
            capability: &descriptor.capability,
        };
        let mut control =
            serde_json::to_vec(&attachment).map_err(|_| RelayClientError::InvalidDescriptor)?;
        let send_result = line.send_binary(&control).map_err(map_transport_error);
        control.fill(0);
        descriptor.capability.fill(0);
        send_result?;

        let mut outer = vec![0; maximum_outer_bytes];
        for _ in 0..2 {
            let length = line
                .receive_binary(&mut outer)
                .map_err(map_transport_error)?;
            match decode_outcome(&outer[..length], &descriptor.route_id)? {
                OutcomeStatus::WaitingForPeer => continue,
                OutcomeStatus::Paired => {
                    return Ok(Self {
                        line,
                        route_id: descriptor.route_id.clone(),
                        outer,
                        maximum_protected_frame_bytes,
                    });
                }
                OutcomeStatus::Pressure => return Err(RelayClientError::Pressure),
                OutcomeStatus::Closed | OutcomeStatus::Lost => {
                    return Err(RelayClientError::TransportLost)
                }
            }
        }
        Err(RelayClientError::Protocol)
    }
}

impl ProtectedFrameCarrier for HostedRelayCarrier {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        let length = RelayEnvelope {
            route_id: &self.route_id,
            protected_frame: frame,
        }
        .encode(&mut self.outer)
        .map_err(|_| CarrierFailure::Pressure)?;
        self.line
            .send_binary(&self.outer[..length])
            .map_err(map_carrier_error)
    }

    fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout_millis: u32,
    ) -> Result<usize, CarrierFailure> {
        self.line
            .set_read_timeout(Some(Duration::from_millis(u64::from(timeout_millis))))
            .map_err(map_carrier_error)?;
        let length = self
            .line
            .receive_binary(&mut self.outer)
            .map_err(map_carrier_error)?;
        if !self.outer[..length].starts_with(b"CNDR") {
            return match decode_outcome(&self.outer[..length], &self.route_id) {
                Ok(OutcomeStatus::Pressure) => Err(CarrierFailure::Pressure),
                Ok(OutcomeStatus::Closed | OutcomeStatus::Lost) => Err(CarrierFailure::Lost),
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
        let control = serde_json::to_vec(&CloseControl {
            schema: CONTROL_SCHEMA,
            kind: "close",
        })
        .map_err(|_| CarrierFailure::Lost)?;
        self.line.send_binary(&control).map_err(map_carrier_error)?;
        self.line.close().map_err(map_carrier_error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayClientError {
    InvalidDescriptor,
    RelayUnreachable,
    TransportAuthenticationFailed,
    TimedOut,
    Pressure,
    TransportLost,
    Protocol,
}

impl RelayClientDescriptor {
    fn validate(&self) -> Result<(), RelayClientError> {
        if self.address.ip().is_unspecified()
            || !self.public_url.starts_with("wss://")
            || self.public_url.len() > 256
            || self.server_identity.is_empty()
            || self.server_identity.len() > 256
            || self.certificate_binding_sha256 == [0; 32]
            || self.route_id.is_empty()
            || self.route_id.len() > 128
            || self.endpoint_binding.is_empty()
            || self.endpoint_binding.len() > 128
            || self.capability == [0; 32]
            || self.maximum_protected_frame_bytes == 0
            || self.maximum_protected_frame_bytes > 65_553
            || self.timeout_millis == 0
            || self.timeout_millis > 300_000
        {
            return Err(RelayClientError::InvalidDescriptor);
        }
        Ok(())
    }
}

fn decode_outcome(bytes: &[u8], route_id: &str) -> Result<OutcomeStatus, RelayClientError> {
    let outcome: Outcome = serde_json::from_slice(bytes).map_err(|_| RelayClientError::Protocol)?;
    if outcome.schema != OUTCOME_SCHEMA
        || outcome.implementation_id != RELAY_SERVICE_IMPLEMENTATION_ID
        || outcome.route_id != route_id
    {
        return Err(RelayClientError::Protocol);
    }
    let _bounded_code = outcome.code.filter(|code| code.len() <= 128);
    Ok(outcome.status)
}

fn map_connect_error(error: SecureWebSocketError) -> RelayClientError {
    match error {
        SecureWebSocketError::Tls | SecureWebSocketError::Handshake => {
            RelayClientError::TransportAuthenticationFailed
        }
        SecureWebSocketError::Transport(std::io::ErrorKind::TimedOut)
        | SecureWebSocketError::AcceptDeadline => RelayClientError::TimedOut,
        SecureWebSocketError::Transport(_) => RelayClientError::RelayUnreachable,
        _ => RelayClientError::Protocol,
    }
}

fn map_transport_error(error: SecureWebSocketError) -> RelayClientError {
    match error {
        SecureWebSocketError::Transport(
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
        ) => RelayClientError::TimedOut,
        SecureWebSocketError::OversizedMessage | SecureWebSocketError::OutputTooSmall => {
            RelayClientError::Pressure
        }
        SecureWebSocketError::Disconnected | SecureWebSocketError::Transport(_) => {
            RelayClientError::TransportLost
        }
        _ => RelayClientError::Protocol,
    }
}

fn map_carrier_error(error: SecureWebSocketError) -> CarrierFailure {
    match map_transport_error(error) {
        RelayClientError::TimedOut => CarrierFailure::TimedOut,
        RelayClientError::Pressure => CarrierFailure::Pressure,
        _ => CarrierFailure::Lost,
    }
}

#[cfg(test)]
mod tests;
