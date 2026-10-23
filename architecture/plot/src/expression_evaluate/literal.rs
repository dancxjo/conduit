//! literal evaluation for exact checked expression values.
use super::*;

pub(super) fn literal_value(
    value_type: &StructuredInfoType,
    literal: &str,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if matches!(value_type.shape(), StructuredInfoTypeShape::Variant { .. }) {
        let unit = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::EMPTY_INFO_ID))
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
            Vec::new(),
        )
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
        return encoded_structured(
            StructuredInfoValue::variant(value_type.clone(), literal, unit)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
        );
    }
    let kind = leaf_kind(value_type)?;
    let encoded = match primitive_info_kind(kind) {
        Some(PrimitiveInfoKind::Empty) if literal == "empty" => Vec::new(),
        Some(PrimitiveInfoKind::Bool) => match literal {
            "true" => InfoBool::TRUE.encode().to_vec(),
            "false" => InfoBool::FALSE.encode().to_vec(),
            _ => return Err(PortableExpressionEvaluationRefusal::InvalidLiteral),
        },
        Some(PrimitiveInfoKind::Text) => crate::text_value::parse_quoted_text(literal)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidLiteral)?
            .into_bytes(),
        Some(PrimitiveInfoKind::F32) => crate::ieee_literal::f32_bits(literal)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?
            .to_vec(),
        Some(PrimitiveInfoKind::F64) => crate::ieee_literal::f64_bits(literal)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?
            .to_vec(),
        Some(PrimitiveInfoKind::Count) => encode_count(
            literal
                .parse()
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
        )
        .to_vec(),
        Some(PrimitiveInfoKind::Scalar) => crate::structured_startup::parse_scalar_literal(literal)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidLiteral)?
            .encode()
            .to_vec(),
        Some(PrimitiveInfoKind::MathematicalValue) => crate::mathematical_value::encode(literal)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
        Some(PrimitiveInfoKind::Unit) => unit_literal_bytes(literal)?,
        Some(PrimitiveInfoKind::Quantity) => quantity_literal_bytes(literal, None)?,
        Some(kind) if quantity_kind(kind) => {
            quantity_literal_bytes(literal, Some(leaf_kind(value_type)?))?
        }
        Some(kind) if fixed_integer(kind) => {
            let canonical = crate::integer_literal::canonicalize(literal, leaf_kind(value_type)?)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?
                .ok_or(PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            let integer = if signed_integer(kind) {
                FixedInteger::from_signed(
                    kind,
                    canonical
                        .parse()
                        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
                )
            } else {
                FixedInteger::from_unsigned(
                    kind,
                    canonical
                        .parse()
                        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?,
                )
            }
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            encode_integer(integer)
        }
        _ => {
            return Err(PortableExpressionEvaluationRefusal::UnsupportedType(
                kind.into(),
            ))
        }
    };
    primitive_value(value_type, encoded)
}

pub(crate) fn literal_primitive_bytes(
    value_type: &StructuredInfoType,
    literal: &str,
) -> Result<Vec<u8>, PortableExpressionEvaluationRefusal> {
    primitive_bytes(&literal_value(value_type, literal)?)
}

// Unit definitions are finite value carriers with substantial inline storage.
// Keep their parse/encoding scratch out of ordinary integer and Boolean literals.
#[inline(never)]
fn unit_literal_bytes(literal: &str) -> Result<Vec<u8>, PortableExpressionEvaluationRefusal> {
    Ok(conduit_core::Unit::resolve(literal)
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?
        .encode()
        .to_vec())
}

#[inline(never)]
fn quantity_literal_bytes(
    literal: &str,
    expected_kind: Option<&str>,
) -> Result<Vec<u8>, PortableExpressionEvaluationRefusal> {
    let quantity = Quantity::parse_plot_literal(literal)
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
    let encoded = quantity.encode();
    if let Some(kind) = expected_kind {
        conduit_core::validate_primitive_info(kind, &encoded)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
    }
    Ok(encoded.to_vec())
}
