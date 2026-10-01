//! Shared bounded structured-value construction for education fixtures.

use alloc::vec::Vec;
use conduit_core::{StructuredFieldValue, StructuredInfoType, StructuredInfoValue};

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
