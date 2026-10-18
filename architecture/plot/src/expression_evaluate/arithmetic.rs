//! Reference evaluation of checked binary expressions.
use super::*;

pub(super) fn binary(
    operator: BinaryOperator,
    proven: bool,
    left: Value,
    right: Value,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if let (Ok(left_kind), Ok(right_kind)) =
        (leaf_kind(&left.value_type), leaf_kind(&right.value_type))
    {
        if let Some(result) = crate::expression_quantity::evaluate(
            operator,
            left_kind,
            right_kind,
            &primitive_bytes(&left)?,
            &primitive_bytes(&right)?,
        ) {
            return primitive_value(expected, result?.to_vec());
        }
    }
    if left.value_type != right.value_type {
        return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
    }
    if matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual) {
        let equal = equality::equal(left, right)?;
        return primitive_value(
            expected,
            InfoBool::new(if matches!(operator, BinaryOperator::Equal) {
                equal
            } else {
                !equal
            })
            .encode()
            .to_vec(),
        );
    }
    let comparison = matches!(
        operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
            | BinaryOperator::NotEqual
    );
    let encoded = if matches!(
        operator,
        BinaryOperator::BooleanAnd | BinaryOperator::BooleanOr
    ) {
        let left = decode_bool(&left)?;
        let right = decode_bool(&right)?;
        InfoBool::new(match operator {
            BinaryOperator::BooleanAnd => left && right,
            BinaryOperator::BooleanOr => left || right,
            _ => unreachable!(),
        })
        .encode()
        .to_vec()
    } else if comparison {
        let ordering = compare(&left, &right)?;
        InfoBool::new(match operator {
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => !ordering.is_gt(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => !ordering.is_lt(),
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => !ordering.is_eq(),
            _ => unreachable!(),
        })
        .encode()
        .to_vec()
    } else if leaf_kind(&left.value_type)? == conduit_core::MATHEMATICAL_VALUE_ID {
        let mut output = vec![0; conduit_core::EXACT_RATIONAL_MAX_ENCODED_BYTES + 3];
        let length = crate::mathematical_value::arithmetic(
            operator,
            &primitive_bytes(&left)?,
            &primitive_bytes(&right)?,
            &mut output,
        )?;
        output.truncate(length);
        output
    } else if leaf_kind(&left.value_type)? == conduit_core::F32_INFO_ID {
        crate::expression_float::binary32::arithmetic(
            operator,
            &primitive_bytes(&left)?,
            &primitive_bytes(&right)?,
        )
        .ok_or(PortableExpressionEvaluationRefusal::Arithmetic)?
        .to_vec()
    } else if leaf_kind(&left.value_type)? == conduit_core::F64_INFO_ID {
        crate::expression_float::binary64::arithmetic(
            operator,
            &primitive_bytes(&left)?,
            &primitive_bytes(&right)?,
        )
        .ok_or(PortableExpressionEvaluationRefusal::Arithmetic)?
        .to_vec()
    } else if leaf_kind(&left.value_type)? == COUNT_INFO_ID {
        let left = decode_count(&primitive_bytes(&left)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let right = decode_count(&primitive_bytes(&right)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let result = match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide if right != 0 => left.checked_div(right),
            BinaryOperator::Remainder if right != 0 => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => None,
        }
        .ok_or(PortableExpressionEvaluationRefusal::Arithmetic)?;
        encode_count(result).to_vec()
    } else if leaf_kind(&left.value_type)? == SCALAR_INFO_ID {
        let left = Scalar::decode(&primitive_bytes(&left)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let right = Scalar::decode(&primitive_bytes(&right)?)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide => left.checked_div(right),
            BinaryOperator::Remainder => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => return Err(PortableExpressionEvaluationRefusal::InvalidProgram),
        }
        .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?
        .encode()
        .to_vec()
    } else {
        let left = decode_integer(&left)?;
        let right_integer = decode_integer(&right)?;
        let result = match (operator, proven) {
            (BinaryOperator::Multiply, true) => left.wrapping_mul(right_integer),
            (BinaryOperator::Add, true) => left.wrapping_add(right_integer),
            (BinaryOperator::Subtract, true) => left.wrapping_sub(right_integer),
            (BinaryOperator::Multiply, false) => left.checked_mul(right_integer),
            (BinaryOperator::Divide, _) => left.checked_div(right_integer),
            (BinaryOperator::Remainder, _) => left.checked_rem(right_integer),
            (BinaryOperator::Add, false) => left.checked_add(right_integer),
            (BinaryOperator::Subtract, false) => left.checked_sub(right_integer),
            (BinaryOperator::BitAnd, _) => left.bit_and(right_integer),
            (BinaryOperator::BitXor, _) => left.bit_xor(right_integer),
            (BinaryOperator::BitOr, _) => left.bit_or(right_integer),
            (BinaryOperator::ShiftLeft, _) => left.checked_shift_left(shift_count(right_integer)?),
            (BinaryOperator::ShiftRight, _) => {
                left.checked_shift_right(shift_count(right_integer)?)
            }
            _ => return Err(PortableExpressionEvaluationRefusal::InvalidProgram),
        }
        .map_err(|_| PortableExpressionEvaluationRefusal::Arithmetic)?;
        encode_integer(result)
    };
    primitive_value(expected, encoded)
}
