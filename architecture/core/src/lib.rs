#![no_std]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod assigned_plan;
mod characteristic;
mod configuration;
mod control_loop;
mod deadline;
mod delivery;
mod device;
mod execution;
mod execution_fusion;
mod face;
mod implementation;
mod info;
mod plan_realization;
mod port;
mod preparation;
mod quantity;
mod resource;
mod resource_canonical;
mod resource_content;
use resource_canonical::push_resource_binding;
mod plan_fingerprint;
mod resource_admission;
mod resource_reference;
mod resource_reference_access;
mod route;
mod shared_pool;
mod state_delay;
pub use plan_fingerprint::compute_fragment_id;
use plan_fingerprint::compute_plan_id;
mod structured_info;
mod temporal;
mod temporal_civil_conversion;
mod temporal_clock;
mod temporal_quantity;

pub use assigned_plan::*;
pub use characteristic::*;
pub use configuration::{ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue};
pub use control_loop::*;
pub use deadline::*;
pub use delivery::*;
pub use device::*;
pub use execution::*;
pub use execution_fusion::*;
pub use face::{CheckedFace, FaceStartupParameter};
pub use implementation::{
    ImplementationOffer, RealizationAdvertisement, RealizationCharacteristic,
};
pub use info::*;
pub use plan_realization::RealizationBack;
pub use port::{PortDescriptor, PortDirection, PortTemporal};
pub use preparation::*;
pub use quantity::*;
pub use resource::*;
pub use resource_admission::*;
pub use resource_content::*;
pub use resource_reference::*;
pub use resource_reference_access::*;
pub use route::*;
pub use shared_pool::*;
pub use state_delay::*;
pub use structured_info::*;
pub use temporal::*;
pub use temporal_civil_conversion::*;
pub use temporal_clock::*;
pub use temporal_quantity::*;

pub const PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_CONNECTION_ITEM_CAPACITY: u16 = 4;
pub const DEFAULT_CONNECTION_BYTE_CAPACITY: u32 = 64;
pub const WAIT_HOST_OPERATION_CONTRACT: &str = "conduit.host/wait@1";
pub const PRESENT_HOST_OPERATION_CONTRACT: &str = "conduit.host/present@1";
pub const AWAIT_TRIGGER_HOST_OPERATION_CONTRACT: &str = "conduit.host/await-trigger@1";
pub const MAX_PRESENTATION_COMPLETION_BYTES: u32 = 256;
pub const TIMER_RESOURCE_CLASS: &str = "conduit.resource/timer-slot@1";
pub const RUNTIME_MEMORY_RESOURCE_CLASS: &str = "conduit.resource/runtime-memory@1";
pub const PRESENTATION_RESOURCE_CLASS: &str = "conduit.resource/presentation-slot@1";
pub const INPUT_RESOURCE_CLASS: &str = "conduit.resource/input-slot@1";
pub const PRESENT_AUTHORITY_CONTRACT: &str = "conduit.authority/present@1";
pub const SHARED_POOL_ADMIT_AUTHORITY_CONTRACT: &str = "conduit.authority/shared-pool-admit@1";
pub const SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT: &str = "conduit.host/shared-pool-admit@1";
pub const SHARED_POOL_AUTHORITY_SUBJECT_KIND: &str = "conduit/shared-pool";

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
identity_type!(DeviceId);
identity_type!(PlannerProfileId);
identity_type!(KindId);
// Immutable identity of one exact semantic-kind contract revision.
identity_type!(KindContractRevision);
// Immutable identity of one exact implementation execution profile.
identity_type!(ExecutionProfileId);
identity_type!(ExecutionRegionId);
identity_type!(FusionId);
identity_type!(ImplementationId);
identity_type!(ArtifactId);
identity_type!(CharacteristicId);
identity_type!(SourceDocumentId);
identity_type!(CheckedFormId);
identity_type!(ExpandedFormId);
identity_type!(PlanId);
identity_type!(ActivePlayId);
identity_type!(SignId);
identity_type!(PresentationId);
identity_type!(FragmentId);
identity_type!(PlacementId);
identity_type!(ConnectionId);
// Identity of one finite connectivity realization offered for Conduit traffic.
identity_type!(LineId);
// Identity of one observed, directional, boot-scoped remote link.
identity_type!(LinkBindingId);
// Base-owned identity of one exact initialized link endpoint.
identity_type!(LinkEndpointId);
// Exact versioned identity of one Base implementation. Concrete values are
// declared by the package that owns the implementation, not by core.
identity_type!(BaseImplementationId);
// Identity of one initialized base instance behind a link observation.
identity_type!(BaseInstanceId);
// Opaque reference only; credential material never enters a plan.
identity_type!(CredentialReferenceId);
identity_type!(PortId);
identity_type!(GearId);
identity_type!(HostProfileId);
// Immutable identity of one host-operation boundary contract.
identity_type!(HostOperationContractId);
identity_type!(HostOperationId);
// Semantic identity of a countable host resource contract.
identity_type!(ResourceClassId);
// Boot-scoped identity of one concrete host resource pool.
identity_type!(ResourcePoolId);
identity_type!(ResourceAllowanceSourceId);
// Semantic identity of one protected resource role within an gear.
identity_type!(ResourceBindingRoleId);
// Boot-scoped identity and stable kind identity for one concrete Host Base.
identity_type!(HostBaseId);
identity_type!(HostBaseKindId);
identity_type!(ArchitectureBaseId);
identity_type!(ComputeTopologyGroupId);
identity_type!(ComputeDomainId);
identity_type!(ComputePerformanceClassId);
identity_type!(BaseExecutionLaneId);
// Opaque base-owned reference; resource locator material never enters a plan.
identity_type!(ResourceHandleId);
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
    pub play_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignIdentity {
    pub sign_id: SignId,
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
    play_sequence: u64,
) -> ActivePlayIdentity {
    let digest = active_play_digest(
        plan_id.as_str(),
        host_id.as_str(),
        boot_id.as_str(),
        play_sequence,
    );
    ActivePlayIdentity {
        active_play_id: ActivePlayId::from(hex_digest(&digest)),
        plan_id: plan_id.clone(),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        play_sequence,
    }
}

