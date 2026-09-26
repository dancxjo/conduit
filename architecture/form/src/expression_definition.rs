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

/// Projects a checked expression into the ordinary Kind/Fore vocabulary used
/// by expansion and planning. The source spelling and spans are absent from the
/// identity; exact checked input, result, temporal law and canonical operation
/// determine it.
pub fn pure_expression_definition(
    expression: &CheckedExpression,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
    let program = PortableExpressionProgram::from_checked(expression)
        .map_err(|refusal| match refusal {
            crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
            _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
        })?
        .canonical_hex()
        .map_err(|refusal| match refusal {
            crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
            _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
        })?;
    let input = expression.input_type.exact_value_kind()?;
    let output = expression.value_type.exact_value_kind()?;
    let identity = hash_string(&format!(
        "pure-expression:{PURE_EXPRESSION_REVISION}:{}:{}:{}:{}",
        input.as_str(),
        output.as_str(),
        temporal.as_str(),
        program
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
            default_value: ConfigurationValue::Text(program),
            rule: KindConfigurationRule::TextBytes {
                maximum: (MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES * 2) as u32,
            },
        }],
    })
}
