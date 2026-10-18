//! Explicit hosted primitive Quantity arithmetic profile and sealed bounds.
use conduit_core::{
    kind_id, CapabilityOffer, PortTemporal, QuantityValueProfile, QuantityWireContract,
    StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_plot::{
    PortableExpressionNode, PortableExpressionOperation as Op, PortableExpressionProgram,
};

pub const PURE_EXACT_QUANTITY_STD_PROFILE: &str = "std/pure-expression-quantity-exact-hosted@1";
pub const PURE_EXACT_QUANTITY_STD_ARTIFACT: &str = "conduit-plot/pure-expression-quantity-exact@1";

/// This incremental installed profile covers primitive arithmetic/comparison.
/// Identity-only and composite programs retain their separately offered profiles.
pub fn pure_exact_quantity_std_offer(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, String> {
    if temporal != PortTemporal::Value {
        return Err("exact Quantity profile requires primitive Value arithmetic/comparison".into());
    }
    let mut remaining = conduit_core::MAXIMUM_STRUCTURED_INFO_NODES;
    if !has_quantity_operation(&program.root, 1, &mut remaining)? {
        return Err("exact Quantity profile requires primitive arithmetic/comparison".into());
    }
    let evaluator =
        conduit_plot::PreparedPortableExpressionEvaluator::new_with_exact_quantities(program)
            .map_err(|why| format!("exact Quantity preparation: {why:?}"))?;
    let (nodes, depth, payload, work) = evaluator
        .exact_quantity_profile_bounds()
        .ok_or("exact Quantity storage profile is not reviewed")?;
    let mut offer = super::pure_expression_std_offer(program, temporal)
        .map_err(|why| format!("exact Quantity Fore: {why:?}"))?;
    offer.capability_id = format!("std/exact-quantity/{}", offer.kind_id.as_str()).into();
    offer.implementation.execution_profile_id = PURE_EXACT_QUANTITY_STD_PROFILE.into();
    offer.implementation.artifact_id = PURE_EXACT_QUANTITY_STD_ARTIFACT.into();
    let inputs = if quantity_type(&program.input_type) {
        vec![("input".into(), QuantityValueProfile::Exact)]
    } else {
        vec![]
    };
    let outputs = if quantity_type(&program.output_type) {
        vec![("output".into(), QuantityValueProfile::Exact)]
    } else {
        vec![]
    };
    let wire = QuantityWireContract::new(inputs, outputs)
        .map_err(|why| format!("exact Quantity ports: {why:?}"))?
        .to_property()
        .map_err(|why| format!("exact Quantity wire property: {why:?}"))?;
    let input = program.input_maximum_bytes.map_or_else(
        || {
            if quantity_type(&program.input_type) {
                Ok(conduit_core::QUANTITY_VALUE_MAX_ENCODED_BYTES as u32)
            } else {
                program
                    .maximum_prepared_input_bytes()
                    .map_err(|why| format!("exact input bound: {why:?}"))
            }
        },
        Ok,
    )?;
    let bounds = bounds_property(&[
        ("maximum-nodes", nodes as u64),
        ("maximum-depth", depth as u64),
        ("retained-prepared-payload-ceiling-bytes", payload as u64),
        ("maximum-numerical-work", work),
        ("maximum-input-bytes", u64::from(input)),
        ("output-capacity-bytes", evaluator.output_capacity() as u64),
    ])?;
    offer.realization_properties = vec![wire, bounds];
    offer
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    Ok(offer)
}
fn quantity_type(ty: &StructuredInfoType) -> bool {
    matches!(ty.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::QUANTITY_INFO_ID || conduit_core::quantity_info_dimension(kind.as_str()).is_some() || conduit_core::parse_quantity_role_info_id(kind.as_str()).is_some())
}
fn has_quantity_operation(
    node: &PortableExpressionNode,
    depth: usize,
    remaining: &mut usize,
) -> Result<bool, String> {
    if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH || *remaining == 0 {
        return Err("exact Quantity operation discovery exceeds portable program bounds".into());
    }
    *remaining -= 1;
    let mut child =
        |node: &PortableExpressionNode| has_quantity_operation(node, depth + 1, remaining);
    Ok(match &node.operation {
        Op::Binary { left, right, .. } => {
            quantity_type(&left.value_type)
                || quantity_type(&right.value_type)
                || child(left)?
                || child(right)?
        }
        Op::Unary { operand, .. } => child(operand)?,
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => child(condition)? || child(when_true)? || child(when_false)?,
        Op::SemanticCall { arguments, .. } => {
            let mut found = false;
            for argument in arguments {
                found |= child(argument)?;
            }
            found
        }
        _ => false,
    })
}
fn bounds_property(fields: &[(&str, u64)]) -> Result<StructuredConfigurationValue, String> {
    let build = || -> Result<_, conduit_core::StructuredInfoRefusal> {
        let integer = StructuredInfoType::leaf(kind_id("value/u64"))?;
        let ty = StructuredInfoType::record(
            kind_id("std/exact-quantity-expression-bounds@1"),
            fields
                .iter()
                .map(|(name, _)| StructuredFieldType::new(*name, integer.clone()))
                .collect::<Result<_, _>>()?,
        )?;
        let value = StructuredInfoValue::record(
            ty,
            fields
                .iter()
                .map(|(name, n)| {
                    StructuredFieldValue::new(
                        *name,
                        StructuredInfoValue::leaf(integer.clone(), n.to_le_bytes().to_vec())?,
                    )
                })
                .collect::<Result<_, _>>()?,
        )?;
        Ok((
            value.value_type().profile()?.value_kind().clone(),
            value.canonical_bytes()?,
        ))
    };
    let (profile, bytes) = build().map_err(|why| format!("exact Quantity bounds: {why:?}"))?;
    StructuredConfigurationValue::new(profile, bytes).ok_or("invalid exact Quantity bounds".into())
}
