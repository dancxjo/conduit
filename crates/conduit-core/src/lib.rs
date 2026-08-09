#![no_std]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod configuration;
mod control_loop;
mod face;
mod implementation;
mod port;
mod resource;
mod route;
mod shared_pool;

pub use configuration::{ConfigurationEntry, ConfigurationValue};
pub use control_loop::*;
pub use face::{CheckedFace, FaceStartupParameter};
pub use implementation::{
    ImplementationOffer, RealizationAdvertisement, RealizationCharacteristic,
    RealizationCharacteristicValue,
};
pub use port::{PortDescriptor, PortDirection, PortTemporal};
pub use resource::*;
pub use route::*;
pub use shared_pool::*;

pub const PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_CONNECTION_ITEM_CAPACITY: u16 = 4;
pub const DEFAULT_CONNECTION_BYTE_CAPACITY: u32 = 64;
pub const WAIT_HOST_OPERATION_CONTRACT: &str = "conduit.host/wait@1";
pub const PRESENT_HOST_OPERATION_CONTRACT: &str = "conduit.host/present@1";
pub const AWAIT_TRIGGER_HOST_OPERATION_CONTRACT: &str = "conduit.host/await-trigger@1";
pub const MAX_PRESENTATION_COMPLETION_BYTES: u32 = 256;
pub const TIMER_RESOURCE_CLASS: &str = "conduit.resource/timer-slot@1";
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
identity_type!(PlannerProfileId);
identity_type!(KindId);
// Immutable identity of one exact semantic-kind contract revision.
identity_type!(KindContractRevision);
// Immutable identity of one exact implementation execution profile.
identity_type!(ExecutionProfileId);
identity_type!(ImplementationId);
identity_type!(ArtifactId);
identity_type!(RealizationCharacteristicId);
identity_type!(SourceDocumentId);
identity_type!(CheckedFormId);
identity_type!(ExpandedFormId);
identity_type!(PlanId);
identity_type!(ActivePlayId);
identity_type!(ClueId);
identity_type!(PresentationId);
identity_type!(FragmentId);
identity_type!(PlacementId);
identity_type!(ConnectionId);
// Identity of one observed, directional, boot-scoped remote link.
identity_type!(LinkBindingId);
// Base-owned identity of one exact initialized link endpoint.
identity_type!(LinkEndpointId);
// Identity of one initialized base instance behind a link observation.
identity_type!(ConnectionBaseInstanceId);
// Opaque reference only; credential material never enters a plan.
identity_type!(CredentialReferenceId);
identity_type!(PortId);
identity_type!(GearId);
identity_type!(HostProfileId);
// Immutable identity of one host-operation boundary contract.
identity_type!(HostOperationContractId);
// Semantic identity of a countable host resource contract.
identity_type!(ResourceClassId);
// Boot-scoped identity of one concrete host resource pool.
identity_type!(ResourcePoolId);
// Semantic identity of one protected resource role within an gear.
identity_type!(ResourceBindingRoleId);
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
pub struct ClueIdentity {
    pub clue_id: ClueId,
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

pub fn bind_clue(
    host_id: &HostId,
    boot_id: &BootId,
    active_play_id: Option<&ActivePlayId>,
    sequence: u64,
) -> ClueIdentity {
    let mut canonical = Vec::new();
    push_string(&mut canonical, "clue");
    push_string(&mut canonical, host_id.as_str());
    push_string(&mut canonical, boot_id.as_str());
    push_string(
        &mut canonical,
        active_play_id.map_or("no-active-play", ActivePlayId::as_str),
    );
    push_u64(&mut canonical, sequence);
    ClueIdentity {
        clue_id: ClueId::from(hash_bytes(&canonical)),
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
    pub maximum_link_bindings: u16,
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
pub enum ExpectedClue {
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
pub struct ClueStorageBudget {
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MandatoryClueReport {
    pub plan_id: PlanId,
    pub expected: Vec<ExpectedClue>,
    pub recorded: Vec<ExpectedClue>,
    pub storage_budget: ClueStorageBudget,
    pub allocated_item_slots: u32,
    pub used_bytes: u32,
    pub overflowed: bool,
}

pub fn mandatory_clue_storage_requirement(clue: &[ExpectedClue]) -> Option<ClueStorageBudget> {
    let item_capacity = u16::try_from(clue.len()).ok()?;
    let mut byte_capacity = 0u32;
    for item in clue {
        let identity = match item {
            ExpectedClue::PlanFragmentReceived | ExpectedClue::PlanTerminal => None,
            ExpectedClue::PlacementPrepared(placement_id)
            | ExpectedClue::PlacementTerminal(placement_id) => Some(placement_id.as_str()),
            ExpectedClue::ConnectionTerminal(connection_id) => Some(connection_id.as_str()),
        };
        let identity_bytes = match identity {
            Some(value) => u32::try_from(value.len()).ok()?,
            None => 0,
        };
        byte_capacity = byte_capacity.checked_add(1)?.checked_add(identity_bytes)?;
    }
    Some(ClueStorageBudget {
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
    pub base: ConnectionBase,
    pub link_binding: Option<LinkBinding>,
    /// Exact ordered permissible routes. Empty retains the legacy single-link
    /// representation; new remote plans seal at least one immutable candidate.
    #[serde(default)]
    pub route_candidates: Vec<BoundLink>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

impl PlannedConnection {
    /// Whether an immutable link is inside this connection's sealed route set.
    pub fn permits_bound_link(&self, link: &BoundLink) -> bool {
        if self.route_candidates.is_empty() {
            self.link_binding
                .as_ref()
                .is_some_and(|binding| binding.bound_link() == *link)
        } else {
            self.route_candidates.contains(link)
        }
    }
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
    pub placements: Vec<PlannedGear>,
    pub connections: Vec<PlannedConnection>,
    #[serde(default)]
    pub shared_pools: Vec<PlannedSharedPool>,
    pub startup_dependencies: Vec<StartupDependency>,
    pub startup_order: Vec<PlacementId>,
    pub cancellation_policy: CancellationPolicy,
    pub terminal_policy: TerminalPolicy,
    pub expected_terminals: Vec<ExpectedTerminal>,
    pub expected_clue: Vec<ExpectedClue>,
    pub clue_storage_budget: ClueStorageBudget,
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
                || connection.base != ConnectionBase::Local
                || connection.link_binding.is_some()
                || !connection.route_candidates.is_empty()
            {
                return false;
            }
        } else {
            let Some(binding) = &connection.link_binding else {
                return false;
            };
            let candidates = if connection.route_candidates.is_empty() {
                alloc::vec![binding.bound_link()]
            } else {
                connection.route_candidates.clone()
            };
            if occurrences.len() != 2
                || connection.base == ConnectionBase::Local
                || binding.binding_id.as_str().is_empty()
                || binding.base != connection.base
                || binding.base_instance_id.as_str().is_empty()
                || !connection.permits_bound_link(&binding.bound_link())
                || candidates
                    .iter()
                    .enumerate()
                    .any(|(index, candidate)| candidates[..index].contains(candidate))
                || candidates
                    .iter()
                    .any(|candidate| invalid_bound_link(candidate, source, sink, connection))
            {
                return false;
            }
        }
    }
    true
}

fn invalid_bound_link(
    candidate: &BoundLink,
    source: &PlannedGear,
    sink: &PlannedGear,
    connection: &PlannedConnection,
) -> bool {
    candidate.binding_id.as_str().is_empty()
        || candidate.base == ConnectionBase::Local
        || candidate.base_instance_id.as_str().is_empty()
        || candidate.source.host_id != source.host_id
        || candidate.source.boot_id != source.boot_id
        || candidate.source.endpoint_id.as_str().is_empty()
        || candidate.sink.host_id != sink.host_id
        || candidate.sink.boot_id != sink.boot_id
        || candidate.sink.endpoint_id.as_str().is_empty()
        || candidate.source.endpoint_id == candidate.sink.endpoint_id
        || candidate.limits.maximum_in_flight_items < connection.item_capacity
        || candidate.limits.maximum_payload_bytes < connection.byte_capacity
        || candidate.limits.maximum_buffered_bytes < connection.byte_capacity
        || candidate.limits.maximum_frame_bytes < candidate.limits.maximum_payload_bytes
        || matches!(
            &candidate.credential,
            LinkCredentialReference::Opaque(reference) if reference.as_str().is_empty()
        )
        || matches!(
            &candidate.authority,
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
    for gear in &fragment.placements {
        push_string(&mut canonical, gear.placement_id.as_str());
        push_string(&mut canonical, gear.gear_id.as_str());
        push_string(&mut canonical, gear.kind_id.as_str());
        push_string(&mut canonical, gear.kind_contract_revision.as_str());
        push_string(&mut canonical, gear.execution_profile_id.as_str());
        push_u32(&mut canonical, gear.configuration.len() as u32);
        for entry in &gear.configuration {
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
                ConfigurationValue::Text(ref value) => {
                    canonical.push(2);
                    push_string(&mut canonical, value);
                }
            }
        }
        push_string(&mut canonical, gear.host_id.as_str());
        push_string(&mut canonical, gear.boot_id.as_str());
        push_u64(&mut canonical, gear.offer_generation.0);
        push_string(&mut canonical, gear.capability_id.as_str());
        push_string(&mut canonical, gear.implementation_id.as_str());
        push_string(&mut canonical, gear.artifact_id.as_str());
        push_u32(
            &mut canonical,
            gear.realization_characteristics.len() as u32,
        );
        for characteristic in &gear.realization_characteristics {
            push_string(&mut canonical, characteristic.characteristic_id.as_str());
            match &characteristic.value {
                RealizationCharacteristicValue::Count(value) => {
                    canonical.push(0);
                    push_u64(&mut canonical, *value);
                }
                RealizationCharacteristicValue::Flag(value) => {
                    canonical.push(1);
                    canonical.push(u8::from(*value));
                }
                RealizationCharacteristicValue::Label(value) => {
                    canonical.push(2);
                    push_string(&mut canonical, value);
                }
            }
        }
        canonical.extend_from_slice(&gear.limits.max_active_instances.to_le_bytes());
        canonical.extend_from_slice(&gear.limits.max_queue_items.to_le_bytes());
        push_u32(&mut canonical, gear.limits.max_queue_bytes);
        push_ports(&mut canonical, &gear.inputs);
        push_ports(&mut canonical, &gear.outputs);
        push_u32(&mut canonical, gear.host_operations.len() as u32);
        for requirement in &gear.host_operations {
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
        push_u32(&mut canonical, gear.resources.len() as u32);
        for binding in &gear.resources {
            push_string(&mut canonical, binding.pool_id.as_str());
            push_string(&mut canonical, binding.class_id.as_str());
            push_u32(&mut canonical, binding.units);
            match &binding.compute {
                Some(compute) => {
                    canonical.push(1);
                    push_u32(&mut canonical, compute.selected_lanes);
                    canonical.push(compute.service_guarantee as u8);
                    push_string(&mut canonical, compute.architecture_base_id.as_str());
                    canonical.push(compute.architecture_base_kind as u8);
                    match &compute.topology_group_id {
                        Some(group) => {
                            canonical.push(1);
                            push_string(&mut canonical, group.as_str());
                        }
                        None => canonical.push(0),
                    }
                }
                None => canonical.push(0),
            }
            match &binding.protected {
                Some(protected) => {
                    canonical.push(1);
                    push_string(&mut canonical, protected.role_id.as_str());
                    push_string(&mut canonical, protected.handle_id.as_str());
                    canonical.push(protected.access as u8);
                    push_u64(&mut canonical, protected.maximum_bytes);
                    canonical.push(protected.commit_policy as u8);
                }
                None => canonical.push(0),
            }
        }
        push_u32(&mut canonical, gear.authority.len() as u32);
        for binding in &gear.authority {
            push_string(&mut canonical, binding.grant_id.as_str());
            push_string(&mut canonical, binding.contract_id.as_str());
            push_string(&mut canonical, binding.host_operation_contract_id.as_str());
            push_string(&mut canonical, binding.subject_kind.as_str());
            push_string(&mut canonical, binding.host_id.as_str());
            push_string(&mut canonical, binding.boot_id.as_str());
            push_string(&mut canonical, binding.capability_id.as_str());
        }
        push_u32(&mut canonical, gear.pool_references.len() as u32);
        for pool in &gear.pool_references {
            push_string(&mut canonical, pool.as_str());
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
        canonical.push(match connection.temporal {
            PortTemporal::Value => 0,
            PortTemporal::Flow { closes: false } => 1,
            PortTemporal::Flow { closes: true } => 2,
            PortTemporal::Current => 3,
        });
        if connection.route_candidates.is_empty() {
            canonical.push(connection.base.canonical_code());
            match &connection.link_binding {
                Some(binding) => {
                    canonical.push(1);
                    push_bound_link(&mut canonical, &binding.bound_link());
                }
                None => canonical.push(0),
            }
        } else {
            push_u32(&mut canonical, connection.route_candidates.len() as u32);
            for candidate in &connection.route_candidates {
                push_bound_link(&mut canonical, candidate);
            }
        }
        canonical.extend_from_slice(&connection.item_capacity.to_le_bytes());
        push_u32(&mut canonical, connection.byte_capacity);
    }
    push_u32(&mut canonical, fragment.shared_pools.len() as u32);
    for pool in &fragment.shared_pools {
        push_string(&mut canonical, pool.pool_id.as_str());
        push_string(&mut canonical, pool.declaration_id.as_str());
        push_checked_face(&mut canonical, &pool.member_face);
        canonical.extend_from_slice(&pool.maximum_members.to_le_bytes());
        canonical.extend_from_slice(&pool.member_limits.queue_item_capacity.to_le_bytes());
        push_u32(&mut canonical, pool.member_limits.queue_byte_capacity);
        canonical.extend_from_slice(&pool.member_limits.clue_item_capacity.to_le_bytes());
        push_u32(&mut canonical, pool.member_limits.clue_byte_capacity);
        push_u32(&mut canonical, pool.realization_envelope.len() as u32);
        for realization in &pool.realization_envelope {
            push_string(&mut canonical, realization.host_id.as_str());
            push_string(&mut canonical, realization.boot_id.as_str());
            push_string(&mut canonical, realization.capability_id.as_str());
            canonical.extend_from_slice(&realization.member_capacity.to_le_bytes());
            push_u32(&mut canonical, realization.resources.len() as u32);
            for resource in &realization.resources {
                push_string(&mut canonical, resource.pool_id.as_str());
                push_string(&mut canonical, resource.class_id.as_str());
                push_u32(&mut canonical, resource.units);
            }
        }
        push_string(&mut canonical, pool.admission_authority.as_str());
        push_u32(&mut canonical, pool.consumers.len() as u32);
        for consumer in &pool.consumers {
            push_string(&mut canonical, consumer.as_str());
        }
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
    push_u32(&mut canonical, fragment.expected_clue.len() as u32);
    for clue in &fragment.expected_clue {
        match clue {
            ExpectedClue::PlanFragmentReceived => canonical.push(0),
            ExpectedClue::PlacementPrepared(placement_id) => {
                canonical.push(1);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedClue::PlacementTerminal(placement_id) => {
                canonical.push(2);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedClue::ConnectionTerminal(connection_id) => {
                canonical.push(3);
                push_string(&mut canonical, connection_id.as_str());
            }
            ExpectedClue::PlanTerminal => canonical.push(4),
        }
    }
    canonical.extend_from_slice(&fragment.clue_storage_budget.item_capacity.to_le_bytes());
    push_u32(&mut canonical, fragment.clue_storage_budget.byte_capacity);
    FragmentId::from(hash_bytes(&canonical))
}

fn push_checked_face(canonical: &mut Vec<u8>, face: &CheckedFace) {
    push_u32(canonical, face.startup_parameters().len() as u32);
    for parameter in face.startup_parameters() {
        push_string(canonical, &parameter.name);
        push_string(canonical, &parameter.value_type);
        canonical.push(u8::from(parameter.has_default));
    }
    push_ports(canonical, face.inputs());
    push_ports(canonical, face.outputs());
    match face.shorthand() {
        Some((input, output)) => {
            canonical.push(1);
            push_string(canonical, input.as_str());
            push_string(canonical, output.as_str());
        }
        None => canonical.push(0),
    }
}

fn push_bound_link(canonical: &mut Vec<u8>, binding: &BoundLink) {
    push_string(canonical, binding.binding_id.as_str());
    push_string(canonical, binding.source.host_id.as_str());
    push_string(canonical, binding.source.boot_id.as_str());
    push_string(canonical, binding.source.endpoint_id.as_str());
    push_string(canonical, binding.sink.host_id.as_str());
    push_string(canonical, binding.sink.boot_id.as_str());
    push_string(canonical, binding.sink.endpoint_id.as_str());
    canonical.push(binding.base.canonical_code());
    push_string(canonical, binding.base_instance_id.as_str());
    match &binding.credential {
        LinkCredentialReference::None => canonical.push(0),
        LinkCredentialReference::Opaque(reference) => {
            canonical.push(1);
            push_string(canonical, reference.as_str());
        }
    }
    match &binding.authority {
        LinkAuthorityReference::ProcessOwned => canonical.push(0),
        LinkAuthorityReference::Grant(grant_id) => {
            canonical.push(1);
            push_string(canonical, grant_id.as_str());
        }
    }
    canonical.extend_from_slice(&binding.limits.maximum_in_flight_items.to_le_bytes());
    push_u32(canonical, binding.limits.maximum_payload_bytes);
    push_u32(canonical, binding.limits.maximum_buffered_bytes);
    push_u32(canonical, binding.limits.maximum_frame_bytes);
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
        canonical.push(match port.temporal {
            PortTemporal::Value => 0,
            PortTemporal::Flow { closes: false } => 1,
            PortTemporal::Flow { closes: true } => 2,
            PortTemporal::Current => 3,
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
    ClueGap,
    InvalidStartupDependencies,
    UnsupportedCancellationPolicy,
    UnsupportedTerminalPolicy,
    ClueBudgetExceeded,
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
    pub clue_id: ClueId,
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
    ClueGap {
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
    MandatoryClueReports {
        items: Vec<MandatoryClueReport>,
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

/// Build a ready link observation for a base whose endpoint access is
/// wholly owned by the current process. This is suitable for deterministic
/// in-process fixtures; actual carriers should supply explicit credential and
/// grant references instead.
pub fn process_owned_link_binding(
    binding_id: &str,
    base: ConnectionBase,
    base_instance_id: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    maximum_in_flight_items: u16,
    maximum_buffered_bytes: u32,
) -> LinkBinding {
    process_owned_link_binding_with_limits(
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

pub fn process_owned_link_binding_with_limits(
    binding_id: &str,
    base: ConnectionBase,
    base_instance_id: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    limits: LinkLimits,
) -> LinkBinding {
    LinkBinding {
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
        base_instance_id: ConnectionBaseInstanceId::from(base_instance_id),
        availability: LinkAvailability::Ready,
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits,
    }
}

#[cfg(test)]
mod tests;
