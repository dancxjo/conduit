//! literal evaluation for exact checked expression values.
use super::*;

pub(super) fn literal_value(
    value_type: &StructuredInfoType,
    literal: &str,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    if matches!(value_type.shape(), StructuredInfoTypeShape::Variant { .. }) {
        let unit = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::UNIT_INFO_ID))
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
        Some(PrimitiveInfoKind::Bool) => match literal {
            "true" => InfoBool::TRUE.encode().to_vec(),
            "false" => InfoBool::FALSE.encode().to_vec(),
            _ => return Err(PortableExpressionEvaluationRefusal::InvalidLiteral),
        },
        Some(PrimitiveInfoKind::Text) => crate::text_value::parse_quoted_text(literal)
            .ok_or(PortableExpressionEvaluationRefusal::InvalidLiteral)?
            .into_bytes(),
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
        Some(kind) if quantity_kind(kind) => {
            let quantity = Quantity::parse_plot_literal(literal)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            conduit_core::validate_primitive_info(leaf_kind(value_type)?, &quantity.encode())
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidLiteral)?;
            quantity.encode().to_vec()
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
