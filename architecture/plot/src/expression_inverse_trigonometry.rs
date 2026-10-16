//! Reviewed exact principal inverse intrinsics, independent of approximation.
use crate::PortableExpressionEvaluationRefusal as R;
use conduit_core::{ExactNumericWork, InverseAngleFunction as F};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Function {
    Unary(F),
    Atan2,
}
pub(crate) fn function(name: &str) -> Option<Function> {
    Some(match name {
        "math/atan" => Function::Unary(F::Arctangent),
        "math/asin" => Function::Unary(F::Arcsine),
        "math/acos" => Function::Unary(F::Arccosine),
        "math/atan2" => Function::Atan2,
        _ => return None,
    })
}
pub(crate) fn quantity_dimension(kind: &str) -> Option<Option<conduit_core::QuantityDimension>> {
    if kind == conduit_core::QUANTITY_INFO_ID {
        Some(None)
    } else {
        conduit_core::quantity_info_dimension(kind).map(Some)
    }
}
pub(crate) fn encode(
    function: Function,
    left: &[u8],
    right: Option<&[u8]>,
    output: &mut [u8],
) -> Result<usize, R> {
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    match function {
        Function::Unary(function) => {
            if right.is_some() {
                return Err(R::InvalidProgram);
            }
            let value = conduit_core::MathematicalValueView::decode(left, &mut work)
                .map_err(|_| R::InvalidInput)?;
            let input = value
                .as_rational(&mut work)
                .map_err(|_| R::InvalidInput)?
                .ok_or(R::InverseAngle(
                    conduit_core::InverseAngleEncodingRefusal::Denotation(
                        conduit_core::MathematicalValueRefusal::UnsupportedDomainProof,
                    ),
                ))?;
            conduit_core::encode_inverse_angle_denotation(function, input, output, &mut work)
                .map_err(R::InverseAngle)
        }
        Function::Atan2 => {
            let y = conduit_core::Quantity::decode(left).map_err(|_| R::InvalidInput)?;
            let x = conduit_core::Quantity::decode(right.ok_or(R::InvalidProgram)?)
                .map_err(|_| R::InvalidInput)?;
            conduit_core::encode_quantity_atan2_denotation(y, x, output, &mut work)
                .map_err(R::InverseAngle)
        }
    }
}
pub(crate) fn evaluate(
    function: Function,
    arguments: &[crate::expression_evaluate::Value],
    expected: &conduit_core::StructuredInfoType,
) -> Result<crate::expression_evaluate::Value, R> {
    use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
    let target =
        StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::MATHEMATICAL_ANGLE_ID))
            .map_err(|_| R::InvalidProgram)?;
    if expected != &target {
        return Err(R::InvalidProgram);
    }
    let valid = |argument: &crate::expression_evaluate::Value| match argument.value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => match function {
            Function::Unary(_) => kind.as_str() == conduit_core::MATHEMATICAL_VALUE_ID,
            Function::Atan2 => quantity_dimension(kind.as_str()).is_some(),
        },
        _ => false,
    };
    let required = if matches!(function, Function::Atan2) {
        2
    } else {
        1
    };
    if arguments.len() != required || !arguments.iter().all(valid) {
        return Err(R::InvalidProgram);
    }
    let mut output = alloc::vec![0;conduit_core::INVERSE_ANGLE_MAX_ENCODED_BYTES];
    let n = encode(
        function,
        &arguments[0].encoded,
        arguments.get(1).map(|a| a.encoded.as_slice()),
        &mut output,
    )?;
    output.truncate(n);
    Ok(crate::expression_evaluate::Value {
        value_type: target,
        encoded: output,
    })
}

/// Zero-pair undefinedness is independent of any possible Unit assignment.
/// This proves only closed literal zero; it does not infer Units for scalars.
pub(crate) fn closed_zero(value: &crate::ExpressionSyntax) -> bool {
    match value {
        crate::ExpressionSyntax::Atomic(value) => conduit_core::MathematicalValue::parse_literal(
            &value.text,
            &mut ExactNumericWork::new(conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK),
        )
        .ok()
        .and_then(|v| v.as_rational())
        .is_some_and(|q| q == conduit_core::ExactRational::ZERO),
        crate::ExpressionSyntax::Unary {
            operator: crate::UnaryOperator::Negate,
            operand,
            ..
        } => closed_zero(operand),
        _ => false,
    }
}
