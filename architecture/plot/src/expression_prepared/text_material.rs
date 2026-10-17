//! Preparation for explicit text material observation, preserving exact UTF-8.
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
    let target =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::TEXT_INFO_ID))
            .map_err(|_| Refusal::InvalidProgram)?;
    if node.value_type != target {
        return Err(Refusal::InvalidProgram);
    }
    let operand = prepare_node(argument, input_type, prepared_input)?;
    if operand.kind != PrimitiveInfoKind::Text {
        return Err(Refusal::InvalidProgram);
    }
    Ok(PreparedOperation::TextMaterial(Box::new(operand)))
}