/// Allocation-independent canonical digest for a boot-scoped active play.
/// Firmware can format this into fixed storage while hosted callers use
/// [`bind_active_play`] for the owned identity wrapper.
pub fn active_play_digest(
    plan_id: &str,
    host_id: &str,
    boot_id: &str,
    play_sequence: u64,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash_identity_string(&mut hash, "active-play");
    hash_identity_string(&mut hash, plan_id);
    hash_identity_string(&mut hash, host_id);
    hash_identity_string(&mut hash, boot_id);
    hash.update(play_sequence.to_le_bytes());
    hash.finalize().into()
}

fn hash_identity_string(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u32).to_le_bytes());
    hash.update(value.as_bytes());
}

pub fn bind_sign(
    host_id: &HostId,
    boot_id: &BootId,
    active_play_id: Option<&ActivePlayId>,
    sequence: u64,
) -> SignIdentity {
    let mut canonical = Vec::new();
    push_string(&mut canonical, "sign");
    push_string(&mut canonical, host_id.as_str());
    push_string(&mut canonical, boot_id.as_str());
    push_string(
        &mut canonical,
        active_play_id.map_or("no-active-play", ActivePlayId::as_str),
    );
    push_u64(&mut canonical, sequence);
    SignIdentity {
        sign_id: SignId::from(hash_bytes(&canonical)),
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
    #[serde(default)]
    pub startup_parameters: Vec<FaceStartupParameter>,
    #[serde(default)]
    pub shorthand: Option<(PortId, PortId)>,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    #[serde(flatten)]
    pub implementation: ImplementationOffer,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resource_requirements: Vec<ResourceRequirement>,
    pub authority_requirements: Vec<AuthorityRequirement>,
    pub limits: CapabilityLimits,
}

/// Finite request shape accepted by one optional planner profile.
///
/// These are admission limits, not hints. A planner must refuse before
/// planning when any supplied portable input exceeds the advertised shape.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerLimits {
    pub maximum_host_advertisements: u16,
    pub maximum_gears: u16,
    pub maximum_connections: u16,
    pub maximum_authority_grants: u16,
    #[serde(default)]
    pub maximum_protected_resource_grants: u16,
    pub maximum_line_offers: u16,
}

