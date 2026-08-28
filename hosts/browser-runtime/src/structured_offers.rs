//! Browser-owned realizations of portable structured-value contracts.

use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement, ArtifactId, CapabilityId,
    CapabilityOffer, ExecutionProfileId, ImplementationId, ImplementationOffer, StructuredInfoType,
    PRESENTATION_RESOURCE_CLASS,
};

pub(crate) struct BrowserOfferIdentity<'a> {
    pub capability: &'a str,
    pub profile: &'a str,
    pub implementation: &'a str,
    pub artifact: &'a str,
}

pub(crate) fn structured_literal_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
    identity: BrowserOfferIdentity<'_>,
) -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::structured_literal_contract(type_name, value_type),
        identity,
        false,
    )
}

pub(crate) fn structured_presentation_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
    identity: BrowserOfferIdentity<'_>,
) -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::structured_presentation_contract(type_name, value_type),
        identity,
        true,
    )
}

fn offer(
    contract: conduit_semantic_catalog::StructuredValueContract,
    identity: BrowserOfferIdentity<'_>,
    presentation: bool,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: contract.startup_parameters,
        shorthand: None,
        capability_id: CapabilityId::from(identity.capability),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(identity.profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: if presentation {
            vec![present_host_operation_requirement(
                kind_id(conduit_semantic_catalog::STRUCTURED_PRESENTATION_TARGET),
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            )]
        } else {
            Vec::new()
        },
        resource_requirements: if presentation {
            vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
        } else {
            Vec::new()
        },
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}
