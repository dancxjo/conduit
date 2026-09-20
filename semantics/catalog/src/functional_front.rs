use crate::StandardConfigurationField;
use alloc::vec::Vec;
use conduit_core::{
    ArtifactId, AuthorityRequirement, Back, BackOfferBuilder, CapabilityId, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, FrontStartupParameter, HostOperationRequirement,
    ImplementationId, Kind, KindIdentity, ResourceRequirement,
};

/// Host-supplied identity for one realization of a portable contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealizationOfferIdentity<'a> {
    pub capability: &'a str,
    pub execution_profile: &'a str,
    pub implementation: &'a str,
    pub artifact: &'a str,
}

/// Constructs one exact realization offer from portable contract truth and
/// explicitly supplied Host identity and requirements.
pub fn realization_offer(
    contract: crate::StandardKindContract,
    revision: &str,
    identity: RealizationOfferIdentity<'_>,
    host_operations: Vec<HostOperationRequirement>,
    resource_requirements: Vec<ResourceRequirement>,
    authority_requirements: Vec<AuthorityRequirement>,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        Kind {
            startup_parameters: startup_front(&contract.configuration),
            shorthand: None,
            kind_id: contract.kind_id,
            kind_contract_revision: KindIdentity::from(revision),
            inputs: contract.inputs,
            outputs: contract.outputs,
            limits: contract.limits,
        },
        Back {
            capability_id: CapabilityId::from(identity.capability),
            execution_profile_id: ExecutionProfileId::from(identity.execution_profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
            host_operations,
            resource_requirements,
            authority_requirements,
        },
    )
    .build()
}

pub fn startup_front(fields: &[StandardConfigurationField]) -> Vec<FrontStartupParameter> {
    fields
        .iter()
        .map(|field| FrontStartupParameter {
            name: field.key.clone(),
            value_type: conduit_core::kind_id(match field.default_value {
                ConfigurationValue::Bool(_) => "value/bool",
                ConfigurationValue::U64(_) => "value/count",
                ConfigurationValue::I64(_) => "value/scalar",
                ConfigurationValue::Text(_) => "value/text",
                ConfigurationValue::Quantity(_) => "value/quantity",
                ConfigurationValue::Structured(ref value) => value.profile().as_str(),
            }),
            has_default: true,
        })
        .collect()
}