/// An optional host capability to perform deterministic Conduit planning.
///
/// The offer identifies a portable execution profile and its exact limits. It
/// deliberately contains no coordinator role, service endpoint, or delegation
/// target. Its host and boot scope come from the containing advertisement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerCapabilityOffer {
    pub profile_id: PlannerProfileId,
    pub limits: PlannerLimits,
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
    #[serde(default)]
    pub planner_capabilities: Vec<PlannerCapabilityOffer>,
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
pub struct PlannedGear {
    pub placement_id: PlacementId,
    pub gear_id: GearId,
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
    #[serde(default)]
    pub realization_characteristics: Vec<RealizationCharacteristic>,
    pub limits: CapabilityLimits,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resources: Vec<ResourceBinding>,
    pub authority: Vec<AuthorityBinding>,
    #[serde(default)]
    pub pool_references: Vec<SharedPoolId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedTerminal {
    PlacementCompleted(PlacementId),
    ConnectionCompleted(ConnectionId),
    PlanCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedSign {
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
pub struct SignStorageBudget {
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MandatorySignReport {
    pub plan_id: PlanId,
    pub expected: Vec<ExpectedSign>,
    pub recorded: Vec<ExpectedSign>,
    pub storage_budget: SignStorageBudget,
    pub allocated_item_slots: u32,
    pub used_bytes: u32,
    pub overflowed: bool,
}

pub fn mandatory_sign_storage_requirement(sign: &[ExpectedSign]) -> Option<SignStorageBudget> {
    let item_capacity = u16::try_from(sign.len()).ok()?;
    let mut byte_capacity = 0u32;
    for item in sign {
        let identity = match item {
            ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => None,
            ExpectedSign::PlacementPrepared(placement_id)
            | ExpectedSign::PlacementTerminal(placement_id) => Some(placement_id.as_str()),
            ExpectedSign::ConnectionTerminal(connection_id) => Some(connection_id.as_str()),
        };
        let identity_bytes = match identity {
            Some(value) => u32::try_from(value.len()).ok()?,
            None => 0,
        };
        byte_capacity = byte_capacity.checked_add(1)?.checked_add(identity_bytes)?;
    }
    Some(SignStorageBudget {
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
    #[serde(default)]
    pub temporal: PortTemporal,
    /// Initially selected exact Line. Local Cords have no Line.
    #[serde(default)]
    pub selected_line: Option<AdmittedLine>,
    /// Exact ordered permissible Lines. Runtime may select only from this
    /// immutable set; availability remains outside the Plan as Signs.
    #[serde(default)]
    pub admitted_lines: Vec<AdmittedLine>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

impl PlannedConnection {
    /// Whether an exact Line is inside this Cord's sealed realization set.
    pub fn permits_line(&self, line: &AdmittedLine) -> bool {
        self.admitted_lines.contains(line)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFragment {
    pub plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    #[serde(default)]
    pub realization_backs: Vec<RealizationBack>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub placements: Vec<PlannedGear>,
    #[serde(default)]
    pub execution_regions: Vec<ExecutionRegion>,
    #[serde(default)]
    pub execution_fusions: Vec<PlannedFusion>,
    /// Exact retained-State contracts owned by placements in this fragment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<PlannedStateBoundary>,
    pub connections: Vec<PlannedConnection>,
    #[serde(default)]
    pub shared_pools: Vec<PlannedSharedPool>,
    pub startup_dependencies: Vec<StartupDependency>,
    pub startup_order: Vec<PlacementId>,
    pub cancellation_policy: CancellationPolicy,
    pub terminal_policy: TerminalPolicy,
    pub expected_terminals: Vec<ExpectedTerminal>,
    pub expected_sign: Vec<ExpectedSign>,
    pub sign_storage_budget: SignStorageBudget,
    pub plan_fragments: Vec<FragmentCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub plan_id: PlanId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    /// Exact reusable Forms selected while expanding high-level Kinds.
    /// Empty means the checked Form reached primitive implementations directly.
    #[serde(default)]
    pub realization_backs: Vec<RealizationBack>,
    pub fragments: Vec<PlanFragment>,
}

pub fn seal_plan(form_identity: FormIdentity, fragments: Vec<PlanFragment>) -> Plan {
    seal_plan_with_realization_backs(form_identity, Vec::new(), fragments)
}

pub fn seal_plan_with_realization_backs(
    form_identity: FormIdentity,
    mut realization_backs: Vec<RealizationBack>,
    mut fragments: Vec<PlanFragment>,
) -> Plan {
    realization_backs.sort();
    for fragment in &mut fragments {
        fragment.plan_id = PlanId::from("");
        fragment.source_document_id = form_identity.source_document_id.clone();
        fragment.checked_form_id = form_identity.checked_form_id.clone();
        fragment.expanded_form_id = form_identity.expanded_form_id.clone();
        fragment.realization_backs = realization_backs.clone();
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
    let plan_id = compute_plan_id(&form_identity, &realization_backs, &commitments);
    for fragment in &mut fragments {
        fragment.plan_id = plan_id.clone();
        fragment.plan_fragments = commitments.clone();
    }
    Plan {
        plan_id,
        source_document_id: form_identity.source_document_id,
        checked_form_id: form_identity.checked_form_id,
        expanded_form_id: form_identity.expanded_form_id,
        realization_backs,
        fragments,
    }
}

pub fn verify_plan(plan: &Plan) -> bool {
    let form_identity = FormIdentity {
        source_document_id: plan.source_document_id.clone(),
        checked_form_id: plan.checked_form_id.clone(),
        expanded_form_id: plan.expanded_form_id.clone(),
    };
    let mut commitments = plan
        .fragments
        .iter()
        .map(|fragment| FragmentCommitment {
            host_id: fragment.host_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
        })
        .collect::<Vec<_>>();
    commitments.sort();
    plan.realization_backs
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        && plan.realization_backs.iter().all(|back| {
            !back.invocation_path.is_empty()
                && !back.kind_id.as_str().is_empty()
                && !back.kind_contract_revision.as_str().is_empty()
                && !back.source_document_id.as_str().is_empty()
                && !back.checked_form_id.as_str().is_empty()
        })
        && plan.plan_id == compute_plan_id(&form_identity, &plan.realization_backs, &commitments)
        && plan.fragments.iter().all(verify_plan_fragment)
        && plan.fragments.iter().all(|fragment| {
            fragment.plan_id == plan.plan_id
                && fragment.source_document_id == plan.source_document_id
                && fragment.checked_form_id == plan.checked_form_id
                && fragment.expanded_form_id == plan.expanded_form_id
                && fragment.realization_backs == plan.realization_backs
        })
        && plan
            .fragments
            .first()
            .is_none_or(|first| first.plan_fragments.len() == plan.fragments.len())
        && plan.fragments.first().is_none_or(|first| {
            first
                .shared_pools
                .iter()
                .all(|pool| pool.validate().is_ok())
                && plan
                    .fragments
                    .iter()
                    .all(|fragment| fragment.shared_pools == first.shared_pools)
        })
        && state_delay::verify_plan_states(plan)
        && verify_plan_shared_pools(plan)
        && verify_plan_connections(plan)
}

fn verify_plan_shared_pools(plan: &Plan) -> bool {
    let Some(first) = plan.fragments.first() else {
        return true;
    };
    let placements = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    for pool in &first.shared_pools {
        if pool.consumers.iter().any(|consumer| {
            placements
                .iter()
                .filter(|item| &item.placement_id == consumer)
                .count()
                != 1
        }) {
            return false;
        }
    }
    placements.iter().all(|placement| {
        placement.pool_references.iter().all(|reference| {
            first.shared_pools.iter().any(|pool| {
                &pool.pool_id == reference && pool.consumers.contains(&placement.placement_id)
            })
        }) && first.shared_pools.iter().all(|pool| {
            pool.consumers.contains(&placement.placement_id)
                == placement.pool_references.contains(&pool.pool_id)
        })
    })
}

fn verify_plan_connections(plan: &Plan) -> bool {
    let connections = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .collect::<Vec<_>>();
    for (index, connection) in connections.iter().enumerate() {
        if connections[..index]
            .iter()
            .any(|prior| prior.connection_id == connection.connection_id)
        {
            continue;
        }
        let occurrences = connections
            .iter()
            .filter(|candidate| candidate.connection_id == connection.connection_id)
            .collect::<Vec<_>>();
        if occurrences.iter().any(|candidate| *candidate != connection) {
            return false;
        }
        let source = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| placement.placement_id == connection.source_placement_id)
            .collect::<Vec<_>>();
        let sink = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| placement.placement_id == connection.sink_placement_id)
            .collect::<Vec<_>>();
        if source.len() != 1 || sink.len() != 1 {
            return false;
        }
        let source = source[0];
        let sink = sink[0];
        if source.host_id == sink.host_id {
            if occurrences.len() != 1
                || connection.selected_line.is_some()
                || !connection.admitted_lines.is_empty()
            {
                return false;
            }
        } else {
            let Some(selected) = &connection.selected_line else {
                return false;
            };
            if occurrences.len() != 2
                || connection.admitted_lines.is_empty()
                || !connection.permits_line(selected)
                || connection
                    .admitted_lines
                    .iter()
                    .enumerate()
                    .any(|(index, candidate)| {
                        connection.admitted_lines[..index].iter().any(|prior| {
                            prior.line_id == candidate.line_id
                                || prior.binding.binding_id == candidate.binding.binding_id
                        })
                    })
                || connection
                    .admitted_lines
                    .iter()
                    .any(|candidate| invalid_admitted_line(candidate, source, sink, connection))
            {
                return false;
            }
        }
    }
    true
}

fn invalid_admitted_line(
    candidate: &AdmittedLine,
    source: &PlannedGear,
    sink: &PlannedGear,
    connection: &PlannedConnection,
) -> bool {
    let binding = &candidate.binding;
    candidate.line_id.as_str().is_empty()
        || binding.binding_id.as_str().is_empty()
        || candidate.line_id.as_str() == binding.binding_id.as_str()
        || candidate.line_id.as_str() == binding.base_instance_id.as_str()
        || candidate.line_id.as_str() == binding.source.endpoint_id.as_str()
        || candidate.line_id.as_str() == binding.sink.endpoint_id.as_str()
        || binding.binding_id.as_str() == binding.base_instance_id.as_str()
        || binding.base == BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)
        || binding.base_instance_id.as_str().is_empty()
        || binding.source.host_id != source.host_id
        || binding.source.boot_id != source.boot_id
        || binding.source.endpoint_id.as_str().is_empty()
        || binding.sink.host_id != sink.host_id
        || binding.sink.boot_id != sink.boot_id
        || binding.sink.endpoint_id.as_str().is_empty()
        || binding.source.endpoint_id == binding.sink.endpoint_id
        || binding.limits.maximum_in_flight_items < connection.item_capacity
        || binding.limits.maximum_payload_bytes < connection.byte_capacity
        || binding.limits.maximum_buffered_bytes < connection.byte_capacity
        || binding.limits.maximum_frame_bytes < binding.limits.maximum_payload_bytes
        || matches!(
            &binding.credential,
            LinkCredentialReference::Opaque(reference) if reference.as_str().is_empty()
        )
        || matches!(
            &binding.authority,
            LinkAuthorityReference::Grant(grant_id) if grant_id.as_str().is_empty()
        )
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
        && state_delay::verify_fragment_state(fragment)
        && execution::verify_execution_regions(fragment)
        && execution_fusion::verify(fragment)
        && compute_plan_id(
            &FormIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_form_id: fragment.checked_form_id.clone(),
                expanded_form_id: fragment.expanded_form_id.clone(),
            },
            &fragment.realization_backs,
            &commitments,
        ) == fragment.plan_id
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
    hex_digest(&digest)
}

fn hex_digest(digest: &[u8]) -> String {
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
    SignGap,
    InvalidStartupDependencies,
    UnsupportedCancellationPolicy,
    UnsupportedTerminalPolicy,
    SignBudgetExceeded,
    HostOperationContractMismatch,
    HostOperationNotPlanned,
    HostOperationInputExceeded,
    HostOperationOutputExceeded,
    ResourceContractMismatch,
    ResourceCapacityExceeded,
    SharedPoolContractMismatch,
    AuthorityContractMismatch,
    AuthorityDenied,
    LinkBindingMismatch,
    LinkUnavailable,
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
    InvalidGearConfiguration,
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
    pub sign_id: SignId,
    pub active_play_id: Option<ActivePlayId>,
    pub presentation_id: Option<PresentationId>,
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
    PlanPlayStarted,
    ExecutionRegionOverlap {
        waiting_region_id: ExecutionRegionId,
        progressing_region_id: ExecutionRegionId,
        physical_parallelism: bool,
    },
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
    SignGap {
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
    StartPlay(PlanId),
    CompleteWait {
        plan_id: PlanId,
        placement_id: PlacementId,
    },
    CompletePresentation {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
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
    PlayStarted {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
    },
    PlayStartRejected {
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
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
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
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
        placement_id: PlacementId,
        value: ValuePayload,
    },
    ManifestationFailed {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
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
    MandatorySignReports {
        items: Vec<MandatorySignReport>,
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
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
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

/// Host-operation requirement for exactly one human/physical trigger input.
/// The platform adapter must block on the admitted input resource (e.g. stdin)
/// until the operator provides the trigger, then complete the request.
/// A 1-byte sequence counter is admitted as a correlation token (no timer semantics).
pub fn await_trigger_host_operation_requirement() -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(AWAIT_TRIGGER_HOST_OPERATION_CONTRACT),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 0,
    }
}

pub fn present_authority_requirement(subject_kind: KindId) -> AuthorityRequirement {
    AuthorityRequirement {
        contract_id: AuthorityContractId::from(PRESENT_AUTHORITY_CONTRACT),
        host_operation_contract_id: HostOperationContractId::from(PRESENT_HOST_OPERATION_CONTRACT),
        subject_kind,
    }
}

pub fn authority_grant(
    grant_id: &str,
    requirement: &AuthorityRequirement,
    host_id: HostId,
    boot_id: BootId,
    capability_id: CapabilityId,
) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from(grant_id),
        contract_id: requirement.contract_id.clone(),
        host_operation_contract_id: requirement.host_operation_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id,
        boot_id,
        capability_id,
    }
}

/// Build a ready Line offer for a base whose endpoint access is
/// wholly owned by the current process. This is suitable for deterministic
/// in-process fixtures; platform Line adapters should supply explicit credential
/// and grant references instead.
#[allow(clippy::too_many_arguments)]
pub fn process_owned_line_offer(
    line_id: &str,
    binding_id: &str,
    base: BaseImplementationId,
    base_instance_id: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    maximum_in_flight_items: u16,
    maximum_buffered_bytes: u32,
) -> LineOffer {
    process_owned_line_offer_with_limits(
        line_id,
        binding_id,
        base,
        base_instance_id,
        source,
        sink,
        LinkLimits {
            maximum_in_flight_items,
            maximum_payload_bytes: maximum_buffered_bytes,
            maximum_buffered_bytes,
            maximum_frame_bytes: maximum_buffered_bytes,
        },
    )
}

pub fn process_owned_line_offer_with_limits(
    line_id: &str,
    binding_id: &str,
    base: BaseImplementationId,
    base_instance_id: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    limits: LinkLimits,
) -> LineOffer {
    let binding = LinkBinding {
        binding_id: LinkBindingId::from(binding_id),
        source: LinkEndpoint {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
            endpoint_id: LinkEndpointId::from(format!("{binding_id}/source")),
        },
        sink: LinkEndpoint {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
            endpoint_id: LinkEndpointId::from(format!("{binding_id}/sink")),
        },
        base,
        base_instance_id: BaseInstanceId::from(base_instance_id),
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits,
    };
    LineOffer {
        line_id: LineId::from(line_id),
        availability: LineAvailabilitySign {
            line_id: LineId::from(line_id),
            binding_id: binding.binding_id.clone(),
            availability: LineAvailability::Ready,
            sign_id: SignId::from(format!("{line_id}/availability/ready")),
        },
        binding,
        contract: LineContract {
            scope: LineScope::Process,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::ProcessBoundary,
        },
    }
}

#[cfg(test)]
mod tests;
