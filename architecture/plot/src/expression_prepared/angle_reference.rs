//! Preparation owns the exact symbolic result buffer and rejects known domains.
use super::*;
pub(super) fn prepare(
    node: &PortableExpressionNode,
    arguments: &[PortableExpressionNode],
    input_type: &conduit_core::StructuredInfoType,
    prepared_input: &PreparedInput,
) -> Result<PreparedOperation, Refusal> {
    let [argument] = arguments else {
        return Err(Refusal::InvalidProgram);
    };
    let angle =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::ANGLE_INFO_ID))
            .map_err(|_| Refusal::InvalidProgram)?;
    let result = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
        conduit_core::MATHEMATICAL_VALUE_ID,
    ))
    .map_err(|_| Refusal::InvalidProgram)?;
    if argument.value_type != angle || node.value_type != result {
        return Err(Refusal::InvalidProgram);
    }
    let argument = Box::new(prepare_node(argument, input_type, prepared_input)?);
    let mut output = alloc::vec![0;crate::expression_angle_reference::MAXIMUM_ENCODED_BYTES];
    if let PreparedOperation::Literal(encoded) = &argument.operation {
        let length = crate::expression_angle_reference::encode(encoded, &mut output)?;
        output.truncate(length);
        return Ok(PreparedOperation::Literal(output));
    }
    Ok(PreparedOperation::AngleReference { argument, output })
}
