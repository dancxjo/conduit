//! Exact turn-origin Angle to symbolic radian coordinate; never a float cast.
use crate::PortableExpressionEvaluationRefusal as R;
use conduit_core::{ExactNumericWork, Quantity, StructuredInfoType, Unit};
pub(crate) const CALL: &str = "math/radians";
pub(crate) const MAXIMUM_ENCODED_BYTES: usize = 10 + conduit_core::EXACT_RATIONAL_MAX_ENCODED_BYTES;
pub(crate) fn encode(input: &[u8], output: &mut [u8]) -> Result<usize, R> {
    let source = Quantity::decode(input).map_err(|_| R::InvalidInput)?;
    conduit_core::encode_angle_radian_denotation(
        source,
        Unit::Radian,
        output,
        &mut ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK),
    )
    .map_err(R::AngleReference)
}
pub(crate) fn evaluate(
    argument: &crate::expression_evaluate::Value,
    expected: &StructuredInfoType,
) -> Result<crate::expression_evaluate::Value, R> {
    let angle = StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::ANGLE_INFO_ID))
        .map_err(|_| R::InvalidProgram)?;
    let target =
        StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::MATHEMATICAL_VALUE_ID))
            .map_err(|_| R::InvalidProgram)?;
    if argument.value_type != angle || expected != &target {
        return Err(R::InvalidProgram);
    }
    let mut encoded = alloc::vec![0; MAXIMUM_ENCODED_BYTES];
    let length = encode(&argument.encoded, &mut encoded)?;
    encoded.truncate(length);
    Ok(crate::expression_evaluate::Value {
        value_type: target,
        encoded,
    })
}
