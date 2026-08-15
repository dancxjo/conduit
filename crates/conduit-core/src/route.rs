use serde::{Deserialize, Serialize};

use crate::{
    AuthorityGrantId, BootId, ConnectionBaseInstanceId, CredentialReferenceId, HostId, LineId,
    LinkBindingId, LinkEndpointId, SignId,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionBase {
    Local,
    InMemory,
    /// Deterministic bounded frame transit used only by conformance fixtures.
    FixtureFrame,
    /// Deterministic bounded datagram transit used only by conformance fixtures.
    FixtureDatagram,
    /// Actual RFC 6455 binary-message Base.
    WebSocket,
    /// Bounded length-framed USB CDC ACM byte-stream Base.
    UsbCdc,
    /// BLE GATT service with bounded write-command and notification characteristics.
    BluetoothLeGatt,
}

impl ConnectionBase {
    pub const fn canonical_code(self) -> u8 {
        match self {
            Self::Local => 0,
            Self::InMemory => 1,
            Self::FixtureFrame => 2,
            Self::FixtureDatagram => 3,
            Self::WebSocket => 4,
            Self::UsbCdc => 5,
            Self::BluetoothLeGatt => 6,
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
            6 => Some(Self::BluetoothLeGatt),
            _ => None,
        }
    }

    /// Contract compatibility does not claim an offered or runnable Line.
    pub const fn supports_remote_session(self) -> bool {
        matches!(
            self,
            Self::FixtureFrame | Self::WebSocket | Self::UsbCdc | Self::BluetoothLeGatt
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineAvailability {
    Ready,
    Unavailable,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineScope {
    Process,
    Machine,
    PointToPoint,
    LocalNetwork,
    RoutedNetwork,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineTrafficShape {
    ByteStream,
    Message,
    Datagram,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineDuplex {
    Simplex,
    HalfDuplex,
    FullDuplex,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineOrdering {
    Ordered,
    Unordered,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineReliability {
    Reliable,
    BestEffort,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineContinuation {
    None,
    BoundedSessionReconciliation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineSecurity {
    ProcessBoundary,
    PhysicalPossession,
    PlaintextNetwork,
    AuthenticatedEncrypted,
}

/// Explicit finite behavior offered by a Line. No guarantee is inferred from
/// the Base name.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineContract {
    pub scope: LineScope,
    pub traffic_shape: LineTrafficShape,
    pub duplex: LineDuplex,
    pub ordering: LineOrdering,
    pub reliability: LineReliability,
    pub continuation: LineContinuation,
    pub security: LineSecurity,
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

/// One directional, boot-scoped binding to an initialized Base instance. This
/// lower-level identity is not Line identity and contains no availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkBinding {
    pub binding_id: LinkBindingId,
    pub source: LinkEndpoint,
    pub sink: LinkEndpoint,
    pub base: ConnectionBase,
    pub base_instance_id: ConnectionBaseInstanceId,
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
    pub base: ConnectionBase,
    pub base_instance_id: ConnectionBaseInstanceId,
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
            base: binding.base,
            base_instance_id: binding.base_instance_id.clone(),
            credential: binding.credential.clone(),
            authority: binding.authority.clone(),
            limits: binding.limits,
        }
    }
}

/// One finite Line offered by its source Host for Conduit traffic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineOffer {
    pub line_id: LineId,
    pub binding: LinkBinding,
    pub contract: LineContract,
    pub availability: LineAvailabilitySign,
}

/// Exact immutable Line facts admitted into a Plan. Availability is excluded:
/// it remains a current Sign and cannot mutate this identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmittedLine {
    pub line_id: LineId,
    pub binding: BoundLink,
    pub contract: LineContract,
}

impl From<&LineOffer> for AdmittedLine {
    fn from(offer: &LineOffer) -> Self {
        Self {
            line_id: offer.line_id.clone(),
            binding: offer.binding.bound_link(),
            contract: offer.contract,
        }
    }
}

/// Mutable availability Sign, deliberately outside admitted Plan identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineAvailabilitySign {
    pub line_id: LineId,
    pub binding_id: LinkBindingId,
    pub availability: LineAvailability,
    pub sign_id: SignId,
}

impl LinkBinding {
    pub fn bound_link(&self) -> BoundLink {
        BoundLink::from(self)
    }
}

impl LineOffer {
    pub fn admitted_line(&self) -> AdmittedLine {
        AdmittedLine::from(self)
    }

    pub fn validate_sign_identity(&self) -> bool {
        self.availability.line_id == self.line_id
            && self.availability.binding_id == self.binding.binding_id
            && !self.availability.sign_id.as_str().is_empty()
    }

    pub fn availability_sign(
        &self,
        availability: LineAvailability,
        sign_id: SignId,
    ) -> LineAvailabilitySign {
        LineAvailabilitySign {
            line_id: self.line_id.clone(),
            binding_id: self.binding.binding_id.clone(),
            availability,
            sign_id,
        }
    }
}
