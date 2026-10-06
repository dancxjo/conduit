//! Serialized CapabilityOffer schema; validation and construction retain their existing owners.
use crate::*;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOffer {
    #[serde(default)]
    pub startup_parameters: Vec<FrontStartupParameter>,
    #[serde(default)]
    pub shorthand: Option<(PortId, PortId)>,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub semantic_contract: KindSemanticContract,
    #[serde(flatten)]
    pub implementation: ImplementationOffer,
    /// Exact keep-duration support of this Back. Absence means that the Back
    /// makes no retained-State promise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_retention: Option<StateRetentionSupport>,
    /// Exact finite domain-owned realization facts. Empty means undeclared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub realization_properties: Vec<StructuredConfigurationValue>,
    pub host_calls: Vec<HostCallRequirement>,
    pub resource_requirements: Vec<ResourceRequirement>,
    pub authority_requirements: Vec<AuthorityRequirement>,
    pub limits: CapabilityLimits,
}
