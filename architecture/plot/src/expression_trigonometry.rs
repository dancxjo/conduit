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

pub(crate) fn encode_profile(
    function: F,
    input: &[u8],
    output: &mut [u8],
    profile: conduit_core::QuantityValueProfile,
) -> Result<usize, R> {
    if profile != conduit_core::QuantityValueProfile::Mathematical {
        return encode(function, input, output);
    }
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    if input.first() == Some(&3) {
        let angle = conduit_core::MathematicalQuantityView::decode(input, &mut work)
            .map_err(|_| R::InvalidInput)?;
        conduit_core::encode_mathematical_angle_trigonometric_denotation(
            function, &angle, output, &mut work,
        )
        .map_err(R::AngleTrigonometry)
    } else {
        let angle = profile
            .decode(conduit_core::ANGLE_INFO_ID, input, &mut work)
            .map_err(|_| R::InvalidInput)?
            .rational(&mut work)
            .map_err(|_| R::InvalidInput)?;
        conduit_core::encode_rational_angle_trigonometric_denotation(
            function, angle, output, &mut work,
        )
        .map_err(R::AngleTrigonometry)
    }
}
