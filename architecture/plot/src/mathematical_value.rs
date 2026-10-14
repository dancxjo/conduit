//! Closed exact mathematical literals at checked preparation boundaries.
use crate::prelude::*;
pub(crate) fn encode(literal: &str) -> Result<Vec<u8>, String> {
    let mut work =
        conduit_core::ExactNumericWork::new(conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let value = conduit_core::MathematicalValue::parse_literal(literal, &mut work)
        .map_err(|reason| format!("exact mathematical literal refused: {reason:?}"))?;
    let mut bytes = vec![0; conduit_core::MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = value
        .encode(&mut bytes)
        .map_err(|reason| format!("exact mathematical encoding refused: {reason:?}"))?;
    bytes.truncate(length);
    conduit_core::validate_primitive_info(conduit_core::MATHEMATICAL_VALUE_ID, &bytes)
        .map_err(|reason| format!("exact mathematical transport refused: {reason:?}"))?;
    Ok(bytes)
}

/// Comparison is a bounded proof attempt. Unknown never becomes Boolean false.
pub(crate) fn equal(
    left: &[u8],
    right: &[u8],
) -> Result<bool, crate::PortableExpressionEvaluationRefusal> {
    use crate::PortableExpressionEvaluationRefusal as R;
    use conduit_core::{MathematicalComparison as C, MathematicalValueView as View};
    let mut work =
        conduit_core::ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let left = View::decode(left, &mut work).map_err(|_| R::InvalidInput)?;
    let right = View::decode(right, &mut work).map_err(|_| R::InvalidInput)?;
    match left.compare(&right, &mut work).map_err(|_| R::Arithmetic)? {
        C::ProvedEqual => Ok(true),
        C::ProvedDifferent => Ok(false),
        C::Unknown => Err(R::UnprovedMathematicalComparison),
    }
}

/// Rational arithmetic spends bounded limb work and writes into admitted storage.
pub(crate) fn arithmetic(
    operator: crate::BinaryOperator,
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
) -> Result<usize, crate::PortableExpressionEvaluationRefusal> {
    use crate::{BinaryOperator as O, PortableExpressionEvaluationRefusal as R};
    use conduit_core::{ExactNumericWork, MathematicalValue, MathematicalValueView};
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let mut rational = |bytes| {
        MathematicalValueView::decode(bytes, &mut work)
            .and_then(|view| view.as_rational(&mut work))
            .map_err(|_| R::Arithmetic)?
            .ok_or(R::Arithmetic)
    };
    let left = rational(left)?;
    let right = rational(right)?;
    let result = match operator {
        O::Add => left.checked_add(right, &mut work),
        O::Subtract => left.checked_subtract(right, &mut work),
        O::Multiply => left.checked_multiply(right, &mut work),
        O::Divide => left.checked_divide(right, &mut work),
        _ => return Err(R::InvalidProgram),
    }
    .map_err(|_| R::Arithmetic)?;
    MathematicalValue::rational(result)
        .encode(output)
        .map_err(|_| R::Arithmetic)
}
