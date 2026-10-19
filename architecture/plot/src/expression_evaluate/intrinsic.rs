//! intrinsic evaluation for exact checked expression values.
use super::*;

pub(super) fn intrinsic_call(
    kind: &str,
    mut arguments: Vec<Value>,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if kind == crate::expression_square_root::CALL {
        return crate::expression_square_root::evaluate(&arguments, expected);
    }
    if let Some(function) = crate::expression_inverse_trigonometry::function(kind) {
        return crate::expression_inverse_trigonometry::evaluate(function, &arguments, expected);
    }
    if kind == crate::expression_angle_reference::CALL {
        let [argument] = arguments.as_slice() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        return crate::expression_angle_reference::evaluate(argument, expected);
    }
    if let Some(function) = crate::expression_trigonometry::function(kind) {
        let [argument] = arguments.as_slice() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        let angle = StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::ANGLE_INFO_ID))
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let target =
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::MATHEMATICAL_VALUE_ID))
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if argument.value_type != angle || expected != &target {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let mut encoded = alloc::vec![0;crate::expression_trigonometry::MAXIMUM_ENCODED_BYTES];
        let length =
            crate::expression_trigonometry::encode(function, &argument.encoded, &mut encoded)?;
        encoded.truncate(length);
        return Ok(Value {
            value_type: target,
            encoded,
        });
    }
    if kind == "text/material" {
        let [argument] = arguments.as_slice() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        let target = StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::TEXT_INFO_ID))
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if expected != &target || leaf_kind(&argument.value_type)? != conduit_core::TEXT_INFO_ID {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let encoded = primitive_bytes(argument)?;
        conduit_core::validate_primitive_info(conduit_core::TEXT_INFO_ID, &encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?;
        return Ok(Value {
            value_type: target,
            encoded,
        });
    }
    if kind == "bytes/window" {
        let source_type = StructuredInfoType::leaf(conduit_core::kind_id("value/bytes"))
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let integer_type = StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if arguments.len() != 3
            || expected != &source_type
            || arguments[0].value_type != source_type
            || arguments[1].value_type != integer_type
            || arguments[2].value_type != integer_type
        {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let integer = |value: &Value| -> Result<u64, PortableExpressionEvaluationRefusal> {
            Ok(u64::from_le_bytes(
                value
                    .encoded
                    .as_slice()
                    .try_into()
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?,
            ))
        };
        let offset = integer(&arguments[1])?;
        let length = integer(&arguments[2])?;
        let start = usize::try_from(offset)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?;
        let end = usize::try_from(
            offset
                .checked_add(length)
                .ok_or(PortableExpressionEvaluationRefusal::InvalidInput)?,
        )
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?;
        let encoded = arguments[0]
            .encoded
            .get(start..end)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidInput)?
            .to_vec();
        return Ok(Value {
            value_type: source_type,
            encoded,
        });
    }
    if matches!(kind, "bytes/length" | "bytes/at") {
        let count = if kind == "bytes/at" { 2 } else { 1 };
        let source_type = StructuredInfoType::leaf(conduit_core::kind_id("value/bytes"))
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        let expected_type =
            StructuredInfoType::leaf(conduit_core::kind_id(if kind == "bytes/at" {
                "value/u8"
            } else {
                "value/u64"
            }))
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if arguments.len() != count
            || arguments[0].value_type != source_type
            || expected != &expected_type
        {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let bytes = &arguments[0].encoded;
        let encoded = if kind == "bytes/at" {
            if arguments[1].value_type
                != StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?
            {
                return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
            }
            let index = u64::from_le_bytes(
                arguments[1]
                    .encoded
                    .as_slice()
                    .try_into()
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?,
            );
            let index = usize::try_from(index)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?;
            core::slice::from_ref(
                bytes
                    .get(index)
                    .ok_or(PortableExpressionEvaluationRefusal::InvalidInput)?,
            )
            .to_vec()
        } else {
            (bytes.len() as u64).to_le_bytes().to_vec()
        };
        return Ok(Value {
            value_type: expected_type,
            encoded,
        });
    }
    if kind == "sequence/at" {
        let [source, index] = arguments.as_slice() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        let element = crate::expression_semantic_call::collection_element(&source.value_type)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
        if element != expected
            || index.value_type
                != StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?
        {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        }
        let index = u64::from_le_bytes(
            index
                .encoded
                .as_slice()
                .try_into()
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?,
        );
        let index = usize::try_from(index)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?;
        let source = StructuredInfoValue::from_canonical_bytes(&source.encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidInput)?;
        let StructuredInfoValueShape::Collection(values) = source.shape() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        let selected = values
            .get(index)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidInput)?;
        return super::structured::encoded_structured(selected.clone());
    }
    if kind == "sequence/length" {
        let [argument] = arguments.as_slice() else {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
        };
        let length = conduit_core::validate_canonical_structured_value(&argument.encoded)
            .and_then(|value| value.collection_length())
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
        return Ok(Value {
            value_type: expected.clone(),
            encoded: u64::from(length).to_le_bytes().to_vec(),
        });
    }
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
