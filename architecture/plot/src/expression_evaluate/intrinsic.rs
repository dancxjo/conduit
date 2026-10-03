//! intrinsic evaluation for exact checked expression values.
use super::*;

pub(super) fn intrinsic_call(
    kind: &str,
    mut arguments: Vec<Value>,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if kind == "variant/tag" {
        let argument = arguments
            .pop()
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if !arguments.is_empty() {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let value = StructuredInfoValue::from_canonical_bytes(&argument.encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        return Ok(Value {
            value_type: expected.clone(),
            encoded: tag.as_bytes().to_vec(),
        });
    }
    if kind == "variant/is" {
        if arguments.len() != 2 {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let case = arguments
            .pop()
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let value = arguments
            .pop()
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let value = StructuredInfoValue::from_canonical_bytes(&value.encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        let matched = tag.as_bytes() == case.encoded;
        return Ok(Value {
            value_type: expected.clone(),
            encoded: vec![u8::from(matched)],
        });
    }
    if matches!(
        kind,
        "value/u16"
            | "value/u32"
            | "value/u64"
            | "value/u128"
            | "value/i16"
            | "value/i32"
            | "value/i64"
            | "value/i128"
    ) {
        let argument = arguments
            .pop()
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if !arguments.is_empty() {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let source = decode_integer(&argument)?;
        let target = primitive_info_kind(leaf_kind(expected)?)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let widened = if signed_integer(source.kind()) {
            FixedInteger::from_signed(
                target,
                source
                    .signed()
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            )
        } else if signed_integer(target) {
            FixedInteger::from_signed(
                target,
                i128::try_from(
                    source
                        .unsigned()
                        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
                )
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            )
        } else {
            FixedInteger::from_unsigned(
                target,
                source
                    .unsigned()
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
            )
        }
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        return Ok(Value {
            value_type: expected.clone(),
            encoded: encode_integer(widened),
        });
    }
    Err(PortableExpressionEvaluationRefusal::UnsupportedSemanticCall(kind.into()))
}
