use alloc::vec::Vec;
use conduit_core::{
    Quantity, QuantityDimension, QuantityUnit, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue,
};

use crate::{RoboticsStructuredRefusal, MAXIMUM_ROBOTICS_IDENTITY_BYTES};

pub(super) fn require_uncertainty(
    value: Quantity,
    dimension: QuantityDimension,
    canonical: QuantityUnit,
    field: &'static str,
) -> Result<(), RoboticsStructuredRefusal> {
    if require_exact(value, dimension, canonical, field)? < 0 {
        return Err(RoboticsStructuredRefusal::NegativeUncertainty { field });
    }
    Ok(())
}

pub(super) fn require_nonnegative(
    value: Quantity,
    dimension: QuantityDimension,
    canonical: QuantityUnit,
    field: &'static str,
) -> Result<(), RoboticsStructuredRefusal> {
    if require_exact(value, dimension, canonical, field)? < 0 {
        return Err(RoboticsStructuredRefusal::NegativeValue { field });
    }
    Ok(())
}

pub(super) fn require_exact(
    value: Quantity,
    dimension: QuantityDimension,
    canonical: QuantityUnit,
    field: &'static str,
) -> Result<i64, RoboticsStructuredRefusal> {
    if value.dimension() != dimension {
        return Err(RoboticsStructuredRefusal::IncompatibleDimension { field });
    }
    value
        .convert(canonical)
        .map(|value| value.value())
        .map_err(|_| RoboticsStructuredRefusal::InexactPrecision { field })
}

pub(super) fn require_identity(value: &str) -> Result<(), RoboticsStructuredRefusal> {
    if value.is_empty() {
        return Err(RoboticsStructuredRefusal::EmptyIdentity);
    }
    if value.len() > MAXIMUM_ROBOTICS_IDENTITY_BYTES {
        return Err(RoboticsStructuredRefusal::IdentityTooLong);
    }
    Ok(())
}

pub(super) fn text_value(value: &str) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_identity(value)?;
    leaf_value("value/text", value.as_bytes().to_vec())
}

pub(super) fn count_value(value: u64) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    leaf_value(
        conduit_core::COUNT_INFO_ID,
        conduit_core::encode_count(value).to_vec(),
    )
}

pub(super) fn quantity_value(
    value: Quantity,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    leaf_value(conduit_core::QUANTITY_INFO_ID, value.encode().to_vec())
}

pub(super) fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        leaf_value("value/unit", Vec::new())?,
    )?)
}

fn leaf_value(
    kind: &str,
    bytes: Vec<u8>,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind))?,
        bytes,
    )?)
}

pub(super) fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}
