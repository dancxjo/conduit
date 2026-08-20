//! Shared bounded structured-value construction for education fixtures.

use alloc::{string::ToString, vec::Vec};
use conduit_core::{
    Quantity, QuantityUnit, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};

use super::education_realization::EducationInfoRefusal;

pub(super) fn ratio_value(value: i64) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    leaf_value(
        conduit_core::QUANTITY_INFO_ID,
        Quantity::new(value, QuantityUnit::Millionth)
            .encode()
            .to_vec(),
    )
}

pub(super) fn unit_value() -> Result<StructuredInfoValue, EducationInfoRefusal> {
    leaf_value("value/unit@1", Vec::new())
}

pub(super) fn text_value(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/text@1")).unwrap(),
        value.as_bytes().to_vec(),
    )
    .expect("bounded deterministic education text")
}

pub(super) fn count_value(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/count@1")).unwrap(),
        value.to_string().into_bytes(),
    )
    .expect("bounded deterministic education count")
}

fn leaf_value(kind: &str, bytes: Vec<u8>) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind))?,
        bytes,
    )?)
}

pub(super) fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

pub(super) fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, EducationInfoRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(EducationInfoRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(EducationInfoRefusal::MalformedInfo)
}

pub(super) fn leaf_text(value: &StructuredInfoValue) -> Result<&str, EducationInfoRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(EducationInfoRefusal::MalformedInfo);
    };
    core::str::from_utf8(bytes).map_err(|_| EducationInfoRefusal::MalformedInfo)
}

pub(super) fn variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, EducationInfoRefusal> {
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(EducationInfoRefusal::MalformedInfo);
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or(EducationInfoRefusal::MalformedInfo)
}
