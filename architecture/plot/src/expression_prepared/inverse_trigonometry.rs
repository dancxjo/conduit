//! Admit inverse result storage once; exact runtime writers do not grow it.
use super::*;
use crate::expression_inverse_trigonometry::{quantity_dimension, Function};
pub(super) fn prepare(
    node: &PortableExpressionNode,
    call: &str,
    arguments: &[PortableExpressionNode],
    input_type: &conduit_core::StructuredInfoType,
    prepared_input: &PreparedInput,
) -> Result<PreparedOperation, Refusal> {
    let function =
        crate::expression_inverse_trigonometry::function(call).ok_or(Refusal::InvalidProgram)?;
    let required = if matches!(function, Function::Atan2) {
        2
    } else {
        1
    };
    let target = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
        conduit_core::MATHEMATICAL_ANGLE_ID,
    ))
    .map_err(|_| Refusal::InvalidProgram)?;
    if arguments.len() != required || node.value_type != target {
        return Err(Refusal::InvalidProgram);
    }
    for argument in arguments {
        let StructuredInfoTypeShape::Leaf(kind) = argument.value_type.shape() else {
            return Err(Refusal::InvalidProgram);
        };
        let valid = match function {
            Function::Unary(_) => kind.as_str() == conduit_core::MATHEMATICAL_VALUE_ID,
            Function::Atan2 => quantity_dimension(kind.as_str()).is_some(),
        };
        if !valid {
            return Err(Refusal::InvalidProgram);
        }
    }
    let left = Box::new(prepare_node(&arguments[0], input_type, prepared_input)?);
    let right = arguments
        .get(1)
        .map(|a| prepare_node(a, input_type, prepared_input).map(Box::new))
        .transpose()?;
    let mut output = alloc::vec![0;conduit_core::INVERSE_ANGLE_MAX_ENCODED_BYTES];
    if let PreparedOperation::Literal(a) = &left.operation {
        let literal = match &right {
            None => Some(None),
            Some(b) => match &b.operation {
                PreparedOperation::Literal(b) => Some(Some(b.as_slice())),
                _ => None,
            },
        };
        if let Some(b) = literal {
            let n = crate::expression_inverse_trigonometry::encode(function, a, b, &mut output)?;
            output.truncate(n);
            return Ok(PreparedOperation::Literal(output));
        }
    }
    Ok(PreparedOperation::InverseTrigonometry {
        function,
        left,
        right,
        output,
    })
}
