use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, AuthorityBinding, AuthorityRequirement, BootId, CapabilityId, CapabilityLimits,
    CheckedFormId, ClueId, ConnectionBase, ConnectionId, ConnectionTerminalDisposition,
    ExecutionProfileId, ExpandedFormId, FragmentId, HostAdvertisement, HostId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision, KindId,
    LinkBinding, Observation, OfferGeneration, PlacementId, Plan, PlanId, PlannerCapabilityOffer,
    PortDescriptor, PresentationId, ResourceBinding, ResourceOffer, ResourceRequirement,
    SourceDocumentId, TerminalDisposition,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationalState {
    Available,
    Stale,
    Unreachable,
    Failed,
    Unsupported,
    Denied,
    Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferFreshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityAvailability {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanLifecycle {
    Unknown,
    Prepared,
    Active,
    Completed,
    Failed,
    Cancelled,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityStatusReport {
    pub capability_id: CapabilityId,
    pub freshness: OfferFreshness,
    pub support: CapabilitySupport,
    pub availability: CapabilityAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostReport {
    pub advertisement: HostAdvertisement,
    pub state: OperationalState,
    pub capabilities: Vec<CapabilityStatusReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkReport {
    pub binding: LinkBinding,
    pub state: OperationalState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PressureReport {
    pub current_in_flight_items: Option<u16>,
    pub current_buffered_bytes: Option<u32>,
    pub pressure_events: u64,
    pub last_pressure_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayPlacementReport {
    pub placement_id: PlacementId,
    pub lifecycle: PlanLifecycle,
    pub terminal_disposition: Option<TerminalDisposition>,
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayConnectionReport {
    pub connection_id: ConnectionId,
    pub lifecycle: PlanLifecycle,
    pub terminal_disposition: Option<ConnectionTerminalDisposition>,
    pub pressure: Option<PressureReport>,
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayReport {
    pub active_play_id: ActivePlayId,
    pub plan_id: PlanId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub lifecycle: PlanLifecycle,
    pub terminal_disposition: Option<TerminalDisposition>,
    pub failure_message: Option<String>,
    pub placements: Vec<PlayPlacementReport>,
    pub connections: Vec<PlayConnectionReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionReport {
    pub item_capacity: u32,
    pub retained_items: u32,
    pub dropped_items: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservatorySnapshot {
    pub schema: String,
    pub hosts: Vec<HostReport>,
    pub links: Vec<LinkReport>,
    pub plans: Vec<Plan>,
    pub plays: Vec<PlayReport>,
    pub observations: Vec<Observation>,
    pub retention: RetentionReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostRow {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub profile: HostProfileId,
    pub offer_generation: OfferGeneration,
    pub state: OperationalState,
    pub capability_count: usize,
    pub planner_capabilities: Vec<PlannerCapabilityOffer>,
    pub resources: Vec<ResourceOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRow {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resource_requirements: Vec<ResourceRequirement>,
    pub authority_requirements: Vec<AuthorityRequirement>,
    pub limits: CapabilityLimits,
    pub freshness: OfferFreshness,
    pub support: CapabilitySupport,
    pub availability: CapabilityAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRow {
    pub binding: LinkBinding,
    pub state: OperationalState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanRow {
    pub plan_id: PlanId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub fragment_count: usize,
    pub placement_count: usize,
    pub connection_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentRow {
    pub plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub host_id: HostId,
    pub boot_id: BootId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementRow {
    pub plan_id: PlanId,
    pub placement_id: PlacementId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resources: Vec<ResourceBinding>,
    pub authority: Vec<AuthorityBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionRow {
    pub plan_id: PlanId,
    pub connection_id: ConnectionId,
    pub source_placement_id: PlacementId,
    pub sink_placement_id: PlacementId,
    pub value_kind: KindId,
    pub base: ConnectionBase,
    pub link_binding: Option<LinkBinding>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClueRow {
    pub clue_id: ClueId,
    pub active_play_id: Option<ActivePlayId>,
    pub presentation_id: Option<PresentationId>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub plan_id: Option<PlanId>,
    pub placement_id: Option<PlacementId>,
    pub connection_id: Option<ConnectionId>,
    pub kind: conduit_core::ObservationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionRow {
    pub bounded: bool,
    pub item_capacity: u32,
    pub retained_items: u32,
    pub visible_gap_count: u64,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservatoryReport {
    pub hosts: Vec<HostRow>,
    pub capabilities: Vec<CapabilityRow>,
    pub links: Vec<LinkRow>,
    pub plans: Vec<PlanRow>,
    pub fragments: Vec<FragmentRow>,
    pub placements: Vec<PlacementRow>,
    pub connections: Vec<ConnectionRow>,
    pub plays: Vec<PlayReport>,
    pub clues: Vec<ClueRow>,
    pub retention: RetentionRow,
}
