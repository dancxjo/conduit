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
    offer.realization_properties = vec![scalar_tape_property(&evaluator)?];
    Ok(offer)
}

/// Inspectable preparation bounds, sealed by ordinary Plan fingerprinting.
/// These payload bounds are distinct from kernel fuel and total Host memory.
pub const PURE_SCALAR_TAPE_BOUNDS: &str = "std/pure-scalar-tape-bounds@1";
fn scalar_tape_property(
    evaluator: &conduit_plot::PreparedPortableExpressionEvaluator,
) -> Result<conduit_core::StructuredConfigurationValue, String> {
    use conduit_core::{
        kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
    };
    let (instructions, scratch) = evaluator.scalar_tape_limits().ok_or("missing tape")?;
    let fields = [
        ("maximum-instructions", instructions),
        ("scratch-payload-bytes", scratch),
        (
            "retained-tape-payload-bytes",
            evaluator
                .scalar_tape_storage_bytes()
                .ok_or("missing tape storage")?,
        ),
        ("output-capacity-bytes", evaluator.output_capacity()),
    ];
    let build = || -> Result<_, conduit_core::StructuredInfoRefusal> {
        let integer = StructuredInfoType::leaf(kind_id("value/u64"))?;
        let ty = StructuredInfoType::record(
            kind_id(PURE_SCALAR_TAPE_BOUNDS),
            fields
                .iter()
                .map(|(name, _)| StructuredFieldType::new(*name, integer.clone()))
                .collect::<Result<_, _>>()?,
        )?;
        let value = StructuredInfoValue::record(
            ty,
            fields
                .iter()
                .map(|(name, value)| {
                    StructuredFieldValue::new(
                        *name,
                        StructuredInfoValue::leaf(
                            integer.clone(),
                            (*value as u64).to_le_bytes().to_vec(),
                        )?,
                    )
                })
                .collect::<Result<_, _>>()?,
        )?;
        Ok((
            value.value_type().profile()?.value_kind().clone(),
            value.canonical_bytes()?,
        ))
    };
    let (profile, bytes) = build().map_err(|e| format!("scalar tape bounds: {e:?}"))?;
    conduit_core::StructuredConfigurationValue::new(profile, bytes)
        .ok_or("invalid tape bounds".into())
}

/// Explicit computational profile for an existing-kernel child Value Gear.
/// This retains the exact checked program Fore and requests no platform effect.
pub const PURE_EXPRESSION_LOCAL_PROFILE: &str = "std/pure-expression-kernel-local@1";
pub const PURE_EXPRESSION_LOCAL_IMPLEMENTATION: &str = "std/kernel-local-pure-expression@1";
pub const PURE_EXPRESSION_LOCAL_ARTIFACT: &str = "conduit-plot/prepared-pure-expression@1";
pub fn pure_expression_local_offer(
    program: &conduit_plot::PortableExpressionProgram,
) -> Result<CapabilityOffer, conduit_core::StructuredInfoRefusal> {
    let mut offer = pure_expression_std_offer(program, PortTemporal::Value)?;
    offer.capability_id = format!("std/local/{}", offer.kind_id.as_str()).into();
    offer.implementation.execution_profile_id = PURE_EXPRESSION_LOCAL_PROFILE.into();
    offer.implementation.implementation_id = PURE_EXPRESSION_LOCAL_IMPLEMENTATION.into();
    offer.implementation.artifact_id = PURE_EXPRESSION_LOCAL_ARTIFACT.into();
    offer.host_calls.clear();
    Ok(offer)
}

/// Closing Flow counterpart using the same prepared evaluator and bounded storage.
pub fn pure_expression_local_flow_offer(
    program: &conduit_plot::PortableExpressionProgram,
) -> Result<CapabilityOffer, conduit_core::StructuredInfoRefusal> {
    let mut offer = pure_expression_std_offer(program, PortTemporal::Flow { closes: true })?;
    offer.capability_id = format!("std/local-flow/{}", offer.kind_id.as_str()).into();
    offer.implementation.execution_profile_id = PURE_EXPRESSION_LOCAL_PROFILE.into();
    offer.implementation.implementation_id = PURE_EXPRESSION_LOCAL_IMPLEMENTATION.into();
    offer.implementation.artifact_id = PURE_EXPRESSION_LOCAL_ARTIFACT.into();
    offer.host_calls.clear();
    Ok(offer)
}

mod quantity_expression;
pub use quantity_expression::*;
