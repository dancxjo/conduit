#![no_std]

extern crate alloc;

use alloc::collections::VecDeque;
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
identity_type!(FragmentId);
identity_type!(PlacementId);
identity_type!(ConnectionId);
identity_type!(PortId);
identity_type!(OperationId);
identity_type!(HostProfileId);
// Immutable identity of one host-operation boundary contract.
identity_type!(HostOperationContractId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OfferGeneration(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormIdentity {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
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
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAdvertisement {
    pub protocol_version: u16,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub profile: HostProfileId,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOperation {
    pub placement_id: PlacementId,
    pub operation_id: OperationId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub configuration: Vec<ConfigurationEntry>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub host_operations: Vec<HostOperationRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedTerminal {
    PlacementCompleted(PlacementId),
    ConnectionCompleted(ConnectionId),
    PlanCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedEvidence {
    PlanFragmentReceived,
    PlacementPrepared(PlacementId),
    PlacementTerminal(PlacementId),
    ConnectionTerminal(ConnectionId),
    PlanTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StartupDependency {
    pub prerequisite_placement_id: PlacementId,
    pub dependent_placement_id: PlacementId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationPolicy {
    CancelAllAndRejectLateCompletion,
    DrainBeforeCancel,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalPolicy {
    RequireAllPlacementsAndConnections,
    RequirePlacementsOnly,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceStorageBudget {
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MandatoryEvidenceReport {
    pub plan_id: PlanId,
    pub expected: Vec<ExpectedEvidence>,
    pub recorded: Vec<ExpectedEvidence>,
    pub storage_budget: EvidenceStorageBudget,
    pub allocated_item_slots: u32,
    pub used_bytes: u32,
    pub overflowed: bool,
}

pub fn mandatory_evidence_storage_requirement(
    evidence: &[ExpectedEvidence],
) -> Option<EvidenceStorageBudget> {
    let item_capacity = u16::try_from(evidence.len()).ok()?;
    let mut byte_capacity = 0u32;
    for item in evidence {
        let identity = match item {
            ExpectedEvidence::PlanFragmentReceived | ExpectedEvidence::PlanTerminal => None,
            ExpectedEvidence::PlacementPrepared(placement_id)
            | ExpectedEvidence::PlacementTerminal(placement_id) => Some(placement_id.as_str()),
            ExpectedEvidence::ConnectionTerminal(connection_id) => Some(connection_id.as_str()),
        };
        let identity_bytes = match identity {
            Some(value) => u32::try_from(value.len()).ok()?,
            None => 0,
        };
        byte_capacity = byte_capacity.checked_add(1)?.checked_add(identity_bytes)?;
    }
    Some(EvidenceStorageBudget {
        item_capacity,
        byte_capacity,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FragmentCommitment {
    pub host_id: HostId,
    pub fragment_id: FragmentId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedConnection {
    pub connection_id: ConnectionId,
    pub source_placement_id: PlacementId,
    pub source_port_id: PortId,
    pub sink_placement_id: PlacementId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
    pub provider: ConnectionProvider,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFragment {
    pub plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub placements: Vec<PlannedOperation>,
    pub connections: Vec<PlannedConnection>,
    pub startup_dependencies: Vec<StartupDependency>,
    pub startup_order: Vec<PlacementId>,
    pub cancellation_policy: CancellationPolicy,
    pub terminal_policy: TerminalPolicy,
    pub expected_terminals: Vec<ExpectedTerminal>,
    pub expected_evidence: Vec<ExpectedEvidence>,
    pub evidence_storage_budget: EvidenceStorageBudget,
    pub plan_fragments: Vec<FragmentCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub plan_id: PlanId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub fragments: Vec<PlanFragment>,
}

pub fn seal_plan(form_identity: FormIdentity, mut fragments: Vec<PlanFragment>) -> Plan {
    for fragment in &mut fragments {
        fragment.plan_id = PlanId::from("");
        fragment.source_document_id = form_identity.source_document_id.clone();
        fragment.checked_form_id = form_identity.checked_form_id.clone();
        fragment.expanded_form_id = form_identity.expanded_form_id.clone();
        fragment.fragment_id = compute_fragment_id(fragment);
        fragment.plan_fragments.clear();
    }
    let mut commitments = fragments
        .iter()
        .map(|fragment| FragmentCommitment {
            host_id: fragment.host_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
        })
        .collect::<Vec<_>>();
    commitments.sort();
    let plan_id = compute_plan_id(&form_identity, &commitments);
    for fragment in &mut fragments {
        fragment.plan_id = plan_id.clone();
        fragment.plan_fragments = commitments.clone();
    }
    Plan {
        plan_id,
        source_document_id: form_identity.source_document_id,
        checked_form_id: form_identity.checked_form_id,
        expanded_form_id: form_identity.expanded_form_id,
        fragments,
    }
}

pub fn verify_plan(plan: &Plan) -> bool {
    plan.fragments.iter().all(verify_plan_fragment)
        && plan.fragments.iter().all(|fragment| {
            fragment.plan_id == plan.plan_id
                && fragment.source_document_id == plan.source_document_id
                && fragment.checked_form_id == plan.checked_form_id
                && fragment.expanded_form_id == plan.expanded_form_id
        })
        && plan
            .fragments
            .first()
            .is_none_or(|first| first.plan_fragments.len() == plan.fragments.len())
}

pub fn verify_plan_fragment(fragment: &PlanFragment) -> bool {
    if compute_fragment_id(fragment) != fragment.fragment_id {
        return false;
    }
    let mut commitments = fragment.plan_fragments.clone();
    commitments.sort();
    if commitments != fragment.plan_fragments
        || commitments
            .windows(2)
            .any(|pair| pair[0].host_id == pair[1].host_id)
    {
        return false;
    }
    let own_matches = commitments
        .iter()
        .filter(|item| item.host_id == fragment.host_id && item.fragment_id == fragment.fragment_id)
        .count();
    own_matches == 1
        && compute_plan_id(
            &FormIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_form_id: fragment.checked_form_id.clone(),
                expanded_form_id: fragment.expanded_form_id.clone(),
            },
            &commitments,
        ) == fragment.plan_id
}

pub fn compute_fragment_id(fragment: &PlanFragment) -> FragmentId {
    let mut canonical = Vec::new();
    push_string(&mut canonical, fragment.source_document_id.as_str());
    push_string(&mut canonical, fragment.checked_form_id.as_str());
    push_string(&mut canonical, fragment.expanded_form_id.as_str());
    push_string(&mut canonical, fragment.host_id.as_str());
    push_string(&mut canonical, fragment.boot_id.as_str());
    push_u64(&mut canonical, fragment.offer_generation.0);
    push_u32(&mut canonical, fragment.placements.len() as u32);
    for operation in &fragment.placements {
        push_string(&mut canonical, operation.placement_id.as_str());
        push_string(&mut canonical, operation.operation_id.as_str());
        push_string(&mut canonical, operation.kind_id.as_str());
        push_string(&mut canonical, operation.kind_contract_revision.as_str());
        push_string(&mut canonical, operation.execution_profile_id.as_str());
        push_u32(&mut canonical, operation.configuration.len() as u32);
        for entry in &operation.configuration {
            push_string(&mut canonical, &entry.key);
            match entry.value {
                ConfigurationValue::Bool(value) => {
                    canonical.push(0);
                    canonical.push(u8::from(value));
                }
                ConfigurationValue::U64(value) => {
                    canonical.push(1);
                    push_u64(&mut canonical, value);
                }
            }
        }
        push_string(&mut canonical, operation.host_id.as_str());
        push_string(&mut canonical, operation.boot_id.as_str());
        push_u64(&mut canonical, operation.offer_generation.0);
        push_string(&mut canonical, operation.capability_id.as_str());
        push_string(&mut canonical, operation.implementation_id.as_str());
        push_string(&mut canonical, operation.artifact_id.as_str());
        push_ports(&mut canonical, &operation.inputs);
        push_ports(&mut canonical, &operation.outputs);
        push_u32(&mut canonical, operation.host_operations.len() as u32);
        for requirement in &operation.host_operations {
            push_string(&mut canonical, requirement.contract_id.as_str());
            match &requirement.target_kind {
                Some(target_kind) => {
                    canonical.push(1);
                    push_string(&mut canonical, target_kind.as_str());
                }
                None => canonical.push(0),
            }
            canonical.extend_from_slice(&requirement.maximum_in_flight.to_le_bytes());
            push_u32(&mut canonical, requirement.maximum_input_bytes);
            push_u32(&mut canonical, requirement.maximum_output_bytes);
        }
    }
    push_u32(&mut canonical, fragment.connections.len() as u32);
    for connection in &fragment.connections {
        push_string(&mut canonical, connection.connection_id.as_str());
        push_string(&mut canonical, connection.source_placement_id.as_str());
        push_string(&mut canonical, connection.source_port_id.as_str());
        push_string(&mut canonical, connection.sink_placement_id.as_str());
        push_string(&mut canonical, connection.sink_port_id.as_str());
        push_string(&mut canonical, connection.value_kind.as_str());
        canonical.push(match connection.provider {
            ConnectionProvider::Local => 0,
            ConnectionProvider::InMemory => 1,
            ConnectionProvider::FixtureFrame => 2,
            ConnectionProvider::FixtureDatagram => 3,
        });
        canonical.extend_from_slice(&connection.item_capacity.to_le_bytes());
        push_u32(&mut canonical, connection.byte_capacity);
    }
    push_u32(&mut canonical, fragment.startup_dependencies.len() as u32);
    for dependency in &fragment.startup_dependencies {
        push_string(
            &mut canonical,
            dependency.prerequisite_placement_id.as_str(),
        );
        push_string(&mut canonical, dependency.dependent_placement_id.as_str());
    }
    push_u32(&mut canonical, fragment.startup_order.len() as u32);
    for placement_id in &fragment.startup_order {
        push_string(&mut canonical, placement_id.as_str());
    }
    canonical.push(match fragment.cancellation_policy {
        CancellationPolicy::CancelAllAndRejectLateCompletion => 0,
        CancellationPolicy::DrainBeforeCancel => 1,
    });
    canonical.push(match fragment.terminal_policy {
        TerminalPolicy::RequireAllPlacementsAndConnections => 0,
        TerminalPolicy::RequirePlacementsOnly => 1,
    });
    push_u32(&mut canonical, fragment.expected_terminals.len() as u32);
    for terminal in &fragment.expected_terminals {
        match terminal {
            ExpectedTerminal::PlacementCompleted(placement_id) => {
                canonical.push(0);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedTerminal::ConnectionCompleted(connection_id) => {
                canonical.push(1);
                push_string(&mut canonical, connection_id.as_str());
            }
            ExpectedTerminal::PlanCompleted => canonical.push(2),
        }
    }
    push_u32(&mut canonical, fragment.expected_evidence.len() as u32);
    for evidence in &fragment.expected_evidence {
        match evidence {
            ExpectedEvidence::PlanFragmentReceived => canonical.push(0),
            ExpectedEvidence::PlacementPrepared(placement_id) => {
                canonical.push(1);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedEvidence::PlacementTerminal(placement_id) => {
                canonical.push(2);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedEvidence::ConnectionTerminal(connection_id) => {
                canonical.push(3);
                push_string(&mut canonical, connection_id.as_str());
            }
            ExpectedEvidence::PlanTerminal => canonical.push(4),
        }
    }
    canonical.extend_from_slice(&fragment.evidence_storage_budget.item_capacity.to_le_bytes());
    push_u32(
        &mut canonical,
        fragment.evidence_storage_budget.byte_capacity,
    );
    FragmentId::from(hash_bytes(&canonical))
}

fn compute_plan_id(form_identity: &FormIdentity, commitments: &[FragmentCommitment]) -> PlanId {
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

fn push_ports(canonical: &mut Vec<u8>, ports: &[PortDescriptor]) {
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

fn push_string(canonical: &mut Vec<u8>, value: &str) {
    push_u32(canonical, value.len() as u32);
    canonical.extend_from_slice(value.as_bytes());
}

fn push_u32(canonical: &mut Vec<u8>, value: u32) {
    canonical.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(canonical: &mut Vec<u8>, value: u64) {
    canonical.extend_from_slice(&value.to_le_bytes());
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!(),
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementLifecycleState {
    Proposed,
    Prepared,
    Active,
    Completed,
    Failed,
    Cancelled,
    Released,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureReason {
    WrongHostIdentity,
    StaleBootIdentity,
    StaleOfferGeneration,
    UnknownCapability,
    CapabilityInstanceLimitExceeded,
    QueueCapacityExceeded,
    ByteCapacityExceeded,
    ManifestationFailed,
    RequiredBranchFailed,
    InvalidLifecycleCommand,
    LatePlatformCompletion,
    EvidenceGap,
    InvalidStartupDependencies,
    UnsupportedCancellationPolicy,
    UnsupportedTerminalPolicy,
    EvidenceBudgetExceeded,
    HostOperationContractMismatch,
    HostOperationNotPlanned,
    HostOperationInputExceeded,
    HostOperationOutputExceeded,
    ConnectionDisconnected,
    MalformedConnectionEnvelope,
    StalePlan,
    CompositeCapabilityFailed,
    UnknownImplementation,
    UnsupportedKind,
    ImplementationKindMismatch,
    KindContractRevisionMismatch,
    ExecutionProfileMismatch,
    PortContractMismatch,
    AdvertisedImplementationMismatch,
    ArtifactIdentityMismatch,
    PlanIdentityMismatch,
    InvalidOperationConfiguration,
    UnsupportedValueKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationReason {
    OperatorRequested,
    RequiredPlanFailed,
    Released,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalDisposition {
    Completed,
    Failed { reason: FailureReason },
    Cancelled { reason: CancellationReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionTerminalDisposition {
    pub disposition: TerminalDisposition,
    pub last_accepted_sequence: Option<u64>,
    pub last_manifested_sequence: Option<u64>,
    pub undeliverable_items: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub plan_id: Option<PlanId>,
    pub placement_id: Option<PlacementId>,
    pub connection_id: Option<ConnectionId>,
    pub kind: ObservationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationKind {
    HostStarted,
    AdvertisementPublished,
    PlanFragmentReceived,
    PlacementPrepared,
    PlanActivated,
    ValueProduced {
        value: ValuePayload,
    },
    ValueAccepted {
        value: ValuePayload,
    },
    ValuePresented {
        value: ValuePayload,
    },
    PlacementCompleted,
    PlanCompleted,
    PlacementTerminal {
        disposition: TerminalDisposition,
    },
    ConnectionTerminal {
        disposition: ConnectionTerminalDisposition,
    },
    PlanTerminal {
        disposition: TerminalDisposition,
    },
    Failure {
        reason: FailureReason,
        message: Option<String>,
    },
    Cancelled,
    Released,
    EvidenceGap {
        dropped: u64,
    },
}

// The allocator-free host boundary keeps the sealed preparation fragment inline. Boxing the
// largest variant would make every no-std host provide allocation for command admission.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCommand {
    PublishAdvertisement(HostAdvertisement),
    Prepare(PlanFragment),
    Activate(PlanId),
    CompleteWait {
        plan_id: PlanId,
        placement_id: PlacementId,
    },
    CompletePresentation {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
        success: bool,
        message: Option<String>,
    },
    AcceptConnectionEnvelope(ConnectionEnvelope),
    CompleteConnectionDelivery {
        plan_id: PlanId,
        connection_id: ConnectionId,
        sequence: u64,
        outcome: ConnectionOutcome,
    },
    CloseConnection {
        plan_id: PlanId,
        connection_id: ConnectionId,
    },
    Cancel(PlanId),
    Release(PlanId),
    Inspect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostEvent {
    Prepared {
        plan_id: PlanId,
    },
    PreparationRejected {
        plan_id: PlanId,
        reason: FailureReason,
        message: Option<String>,
    },
    Activated {
        plan_id: PlanId,
    },
    ActivationRejected {
        plan_id: PlanId,
        reason: FailureReason,
        message: Option<String>,
    },
    TimerRequested {
        plan_id: PlanId,
        placement_id: PlacementId,
        duration_ms: u64,
    },
    PresentValueRequested {
        plan_id: PlanId,
        placement_id: PlacementId,
        presentation_kind: KindId,
        value: ValuePayload,
    },
    ConnectionBlocked {
        plan_id: PlanId,
        connection_id: ConnectionId,
    },
    ConnectionEnvelopeOutcome {
        plan_id: PlanId,
        connection_id: ConnectionId,
        sequence: u64,
        outcome: ConnectionOutcome,
    },
    ValueDelivered {
        plan_id: PlanId,
        connection_id: ConnectionId,
        value: ValuePayload,
    },
    ManifestationCompleted {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
    },
    ManifestationFailed {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
        reason: FailureReason,
        message: Option<String>,
    },
    PlacementCompleted {
        plan_id: PlanId,
        placement_id: PlacementId,
    },
    PlanCompleted {
        plan_id: PlanId,
    },
    PlacementTerminated {
        plan_id: PlanId,
        placement_id: PlacementId,
        disposition: TerminalDisposition,
    },
    ConnectionTerminated {
        plan_id: PlanId,
        connection_id: ConnectionId,
        disposition: ConnectionTerminalDisposition,
    },
    PlanTerminated {
        plan_id: PlanId,
        disposition: TerminalDisposition,
    },
    Cancelled {
        plan_id: PlanId,
    },
    Released {
        plan_id: PlanId,
    },
    CommandRejected {
        plan_id: Option<PlanId>,
        reason: FailureReason,
    },
    Observations {
        items: Vec<Observation>,
    },
    MandatoryEvidenceReports {
        items: Vec<MandatoryEvidenceReport>,
    },
}

/// Host-neutral work requested by an installed semantic implementation.
///
/// A std adapter may map `PresentValue` to stdout, a browser adapter may map it to DOM
/// presentation, and a Pico W adapter may map it to an LED. Those manifestations are adapter
/// policy; the semantic operation remains unaware of the platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformEffect {
    Wait {
        plan_id: PlanId,
        placement_id: PlacementId,
        duration_ms: u64,
    },
    PresentValue {
        plan_id: PlanId,
        placement_id: PlacementId,
        presentation_kind: KindId,
        value: ValuePayload,
    },
    TransmitConnection {
        envelope: ConnectionEnvelope,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedQueue<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> BoundedQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: VecDeque::new(),
        }
    }

    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.items.len() >= self.capacity {
            return Err(item);
        }
        self.items.push_back(item);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    pub fn front(&self) -> Option<&T> {
        self.items.front()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

pub fn kind_id(value: &str) -> KindId {
    KindId::from(value)
}

pub fn port_id(value: &str) -> PortId {
    PortId::from(value)
}

pub fn wait_host_operation_requirement() -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(WAIT_HOST_OPERATION_CONTRACT),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: core::mem::size_of::<u64>() as u32,
        maximum_output_bytes: 0,
    }
}

pub fn present_host_operation_requirement(
    target_kind: KindId,
    maximum_input_bytes: u32,
) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(PRESENT_HOST_OPERATION_CONTRACT),
        target_kind: Some(target_kind),
        maximum_in_flight: 1,
        maximum_input_bytes,
        maximum_output_bytes: MAX_PRESENTATION_COMPLETION_BYTES,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{
        mandatory_evidence_storage_requirement, EvidenceStorageBudget, ExpectedEvidence,
        PlacementId,
    };

    #[test]
    fn mandatory_evidence_budget_counts_items_and_identity_bytes_independently() {
        let evidence = vec![
            ExpectedEvidence::PlanFragmentReceived,
            ExpectedEvidence::PlacementPrepared(PlacementId::from("abc")),
            ExpectedEvidence::PlanTerminal,
        ];
        assert_eq!(
            mandatory_evidence_storage_requirement(&evidence),
            Some(EvidenceStorageBudget {
                item_capacity: 3,
                byte_capacity: 6,
            })
        );
    }
}
