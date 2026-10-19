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

mod arithmetic;
pub use arithmetic::failure_detail as mathematical_arithmetic_failure_detail;
pub(crate) use arithmetic::{evaluate as arithmetic, PreparedArithmetic};

/// Angle coordinates use the same bounded proof relation; Unit is canonical turn.
pub(crate) fn angle_equal(
    left: &[u8],
    right: &[u8],
) -> Result<bool, crate::PortableExpressionEvaluationRefusal> {
    use crate::PortableExpressionEvaluationRefusal as R;
    use conduit_core::{MathematicalAngleView as View, MathematicalComparison as C};
    let mut work =
        conduit_core::ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let left = View::decode(left, &mut work).map_err(|_| R::InvalidInput)?;
    let right = View::decode(right, &mut work).map_err(|_| R::InvalidInput)?;
    match left
        .coordinate()
        .compare(right.coordinate(), &mut work)
        .map_err(|_| R::Arithmetic)?
    {
        C::ProvedEqual => Ok(true),
        C::ProvedDifferent => Ok(false),
        C::Unknown => Err(R::UnprovedMathematicalComparison),
    }
}
