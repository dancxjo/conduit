use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, AdmittedLine, ArtifactId, AuthorityBinding, AuthorityRequirement, BootId,
    CapabilityId, CapabilityLimits, CheckedFormId, ConnectionId, ConnectionTerminalDisposition,
    DeviceAssociation, ExecutionProfileId, ExecutionRegionId, ExecutionScheduling, ExpandedFormId,
    FragmentId, HostAdvertisement, HostBaseId, HostBaseKindId, HostId, HostOperationRequirement,
    HostProfileId, ImplementationId, KindContractRevision, KindId, LineOffer, Observation,
    OfferGeneration, PlacementId, Plan, PlanId, PlannerCapabilityOffer, PortDescriptor,
    PresentationId, ResourceBinding, ResourceOffer, ResourceRequirement, SignId, SourceDocumentId,
    TerminalDisposition,
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
    #[serde(default)]
    pub devices: Vec<DeviceAssociation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineReport {
    pub offer: LineOffer,
    pub state: OperationalState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseReport {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub base_id: HostBaseId,
    pub kind_id: HostBaseKindId,
    pub state: OperationalState,
    pub capacity_units: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootProofClass {
    Unknown,
    FreestandingEmulator,
    FirmwareExecution,
    PhysicalHil,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryMapSummary {
    pub normalized_region_count: u16,
    pub runtime_arena_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramebufferBasis {
    pub base_id: HostBaseId,
    pub width: u32,
    pub height: u32,
    pub pitch_bytes: u32,
    pub bits_per_pixel: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildInclusionPathReport {
    pub request: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageBuildTraceReport {
    pub profile_id: String,
    pub inclusions: Vec<BuildInclusionPathReport>,
}

/// Immutable historical boot facts. This is never a live offer, Base, or
/// authority source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedBootProvenanceReport {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub firmware_environment: String,
    pub adapter_name: String,
    pub adapter_version: String,
    pub adapter_revision: String,
    pub image_id: ArtifactId,
    pub build_id: ArtifactId,
    pub image_build_trace: Option<ImageBuildTraceReport>,
    pub memory_map: MemoryMapSummary,
    pub boot_artifacts: Vec<ArtifactId>,
    pub initial_plan_artifact_id: Option<ArtifactId>,
    pub recovery_plan_artifact_id: Option<ArtifactId>,
    pub framebuffers: Vec<FramebufferBasis>,
    pub proof_class: BootProofClass,
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
    pub bases: Vec<BaseReport>,
    pub lines: Vec<LineReport>,
    pub plans: Vec<Plan>,
    pub plays: Vec<PlayReport>,
    pub observations: Vec<Observation>,
    pub historical_observations: Vec<Observation>,
    pub sealed_boot_provenance: Vec<SealedBootProvenanceReport>,
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
pub struct DeviceRow {
    pub association: DeviceAssociation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRow {
    pub offer: LineOffer,
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
    pub execution_region_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRegionRow {
    pub plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub region_id: ExecutionRegionId,
    pub admitted_placements: Vec<PlacementId>,
    pub execution_profile_id: ExecutionProfileId,
    pub scheduling: ExecutionScheduling,
    pub lane_count: u32,
    pub lane_resource: ResourceBinding,
    pub lane_base_id: HostBaseId,
    pub requirements: conduit_core::ExecutionRegionRequirements,
    pub preemption_required: bool,
    pub isolation_required: bool,
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
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
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
    pub selected_line: Option<AdmittedLine>,
    pub admitted_lines: Vec<AdmittedLine>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignRow {
    pub sign_id: SignId,
    pub active_play_id: Option<ActivePlayId>,
    pub presentation_id: Option<PresentationId>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub plan_id: Option<PlanId>,
    pub placement_id: Option<PlacementId>,
    pub connection_id: Option<ConnectionId>,
    pub kind: conduit_core::ObservationKind,
    pub historical: bool,
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
    pub devices: Vec<DeviceRow>,
    pub bases: Vec<BaseReport>,
    pub lines: Vec<LineRow>,
    pub plans: Vec<PlanRow>,
    pub execution_regions: Vec<ExecutionRegionRow>,
    pub fragments: Vec<FragmentRow>,
    pub placements: Vec<PlacementRow>,
    pub connections: Vec<ConnectionRow>,
    pub plays: Vec<PlayReport>,
    pub signs: Vec<SignRow>,
    pub sealed_boot_provenance: Vec<SealedBootProvenanceReport>,
    pub retention: RetentionRow,
}
