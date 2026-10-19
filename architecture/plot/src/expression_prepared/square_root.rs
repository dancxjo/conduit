//! Prepares every builder and result buffer before an admitted Step.
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
    let target = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
        conduit_core::MATHEMATICAL_VALUE_ID,
    ))
    .map_err(|_| Refusal::InvalidProgram)?;
    if argument.value_type != target || node.value_type != target {
        return Err(Refusal::InvalidProgram);
    }
    let argument = Box::new(prepare_node(argument, input_type, prepared_input)?);
    let mut writer = Box::new(crate::expression_square_root::Writer::new()?);
    let mut output = alloc::vec![0; conduit_core::MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    if let PreparedOperation::Literal(encoded) = &argument.operation {
        let n = writer.encode(encoded, &mut output)?;
        output.truncate(n);
        return Ok(PreparedOperation::Literal(output));
    }
    Ok(PreparedOperation::SquareRoot {
        argument,
        writer,
        output,
    })
}
