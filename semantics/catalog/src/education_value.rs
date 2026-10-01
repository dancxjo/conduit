//! Shared bounded structured-value construction for education fixtures.

use alloc::vec::Vec;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
};

use super::education_realization::EducationInfoRefusal;

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

pub(super) fn leaf_count(value: &StructuredInfoValue) -> Result<u64, EducationInfoRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(EducationInfoRefusal::MalformedInfo);
    };
    conduit_core::decode_count(bytes).map_err(|_| EducationInfoRefusal::MalformedInfo)
}
