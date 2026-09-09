//! Private structural helpers for the navigation canonical codec.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::{
    kind_id, Quantity, QuantityUnit, StructuredFieldValue, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};

use crate::NavigationRefusal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationCodecError {
    Malformed,
    InexactQuantity,
    Structured(StructuredInfoRefusal),
    Navigation(NavigationRefusal),
    Robotics,
}

impl From<StructuredInfoRefusal> for NavigationCodecError {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

impl From<NavigationRefusal> for NavigationCodecError {
    fn from(value: NavigationRefusal) -> Self {
        Self::Navigation(value)
    }
}

pub(crate) fn exact(
    encoded: &[u8],
    expected: &StructuredInfoType,
) -> Result<StructuredInfoValue, NavigationCodecError> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)?;
    (value.value_type() == expected)
        .then_some(value)
        .ok_or(NavigationCodecError::Malformed)
}

pub(crate) fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, NavigationCodecError> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(NavigationCodecError::Malformed)
}

pub(crate) fn text(value: &StructuredInfoValue) -> Result<String, NavigationCodecError> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    core::str::from_utf8(bytes)
        .map(ToString::to_string)
        .map_err(|_| NavigationCodecError::Malformed)
}

pub(crate) fn count(value: &StructuredInfoValue) -> Result<u64, NavigationCodecError> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    core::str::from_utf8(bytes)
        .map_err(|_| NavigationCodecError::Malformed)?
        .parse()
        .map_err(|_| NavigationCodecError::Malformed)
}

pub(crate) fn quantity(
    value: &StructuredInfoValue,
    unit: QuantityUnit,
) -> Result<i64, NavigationCodecError> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    Quantity::decode(bytes)
        .map_err(|_| NavigationCodecError::Malformed)?
        .convert(unit)
        .map(|value| value.value())
        .map_err(|_| NavigationCodecError::InexactQuantity)
}

pub(crate) fn u64_quantity(
    value: &StructuredInfoValue,
    unit: QuantityUnit,
) -> Result<u64, NavigationCodecError> {
    quantity(value, unit)?
        .try_into()
        .map_err(|_| NavigationCodecError::InexactQuantity)
}
pub(crate) fn u32_quantity(
    value: &StructuredInfoValue,
    unit: QuantityUnit,
) -> Result<u32, NavigationCodecError> {
    quantity(value, unit)?
        .try_into()
        .map_err(|_| NavigationCodecError::InexactQuantity)
}
pub(crate) fn i32_quantity(
    value: &StructuredInfoValue,
    unit: QuantityUnit,
) -> Result<i32, NavigationCodecError> {
    quantity(value, unit)?
        .try_into()
        .map_err(|_| NavigationCodecError::InexactQuantity)
}

pub(crate) fn validity(value: &StructuredInfoValue) -> Result<(String, u64), NavigationCodecError> {
    Ok((
        text(record_field(value, "clock_identity")?)?,
        u64_quantity(
            record_field(value, "valid_until")?,
            QuantityUnit::Millisecond,
        )?,
    ))
}

pub(crate) fn text_value(value: &str) -> Result<StructuredInfoValue, NavigationCodecError> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text@1"))?,
        value.as_bytes().to_vec(),
    )?)
}

pub(crate) fn quantity_value(
    value: i64,
    unit: QuantityUnit,
) -> Result<StructuredInfoValue, NavigationCodecError> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(conduit_core::QUANTITY_INFO_ID))?,
        Quantity::new(value, unit).encode().to_vec(),
    )?)
}

pub(crate) fn record_value(
    ty: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, NavigationCodecError> {
    Ok(StructuredInfoValue::record(
        ty,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<_, _>>()?,
    )?)
}

pub(crate) fn field_type(
    ty: &StructuredInfoType,
    name: &str,
) -> Result<StructuredInfoType, NavigationCodecError> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value_type().clone())
        .ok_or(NavigationCodecError::Malformed)
}

pub(crate) fn variant_type(
    ty: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, NavigationCodecError> {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or(NavigationCodecError::Malformed)
}

pub(crate) fn count_tag(count: usize) -> Result<&'static str, NavigationCodecError> {
    match count {
        1 => Ok("one"),
        2 => Ok("two"),
        3 => Ok("three"),
        4 => Ok("four"),
        _ => Err(NavigationCodecError::Malformed),
    }
}
