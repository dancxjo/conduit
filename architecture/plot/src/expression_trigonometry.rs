//! Reviewed typed Angle intrinsics; mathematical denotation precedes projection.
use crate::PortableExpressionEvaluationRefusal as R;
use conduit_core::{AngleTrigonometricFunction as F, ExactNumericWork, Quantity};
pub(crate) const MAXIMUM_ENCODED_BYTES: usize = 8 + conduit_core::EXACT_RATIONAL_MAX_ENCODED_BYTES;
pub(crate) fn function(name: &str) -> Option<F> {
    match name {
        "math/sin" => Some(F::Sine),
        "math/cos" => Some(F::Cosine),
        "math/tan" => Some(F::Tangent),
        _ => None,
    }
}
pub(crate) fn encode(function: F, input: &[u8], output: &mut [u8]) -> Result<usize, R> {
    let quantity = Quantity::decode(input).map_err(|_| R::InvalidInput)?;
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    conduit_core::encode_angle_trigonometric_denotation(function, quantity, output, &mut work)
        .map_err(R::AngleTrigonometry)
}
