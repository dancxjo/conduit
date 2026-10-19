//! Exact owned computational Value alternative for a checked pure expression.
use crate::{prepared_expression_bounds, pure_expression_contract, PreparedExpressionBack};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_plot::PortableExpressionProgram;
pub const EXPRESSION_VALUE_PROFILE: &str = "conduit.expression/prepared-value@1";
pub const EXPRESSION_VALUE_IMPLEMENTATION: &str = "conduit.expression/kernel-local-value@1";
const ARTIFACT: &str = "conduit-plot/prepared-pure-expression@1";
pub fn prepared_expression_value_offer(
    program: &PortableExpressionProgram,
) -> Result<CapabilityOffer, String> {
    let kind = pure_expression_contract(program, PortTemporal::Value)
        .map_err(|error| format!("expression contract: {error:?}"))?;
    let (input, output) = prepared_expression_bounds(program)?;
    let mut limits = kind.limits.clone();
    limits.max_active_instances = limits.max_active_instances.min(16);
    limits.max_queue_bytes = limits.max_queue_bytes.min(input.max(output).max(1));
    let back = Back {
        capability_id: format!("{EXPRESSION_VALUE_PROFILE}/{}", kind.kind_id.as_str()).into(),
        execution_profile_id: EXPRESSION_VALUE_PROFILE.into(),
        implementation_id: EXPRESSION_VALUE_IMPLEMENTATION.into(),
        artifact_id: ARTIFACT.into(),
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
    };
    derive_transformation_eligibility(&kind, &back)
        .and_then(|eligibility| eligibility.require(WorkTransformation::Recompute))
        .map_err(|error| format!("computational expression semantic eligibility: {error:?}"))?;
    BackOfferBuilder::new(kind, back)
        .narrow_capacity(limits)
        .map_err(|error| format!("computational expression capacity: {error:?}"))
        .map(BackOfferBuilder::build)
}
pub fn prepare_expression_value(placement: &PlannedGear) -> Result<PreparedExpressionBack, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("one exact expression program required".into());
    };
    let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("canonical expression program required".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|error| format!("expression program: {error:?}"))?;
    let expected = prepared_expression_value_offer(&program)?;
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.capability_id != expected.capability_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.host_calls != expected.host_calls
        || placement.base.is_some()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.realization_characteristics.is_empty()
        || !placement.terminal_transductions.is_empty()
    {
        return Err("selected expression differs from exact owned Value profile".into());
    }
    PreparedExpressionBack::prepare(&program)
}
