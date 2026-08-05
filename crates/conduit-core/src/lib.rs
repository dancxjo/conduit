#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_CONNECTION_ITEM_CAPACITY: u16 = 4;
pub const DEFAULT_CONNECTION_BYTE_CAPACITY: u32 = 64;
pub const WAIT_HOST_OPERATION_CONTRACT: &str = "conduit.host/wait@1";
pub const PRESENT_HOST_OPERATION_CONTRACT: &str = "conduit.host/present@1";
pub const MAX_PRESENTATION_COMPLETION_BYTES: u32 = 256;
pub const TIMER_RESOURCE_CLASS: &str = "conduit.resource/timer-slot@1";
pub const PRESENTATION_RESOURCE_CLASS: &str = "conduit.resource/presentation-slot@1";
pub const PRESENT_AUTHORITY_CONTRACT: &str = "conduit.authority/present@1";

macro_rules! identity_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

identity_type!(HostId);
identity_type!(BootId);
identity_type!(CapabilityId);
identity_type!(KindId);
// Immutable identity of one exact semantic-kind contract revision.
identity_type!(KindContractRevision);
// Immutable identity of one exact implementation execution profile.
identity_type!(ExecutionProfileId);
identity_type!(ImplementationId);
identity_type!(ArtifactId);
identity_type!(SourceDocumentId);
identity_type!(CheckedFormId);
identity_type!(ExpandedFormId);
identity_type!(PlanId);
identity_type!(ActivePlayId);
identity_type!(EvidenceId);
identity_type!(PresentationId);
identity_type!(FragmentId);
identity_type!(PlacementId);
identity_type!(ConnectionId);
// Identity of one observed, directional, boot-scoped remote link.
identity_type!(LinkBindingId);
// Provider-owned identity of one exact initialized link endpoint.
identity_type!(LinkEndpointId);
// Identity of one initialized provider instance behind a link observation.
identity_type!(ConnectionProviderInstanceId);
// Opaque reference only; credential material never enters a plan.
identity_type!(CredentialReferenceId);
identity_type!(PortId);
identity_type!(OperationId);
identity_type!(HostProfileId);
// Immutable identity of one host-operation boundary contract.
identity_type!(HostOperationContractId);
// Semantic identity of a countable host resource contract.
identity_type!(ResourceClassId);
// Boot-scoped identity of one concrete host resource pool.
identity_type!(ResourcePoolId);
// Immutable identity of one authority contract and one issued grant.
identity_type!(AuthorityContractId);
identity_type!(AuthorityGrantId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OfferGeneration(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormIdentity {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivePlayIdentity {
    pub active_play_id: ActivePlayId,
    pub plan_id: PlanId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub activation_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceIdentity {
    pub evidence_id: EvidenceId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub active_play_id: Option<ActivePlayId>,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationIdentity {
    pub presentation_id: PresentationId,
    pub active_play_id: ActivePlayId,
    pub placement_id: PlacementId,
    pub sequence: u64,
}

pub fn bind_active_play(
    plan_id: &PlanId,
    host_id: &HostId,
    boot_id: &BootId,
    activation_sequence: u64,
) -> ActivePlayIdentity {
    let mut canonical = Vec::new();
    push_string(&mut canonical, "active-play");
    push_string(&mut canonical, plan_id.as_str());
    push_string(&mut canonical, host_id.as_str());
    push_string(&mut canonical, boot_id.as_str());
    push_u64(&mut canonical, activation_sequence);
    ActivePlayIdentity {
        active_play_id: ActivePlayId::from(hash_bytes(&canonical)),
        plan_id: plan_id.clone(),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        activation_sequence,
    }
}

pub fn bind_evidence(
    host_id: &HostId,
    boot_id: &BootId,
    active_play_id: Option<&ActivePlayId>,
    sequence: u64,
) -> EvidenceIdentity {
    let mut canonical = Vec::new();
    push_string(&mut canonical, "evidence");
    push_string(&mut canonical, host_id.as_str());
    push_string(&mut canonical, boot_id.as_str());
    push_string(
        &mut canonical,
        active_play_id.map_or("no-active-play", ActivePlayId::as_str),
    );
    push_u64(&mut canonical, sequence);
    EvidenceIdentity {
        evidence_id: EvidenceId::from(hash_bytes(&canonical)),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        active_play_id: active_play_id.cloned(),
        sequence,
    }
}

pub fn bind_presentation(
    active_play_id: &ActivePlayId,
    placement_id: &PlacementId,
    sequence: u64,
) -> PresentationIdentity {
    let mut canonical = Vec::new();
    push_string(&mut canonical, "presentation");
    push_string(&mut canonical, active_play_id.as_str());
    push_string(&mut canonical, placement_id.as_str());
    push_u64(&mut canonical, sequence);
    PresentationIdentity {
        presentation_id: PresentationId::from(hash_bytes(&canonical)),
        active_play_id: active_play_id.clone(),
        placement_id: placement_id.clone(),
        sequence,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortDescriptor {
    pub port_id: PortId,
    pub value_kind: KindId,
    pub direction: PortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationValue {
    Bool(bool),
    U64(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationEntry {
    pub key: String,
    pub value: ConfigurationValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValuePayload {
    pub value_kind: KindId,
    pub encoded: Vec<u8>,
}

impl ValuePayload {
    pub fn encoded_len(&self) -> u32 {
        self.encoded.len() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityLimits {
    pub max_active_instances: u16,
    pub max_queue_items: u16,
    pub max_queue_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HostOperationRequirement {
    pub contract_id: HostOperationContractId,
    pub target_kind: Option<KindId>,
    pub maximum_in_flight: u16,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceRequirement {
    pub class_id: ResourceClassId,
    pub units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceOffer {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub capacity_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceBinding {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuthorityRequirement {
    pub contract_id: AuthorityContractId,
    pub host_operation_contract_id: HostOperationContractId,
    pub subject_kind: KindId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuthorityGrant {
    pub grant_id: AuthorityGrantId,
    pub contract_id: AuthorityContractId,
    pub host_operation_contract_id: HostOperationContractId,
    pub subject_kind: KindId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuthorityBinding {
    pub grant_id: AuthorityGrantId,
    pub contract_id: AuthorityContractId,
    pub host_operation_contract_id: HostOperationContractId,
    pub subject_kind: KindId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOffer {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resource_requirements: Vec<ResourceRequirement>,
    pub authority_requirements: Vec<AuthorityRequirement>,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAdvertisement {
    pub protocol_version: u16,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub profile: HostProfileId,
    pub resources: Vec<ResourceOffer>,
    pub capabilities: Vec<CapabilityOffer>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionProvider {
    Local,
    InMemory,
    /// Deterministic bounded frame transit used only by conformance fixtures.
    FixtureFrame,
    /// Deterministic bounded datagram transit used only by conformance fixtures.
    FixtureDatagram,
    /// Actual RFC 6455 binary-message carrier. Availability is valid only for
    /// an initialized provider instance observed at exact boot-scoped endpoints.
    WebSocket,
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

/// One observed, directional remote-link fact. It identifies an initialized
/// provider instance but contains no provider configuration or secret material.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionEnvelope {
    pub protocol_version: u16,
    pub plan_id: PlanId,
    pub connection_id: ConnectionId,
    pub sequence: u64,
    pub value_kind: KindId,
    pub payload: Vec<u8>,
}

impl ConnectionEnvelope {
    pub fn encoded_len(&self) -> u32 {
        self.payload.len() as u32
    }

    pub fn into_value(self) -> ValuePayload {
        ValuePayload {
            value_kind: self.value_kind,
            encoded: self.payload,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionOutcome {
    Ready,
    Accepted,
    Full,
    Delivered,
    Disconnected,
    Malformed,
    Terminal,
}

pub mod lifecycle;
pub mod plan;

pub use lifecycle::{
    BoundedQueue, CancellationReason, ConnectionTerminalDisposition, FailureReason, HostCommand,
    HostEvent, Observation, ObservationKind, PlacementLifecycleState, PlatformEffect,
    TerminalDisposition, authority_grant, kind_id, port_id, present_authority_requirement,
    present_host_operation_requirement, process_owned_link_binding,
    process_owned_link_binding_with_limits, resource_offer, resource_requirement,
    wait_host_operation_requirement,
};
pub use plan::{
    CancellationPolicy, EvidenceStorageBudget, ExpectedEvidence, ExpectedTerminal,
    FragmentCommitment, MandatoryEvidenceReport, Plan, PlannedConnection, PlannedOperation,
    PlanFragment, StartupDependency, TerminalPolicy, compute_fragment_id,
    mandatory_evidence_storage_requirement, seal_plan, verify_plan, verify_plan_fragment,
};

pub(crate) fn compute_plan_id(form_identity: &FormIdentity, commitments: &[FragmentCommitment]) -> PlanId {
    let mut canonical = Vec::new();
    push_string(&mut canonical, form_identity.source_document_id.as_str());
    push_string(&mut canonical, form_identity.checked_form_id.as_str());
    push_string(&mut canonical, form_identity.expanded_form_id.as_str());
    push_u32(&mut canonical, commitments.len() as u32);
    for commitment in commitments {
        push_string(&mut canonical, commitment.host_id.as_str());
        push_string(&mut canonical, commitment.fragment_id.as_str());
    }
    PlanId::from(hash_bytes(&canonical))
}

pub(crate) fn push_ports(canonical: &mut Vec<u8>, ports: &[PortDescriptor]) {
    push_u32(canonical, ports.len() as u32);
    for port in ports {
        push_string(canonical, port.port_id.as_str());
        push_string(canonical, port.value_kind.as_str());
        canonical.push(match port.direction {
            PortDirection::Input => 0,
            PortDirection::Output => 1,
        });
    }
}

pub(crate) fn push_string(canonical: &mut Vec<u8>, value: &str) {
    push_u32(canonical, value.len() as u32);
    canonical.extend_from_slice(value.as_bytes());
}

pub(crate) fn push_u32(canonical: &mut Vec<u8>, value: u32) {
    canonical.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn push_u64(canonical: &mut Vec<u8>, value: u64) {
    canonical.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

pub(crate) fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!(),
    }
}

