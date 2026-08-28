use serde::{Deserialize, Serialize};

use crate::{
    AuthorityGrantId, BaseImplementationId, BaseInstanceId, BootId, CredentialReferenceId, HostId,
    LineId, LinkBindingId, LinkEndpointId, SignId,
};

/// Reserved identity for realization wholly inside one Host process.
///
/// Local execution is the sole universal Base identity because it denotes the
/// absence of a cross-Host Line: same-Host Cords are lowered directly into the
/// one kernel and therefore must never be admitted as offered connectivity.
pub const LOCAL_BASE_IMPLEMENTATION_ID: &str = "conduit.base/local@1";

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
    pub base: BaseImplementationId,
    pub base_instance_id: BaseInstanceId,
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
    pub base: BaseImplementationId,
    pub base_instance_id: BaseInstanceId,
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
            base: binding.base.clone(),
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
