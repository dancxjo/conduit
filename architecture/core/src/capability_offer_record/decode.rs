//! Decode each known offer field directly instead of buffering flattened JSON.
//! This removes the flatten intermediate; nested typed allocations still require
//! a separately admitted preparation workspace.
use super::*;
use core::fmt;
use serde::{
    de::{value::MapAccessDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer,
};

#[derive(Deserialize)]
struct OfferFields {
    #[serde(default)]
    startup_parameters: Vec<FrontStartupParameter>,
    #[serde(default)]
    shorthand: Option<(PortId, PortId)>,
    capability_id: CapabilityId,
    kind_id: KindId,
    kind_contract_revision: KindIdentity,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    semantic_contract: KindSemanticContract,
    execution_profile_id: ExecutionProfileId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state_retention: Option<StateRetentionSupport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    realization_properties: Vec<StructuredConfigurationValue>,
    host_calls: Vec<HostCallRequirement>,
    resource_requirements: Vec<ResourceRequirement>,
    authority_requirements: Vec<AuthorityRequirement>,
    limits: CapabilityLimits,
}
impl From<OfferFields> for CapabilityOffer {
    fn from(value: OfferFields) -> Self {
        Self {
            startup_parameters: value.startup_parameters,
            shorthand: value.shorthand,
            capability_id: value.capability_id,
            kind_id: value.kind_id,
            kind_contract_revision: value.kind_contract_revision,
            inputs: value.inputs,
            outputs: value.outputs,
            semantic_contract: value.semantic_contract,
            implementation: ImplementationOffer {
                execution_profile_id: value.execution_profile_id,
                implementation_id: value.implementation_id,
                artifact_id: value.artifact_id,
            },
            state_retention: value.state_retention,
            realization_properties: value.realization_properties,
            host_calls: value.host_calls,
            resource_requirements: value.resource_requirements,
            authority_requirements: value.authority_requirements,
            limits: value.limits,
        }
    }
}
impl<'de> Deserialize<'de> for CapabilityOffer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OfferMap;
        impl<'de> Visitor<'de> for OfferMap {
            type Value = CapabilityOffer;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a capability offer map")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                OfferFields::deserialize(MapAccessDeserializer::new(map)).map(Into::into)
            }
        }
        // The former flattened derive rejected sequences. Keep that boundary.
        deserializer.deserialize_map(OfferMap)
    }
}

#[cfg(test)]
mod tests;
