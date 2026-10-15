//! Every trig result slot is admitted before Play; the writer never grows it.
use super::*;
pub(super) fn prepare(
    node: &PortableExpressionNode,
    call: &str,
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
    Ok(PreparedOperation::Trigonometry {
        function: crate::expression_trigonometry::function(call).ok_or(Refusal::InvalidProgram)?,
        argument: Box::new(prepare_node(argument, input_type, prepared_input)?),
        output: alloc::vec![0;crate::expression_trigonometry::MAXIMUM_ENCODED_BYTES],
    })
}
