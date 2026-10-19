//! Preparation owns the exact symbolic result buffer and rejects known domains.
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
    let physical = call == crate::expression_angle_reference::ANGLE_CALL;
    if physical
        && prepared_input.quantity_profile() != conduit_core::QuantityValueProfile::Mathematical
    {
        return Err(Refusal::UnsupportedType(
            "preserved Angle requires explicit mathematical profile".into(),
        ));
    }
    let result = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(if physical {
        conduit_core::ANGLE_INFO_ID
    } else {
        conduit_core::MATHEMATICAL_VALUE_ID
    }))
    .map_err(|_| Refusal::InvalidProgram)?;
    if argument.value_type != angle || node.value_type != result {
        return Err(Refusal::InvalidProgram);
    }
    let argument = Box::new(prepare_node(argument, input_type, prepared_input)?);
    if physical {
        let mut coordinate =
            alloc::vec![0;crate::expression_angle_reference::MAXIMUM_ENCODED_BYTES];
        let mut output = alloc::vec![0;conduit_core::MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
        if let PreparedOperation::Literal(encoded) = &argument.operation {
            let n = crate::expression_angle_reference::encode_angle(
                encoded,
                &mut coordinate,
                &mut output,
            )?;
            output.truncate(n);
            return Ok(PreparedOperation::Literal(output));
        }
        return Ok(PreparedOperation::AngleReference {
            coordinate: Some(coordinate),
            argument,
            output,
        });
    }
    let mut output = alloc::vec![0;crate::expression_angle_reference::MAXIMUM_ENCODED_BYTES];
    if let PreparedOperation::Literal(encoded) = &argument.operation {
        let length = crate::expression_angle_reference::encode(encoded, &mut output)?;
        output.truncate(length);
        return Ok(PreparedOperation::Literal(output));
    }
    Ok(PreparedOperation::AngleReference {
        coordinate: None,
        argument,
        output,
    })
}
