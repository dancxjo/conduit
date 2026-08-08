use serde::{Deserialize, Serialize};

use crate::{
    AuthorityGrantId, BootId, ConnectionProviderInstanceId, CredentialReferenceId, EvidenceId,
    HostId, LinkBindingId, LinkEndpointId,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionProvider {
    Local,
    InMemory,
    /// Deterministic bounded frame transit used only by conformance fixtures.
    FixtureFrame,
    /// Deterministic bounded datagram transit used only by conformance fixtures.
    FixtureDatagram,
    /// Actual RFC 6455 binary-message carrier.
    WebSocket,
    /// Bounded length-framed USB CDC ACM byte-stream carrier.
    UsbCdc,
}

impl ConnectionProvider {
    pub const fn canonical_code(self) -> u8 {
        match self {
            Self::Local => 0,
            Self::InMemory => 1,
            Self::FixtureFrame => 2,
            Self::FixtureDatagram => 3,
            Self::WebSocket => 4,
            Self::UsbCdc => 5,
        }
    }

    pub const fn from_canonical_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Local),
            1 => Some(Self::InMemory),
            2 => Some(Self::FixtureFrame),
            3 => Some(Self::FixtureDatagram),
            4 => Some(Self::WebSocket),
            5 => Some(Self::UsbCdc),
            _ => None,
        }
    }

    /// Contract compatibility does not claim an installed or runnable carrier.
    pub const fn supports_remote_session(self) -> bool {
        matches!(self, Self::FixtureFrame | Self::WebSocket | Self::UsbCdc)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkAvailability {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkCredentialReference {
    None,
    Opaque(CredentialReferenceId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkAuthorityReference {
    ProcessOwned,
    Grant(AuthorityGrantId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkEndpoint {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub endpoint_id: LinkEndpointId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkLimits {
    pub maximum_in_flight_items: u16,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: u32,
    pub maximum_frame_bytes: u32,
}

/// One observed, directional, boot-scoped initialized provider instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkBinding {
    pub binding_id: LinkBindingId,
    pub source: LinkEndpoint,
    pub sink: LinkEndpoint,
    pub provider: ConnectionProvider,
    pub provider_instance_id: ConnectionProviderInstanceId,
    pub availability: LinkAvailability,
    pub credential: LinkCredentialReference,
    pub authority: LinkAuthorityReference,
    pub limits: LinkLimits,
}

/// Immutable identity and contract facts for one exact permissible route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundLink {
    pub binding_id: LinkBindingId,
    pub source: LinkEndpoint,
    pub sink: LinkEndpoint,
    pub provider: ConnectionProvider,
    pub provider_instance_id: ConnectionProviderInstanceId,
    pub credential: LinkCredentialReference,
    pub authority: LinkAuthorityReference,
    pub limits: LinkLimits,
}

impl From<&LinkBinding> for BoundLink {
    fn from(binding: &LinkBinding) -> Self {
        Self {
            binding_id: binding.binding_id.clone(),
            source: binding.source.clone(),
            sink: binding.sink.clone(),
            provider: binding.provider,
            provider_instance_id: binding.provider_instance_id.clone(),
            credential: binding.credential.clone(),
            authority: binding.authority.clone(),
            limits: binding.limits,
        }
    }
}

/// Mutable evidence about a link, deliberately outside route identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkObservation {
    pub binding_id: LinkBindingId,
    pub availability: LinkAvailability,
    pub evidence_id: EvidenceId,
}

impl LinkBinding {
    pub fn bound_link(&self) -> BoundLink {
        BoundLink::from(self)
    }

    pub fn observation(&self, evidence_id: EvidenceId) -> LinkObservation {
        LinkObservation {
            binding_id: self.binding_id.clone(),
            availability: self.availability,
            evidence_id,
        }
    }
}
