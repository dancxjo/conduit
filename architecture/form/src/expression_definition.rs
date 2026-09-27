//! Exact ordinary Gear contract for one checked pure expression.

use crate::{
    hash_string, CheckedExpression, KindConfigurationField, KindConfigurationRule, KindProjection,
    PortableExpressionProgram, MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoRefusal,
};

pub const PURE_EXPRESSION_REVISION: &str = "conduitese/pure-expression-operation@1";
pub const PURE_FILTER_REVISION: &str = "conduitese/pure-filter-operation@1";

/// Projects a checked expression into the ordinary Kind/Fore vocabulary used
/// by expansion and planning. The source spelling and spans are absent from the
/// identity; exact checked input, result, temporal law and canonical operation
/// determine it.
pub fn pure_expression_definition(
    expression: &CheckedExpression,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
    let program =
        PortableExpressionProgram::from_checked(expression).map_err(|refusal| match refusal {
            crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
            _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
        })?;
    portable_expression_definition(&program, temporal)
}

/// Projects a checked Boolean predicate into a flow-preserving filter. A
/// rejected item produces no output; closure remains the input flow's closure.
pub fn pure_filter_definition(
    expression: &CheckedExpression,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
    if expression.value_type != crate::CheckedExpressionType::semantic(conduit_core::BOOL_INFO_ID)
        || !matches!(temporal, PortTemporal::Flow { .. })
    {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    let program =
        PortableExpressionProgram::from_checked(expression).map_err(|refusal| match refusal {
            crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
            _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
        })?;
    portable_filter_definition(&program, temporal)
}

pub fn portable_filter_definition(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
    if program.output_type
        != conduit_core::StructuredInfoType::leaf(kind_id(conduit_core::BOOL_INFO_ID))?
        || !matches!(temporal, PortTemporal::Flow { .. })
    {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    let encoded_program = program.canonical_hex().map_err(|refusal| match refusal {
        crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
        _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
    })?;
    let value = expression_port_kind(&program.input_type)?;
    let identity = hash_string(&format!(
        "pure-filter:{PURE_FILTER_REVISION}:{}:{}:{}",
        value.as_str(),
        temporal.as_str(),
        encoded_program
    ));
    Ok(KindProjection {
        kind_id: kind_id(&format!("conduitese/pure-filter/{identity}")),
        kind_contract_revision: KindIdentity::from(PURE_FILTER_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: value.clone(),
            direction: PortDirection::Input,
            temporal,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: value,
            direction: PortDirection::Output,
            temporal,
        }],
        configuration: vec![KindConfigurationField {
            key: "program".into(),
            default_value: ConfigurationValue::Text(encoded_program),
            rule: KindConfigurationRule::TextBytes {
                maximum: (MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES * 2) as u32,
            },
        }],
    })
}

pub fn portable_expression_definition(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
    let encoded_program = program.canonical_hex().map_err(|refusal| match refusal {
        crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
        _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
    })?;
    let input = expression_port_kind(&program.input_type)?;
    let output = expression_port_kind(&program.output_type)?;
    let identity = hash_string(&format!(
        "pure-expression:{PURE_EXPRESSION_REVISION}:{}:{}:{}:{}",
        input.as_str(),
        output.as_str(),
        temporal.as_str(),
        encoded_program
    ));
    Ok(KindProjection {
        kind_id: kind_id(&format!("conduitese/pure-expression/{identity}")),
        kind_contract_revision: KindIdentity::from(PURE_EXPRESSION_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: input,
            direction: PortDirection::Input,
            temporal,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: output,
            direction: PortDirection::Output,
            temporal,
        }],
        configuration: vec![KindConfigurationField {
            key: "program".into(),
            default_value: ConfigurationValue::Text(encoded_program),
            rule: KindConfigurationRule::TextBytes {
                maximum: (MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES * 2) as u32,
            },
        }],
    })
}

fn expression_port_kind(
    value_type: &conduit_core::StructuredInfoType,
) -> Result<conduit_core::KindId, StructuredInfoRefusal> {
    match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Leaf(kind) => Ok(kind.clone()),
        _ => Ok(value_type.profile()?.value_kind().clone()),
    }
}
