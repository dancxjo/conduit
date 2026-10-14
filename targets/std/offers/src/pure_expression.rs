//! Exact pure-expression realization offers owned by the hosted std Host.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, PortTemporal,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const PURE_EXPRESSION_STD_PROFILE: &str = "std/pure-expression-kernel-hosted@1";
pub const PURE_EXPRESSION_STD_IMPLEMENTATION: &str = "std/kernel-pure-expression@1";
pub const PURE_EXPRESSION_STD_ARTIFACT: &str = "conduit-plot/pure-expression@1";
pub const PURE_EXPRESSION_HOST_CALL: &str = "conduit.host/pure-expression@1";
pub const PURE_FILTER_STD_PROFILE: &str = "std/pure-filter-kernel-hosted@1";
pub const PURE_FILTER_STD_IMPLEMENTATION: &str = "std/kernel-pure-filter@1";
pub const PURE_FILTER_STD_ARTIFACT: &str = "conduit-plot/pure-filter@1";
pub const PURE_FILTER_HOST_CALL: &str = "conduit.host/pure-filter@1";

pub fn pure_expression_std_offer(
    program: &conduit_plot::PortableExpressionProgram,
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
    program: &conduit_plot::PortableExpressionProgram,
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

pub const PURE_SCALAR_TAPE_STD_PROFILE: &str = "std/pure-scalar-tape-kernel-hosted@1";
pub const PURE_SCALAR_TAPE_STD_IMPLEMENTATION: &str = "std/kernel-pure-scalar-tape@1";
pub const PURE_SCALAR_TAPE_STD_ARTIFACT: &str = "conduit-plot/pure-scalar-tape@1";
/// Explicit bounded profile selection; unsupported shapes do not become a tape.
pub fn pure_scalar_tape_std_offer(
    program: &conduit_plot::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, String> {
    if temporal != PortTemporal::Value {
        return Err("scalar tape profile requires Value temporal contract".into());
    }
    let evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new_scalar_tape(program)
        .map_err(|e| format!("scalar tape profile refusal: {e:?}"))?;
    let limits = evaluator
        .scalar_tape_limits()
        .ok_or("scalar tape limits missing")?;
    if limits.0 > 128 {
        return Err("scalar tape instruction limit exceeded".into());
    }
    let mut offer = pure_expression_std_offer(program, temporal)
        .map_err(|e| format!("scalar tape contract: {e:?}"))?;
    offer.capability_id = format!("std/scalar-tape/{}", offer.kind_id.as_str()).into();
    offer.implementation.execution_profile_id = PURE_SCALAR_TAPE_STD_PROFILE.into();
    offer.implementation.implementation_id = PURE_SCALAR_TAPE_STD_IMPLEMENTATION.into();
    offer.implementation.artifact_id = PURE_SCALAR_TAPE_STD_ARTIFACT.into();
    Ok(offer)
}
