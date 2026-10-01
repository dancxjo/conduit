//! Browser-owned realizations of portable structured-value contracts.

use conduit_core::{
    kind_id, present_host_call_requirement, resource_requirement, ArtifactId, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, ImplementationId,
    StructuredInfoType, StructuredInfoValue, PRESENTATION_RESOURCE_CLASS,
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
    default_value: &StructuredInfoValue,
    identity: BrowserOfferIdentity<'_>,
) -> CapabilityOffer {
    exact_offer(
        conduit_semantic_catalog::structured_literal_semantic_contract(
            type_name,
            value_type,
            default_value,
        )
        .expect("checked structured literal has one exact semantic contract"),
        identity,
        false,
    )
}

pub(crate) fn structured_presentation_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
    identity: BrowserOfferIdentity<'_>,
) -> CapabilityOffer {
    exact_offer(
        conduit_semantic_catalog::structured_presentation_semantic_contract(type_name, value_type),
        identity,
        true,
    )
}

fn exact_offer(
    contract: conduit_core::Kind,
    identity: BrowserOfferIdentity<'_>,
    presentation: bool,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(identity.capability),
            execution_profile_id: ExecutionProfileId::from(identity.profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
            host_calls: if presentation {
                vec![present_host_call_requirement(
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
        },
    )
    .build()
}
