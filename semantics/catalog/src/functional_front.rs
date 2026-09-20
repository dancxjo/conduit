use crate::StandardConfigurationField;
use alloc::vec::Vec;
use conduit_core::{
    ArtifactId, AuthorityRequirement, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, FaceStartupParameter, HostOperationRequirement, ImplementationId,
    KindContractRevision, ResourceRequirement,
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
    CapabilityOffer {
        startup_parameters: startup_front(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(identity.capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(identity.execution_profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations,
        resource_requirements,
        authority_requirements,
        limits: contract.limits,
    }
}

pub fn startup_front(fields: &[StandardConfigurationField]) -> Vec<FaceStartupParameter> {
    fields
        .iter()
        .map(|field| FaceStartupParameter {
            name: field.key.clone(),
            value_type: conduit_core::kind_id(match field.default_value {
                ConfigurationValue::Bool(_) => "value/bool",
                ConfigurationValue::U64(_) => "value/count",
                ConfigurationValue::I64(_) => "value/scalar",
                ConfigurationValue::Text(_) => "value/text",
                ConfigurationValue::Structured(ref value) => value.profile().as_str(),
            }),
            has_default: true,
        })
        .collect()
}
