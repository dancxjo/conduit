//! Serialized PlannedGear schema; validation and construction retain their existing owners.
use crate::*;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedGear {
    pub placement_id: PlacementId,
    pub gear_id: GearId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    /// Authored provenance where one exists. Generated realization machinery
    /// remains honestly spanless rather than borrowing a nearby location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SourceSpan>,
    pub execution_profile_id: ExecutionProfileId,
    pub configuration: Vec<ConfigurationEntry>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    /// Exact current provider for Base-backed work. Pure work remains `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<BaseProviderBinding>,
    #[serde(default)]
    pub realization_characteristics: Vec<RealizationCharacteristic>,
    /// Exact selected Back facts, independently sealed from authored meaning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub realization_properties: Vec<StructuredConfigurationValue>,
    pub limits: CapabilityLimits,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub semantic_contract: KindSemanticContract,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminal_transductions: Vec<TerminalTransductionProfile>,
    pub host_calls: Vec<HostCallRequirement>,
    pub resources: Vec<ResourceBinding>,
    pub authority: Vec<AuthorityBinding>,
    #[serde(default)]
    pub pool_references: Vec<SharedPoolId>,
}
