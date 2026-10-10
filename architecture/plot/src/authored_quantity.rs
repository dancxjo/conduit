//! Checked physical values and separately retained authored spelling.
use crate::prelude::*;
use crate::{CanonicalStartupValue as V, SyntaxCheckError};
use conduit_core::*;

pub(crate) fn expected_role(ty: &StructuredInfoType) -> Option<&'static str> {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind)
            if (kind.as_str() == QUANTITY_INFO_ID
                || quantity_info_dimension(kind.as_str()).is_some()) =>
        {
            Some("quantity")
        }
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == UNIT_INFO_ID => Some("unit"),
        StructuredInfoTypeShape::Record { schema, .. }
            if schema.as_str() == EXACT_TEMPERATURE_DIFFERENCE_INFO_ID =>
        {
            Some("difference")
        }
        _ => None,
    }
}
pub(crate) fn parse(text: &str, role: &str) -> Result<V, SyntaxCheckError> {
    let invalid = |reason: String| {
        SyntaxCheckError::QuantityLiteral(format!(
            "'{text}' is not a checked {role} value: {reason}"
        ))
    };
    match role {
        "quantity" => QuantityConfigurationValue::parse(text)
            .map(V::Quantity)
            .map_err(|r| invalid(format!("{r:?}"))),
        "unit" => UnitConfigurationValue::parse(text)
            .map(V::Unit)
            .map_err(|r| invalid(format!("{r:?}"))),
        "difference" => ExactTemperatureDifferenceConfigurationValue::parse(text)
            .map(V::TemperatureDifference)
            .map_err(|r| invalid(format!("{r:?}"))),
        _ => Err(invalid("unknown role".into())),
    }
}
pub(crate) fn bytes(value: &V, expected: &StructuredInfoType) -> Option<Vec<u8>> {
    let role = expected_role(expected)?;
    match (role, value) {
        ("quantity", V::Quantity(value)) => {
            let bytes = value.value().encode().to_vec();
            let StructuredInfoTypeShape::Leaf(kind) = expected.shape() else {
                return None;
            };
            validate_primitive_info(kind.as_str(), &bytes).ok()?;
            Some(bytes)
        }
        ("unit", V::Unit(value)) => Some(value.value().encode().to_vec()),
        _ => None,
    }
}
pub(crate) fn value_kind(value: &V) -> Option<KindId> {
    match value {
        V::Quantity(_) => Some(kind_id(QUANTITY_INFO_ID)),
        V::Unit(_) => Some(kind_id(UNIT_INFO_ID)),
        V::TemperatureDifference(_) => {
            crate::quantity_conversion::temperature_difference::source_type()
                .profile()
                .ok()
                .map(|p| p.value_kind().clone())
        }
        _ => None,
    }
}
