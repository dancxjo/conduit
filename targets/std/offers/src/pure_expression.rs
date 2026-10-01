//! Exact pure-expression realization offers owned by the hosted std Host.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, PortTemporal,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const PURE_EXPRESSION_STD_PROFILE: &str = "std/pure-expression-kernel-hosted@1";
pub const PURE_EXPRESSION_STD_IMPLEMENTATION: &str = "std/kernel-pure-expression@1";
pub const PURE_EXPRESSION_STD_ARTIFACT: &str = "conduit-form/pure-expression@1";
pub const PURE_EXPRESSION_HOST_CALL: &str = "conduit.host/pure-expression@1";
pub const PURE_FILTER_STD_PROFILE: &str = "std/pure-filter-kernel-hosted@1";
pub const PURE_FILTER_STD_IMPLEMENTATION: &str = "std/kernel-pure-filter@1";
pub const PURE_FILTER_STD_ARTIFACT: &str = "conduit-form/pure-filter@1";
pub const PURE_FILTER_HOST_CALL: &str = "conduit.host/pure-filter@1";

pub fn pure_expression_std_offer(
    program: &conduit_form::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, conduit_core::StructuredInfoRefusal> {
    let contract = conduit_semantic_catalog::pure_expression_contract(program, temporal)?;
    let target_kind = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("std/{}", target_kind.as_str())),
            execution_profile_id: ExecutionProfileId::from(PURE_EXPRESSION_STD_PROFILE),
            implementation_id: ImplementationId::from(PURE_EXPRESSION_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(PURE_EXPRESSION_STD_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(PURE_EXPRESSION_HOST_CALL),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

pub fn pure_filter_std_offer(
    program: &conduit_form::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, conduit_core::StructuredInfoRefusal> {
    let contract = conduit_semantic_catalog::pure_filter_contract(program, temporal)?;
    let target_kind = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("std/{}", target_kind.as_str())),
            execution_profile_id: ExecutionProfileId::from(PURE_FILTER_STD_PROFILE),
            implementation_id: ImplementationId::from(PURE_FILTER_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(PURE_FILTER_STD_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(PURE_FILTER_HOST_CALL),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}
