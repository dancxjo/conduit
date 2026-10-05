//! Exact ordinary Gear contract for one checked pure expression.

use crate::{
    hash_string, CheckedExpression, KindConfigurationField, KindConfigurationRule, KindProjection,
    PortableExpressionProgram, MAXIMUM_PURE_EXPRESSION_PROGRAM_BYTES,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, ExternalEffectBehavior, KindIdentity, KindSemanticLaw,
    PortDescriptor, PortDirection, PortTemporal, ReplayBehavior, SemanticDependence,
    StructuredInfoRefusal, SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
};

pub const PURE_EXPRESSION_REVISION: &str = "conduitese/pure-expression-operation@1";
pub const PURE_FILTER_REVISION: &str = "conduitese/pure-filter-operation@1";

pub fn pure_expression_semantic_laws() -> alloc::vec::Vec<KindSemanticLaw> {
    vec![
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
        KindSemanticLaw::TimeDependence(SemanticDependence::None),
        KindSemanticLaw::RandomDependence(SemanticDependence::None),
        KindSemanticLaw::ResourceDependence(SemanticDependence::None),
        KindSemanticLaw::Suspension(SuspensionBehavior::Never),
        KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
        KindSemanticLaw::Replay(ReplayBehavior::Exact),
    ]
}

/// Projects a checked expression into the ordinary Kind/Fore vocabulary used
/// by expansion and planning. The source spelling and spans are absent from the
/// identity; exact checked input, result, temporal law and canonical operation
/// determine it.
pub fn pure_expression_definition(
    expression: &CheckedExpression,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
    let encoded =
        crate::expression_program::checked_canonical_hex(expression).map_err(|refusal| {
            match refusal {
                crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
                _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
            }
        })?;
    let input = expression
        .input_type
        .structured_info_type_with(&expression.semantic_structures)?;
    let output = expression
        .value_type
        .structured_info_type_with(&expression.semantic_structures)?;
    expression_definition_from_encoding(
        expression_port_kind(&input)?,
        expression_port_kind(&output)?,
        encoded,
        temporal,
    )
}

/// Projects a checked Boolean predicate into canonical unary filtering. Flows
/// drop rejected items; one values emit the exact finite optional variant.
pub fn pure_filter_definition(
    expression: &CheckedExpression,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
    if expression.value_type != crate::CheckedExpressionType::semantic(conduit_core::BOOL_INFO_ID) {
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
        || temporal == PortTemporal::Current
    {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    let encoded_program = program.canonical_hex().map_err(|refusal| match refusal {
        crate::PortableExpressionProgramRefusal::InvalidType(refusal) => refusal,
        _ => StructuredInfoRefusal::MalformedCanonicalEncoding,
    })?;
    let value = expression_port_kind(&program.input_type)?;
    let output = if temporal == PortTemporal::Value {
        conduit_core::optional_info_type(program.input_type.clone())?
            .profile()?
            .value_kind()
            .clone()
    } else {
        value.clone()
    };
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
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: output,
            direction: PortDirection::Output,
            temporal,
            abnormal_kind: None,
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
    expression_definition_from_encoding(input, output, encoded_program, temporal)
}

fn expression_definition_from_encoding(
    input: conduit_core::KindId,
    output: conduit_core::KindId,
    encoded_program: alloc::string::String,
    temporal: PortTemporal,
) -> Result<KindProjection, StructuredInfoRefusal> {
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
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: output,
            direction: PortDirection::Output,
            temporal,
            abnormal_kind: None,
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
