//! Exact-type equality across admitted structured values. IEEE leaves follow
//! core's encoded-bit identity; ordered scalar comparison keeps its prior law.
use super::*;

pub(super) fn equal(
    left: Value,
    right: Value,
) -> Result<bool, PortableExpressionEvaluationRefusal> {
    if left.value_type != right.value_type {
        return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
    }
    if let Ok(kind) = leaf_kind(&left.value_type) {
        if kind == conduit_core::UNIT_INFO_ID {
            let left = conduit_core::Unit::decode(&left.encoded)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
            let right = conduit_core::Unit::decode(&right.encoded)
                .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
            return Ok(left.same_physical_definition(right));
        }
        return if matches!(
            kind,
            conduit_core::F32_INFO_ID
                | conduit_core::F64_INFO_ID
                | conduit_core::EMPTY_INFO_ID
                | conduit_core::UNIT_INFO_ID
        ) {
            Ok(left.encoded == right.encoded)
        } else {
            Ok(compare(&left, &right)?.is_eq())
        };
    }
    let left = structured::structured_value(left)?;
    let right = structured::structured_value(right)?;
    equal_structured(&left, &right)
}
fn equal_structured(
    left: &StructuredInfoValue,
    right: &StructuredInfoValue,
) -> Result<bool, PortableExpressionEvaluationRefusal> {
    if left.value_type() != right.value_type() {
        return Ok(false);
    }
    match (left.shape(), right.shape()) {
        (StructuredInfoValueShape::Leaf(a), StructuredInfoValueShape::Leaf(b)) => {
            let kind = leaf_kind(left.value_type())?;
            if kind == conduit_core::UNIT_INFO_ID {
                let left = conduit_core::Unit::decode(a)
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
                let right = conduit_core::Unit::decode(b)
                    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
                return Ok(left.same_physical_definition(right));
            }
            if matches!(
                kind,
                conduit_core::F32_INFO_ID
                    | conduit_core::F64_INFO_ID
                    | conduit_core::EMPTY_INFO_ID
                    | conduit_core::UNIT_INFO_ID
            ) {
                Ok(a == b)
            } else {
                Ok(compare(
                    &encoded_structured(left.clone())?,
                    &encoded_structured(right.clone())?,
                )?
                .is_eq())
            }
        }
        (StructuredInfoValueShape::Collection(a), StructuredInfoValueShape::Collection(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (left, right) in a.iter().zip(b) {
                if !equal_structured(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (StructuredInfoValueShape::Record(a), StructuredInfoValueShape::Record(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (left, right) in a.iter().zip(b) {
                if left.name() != right.name() || !equal_structured(left.value(), right.value())? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (
            StructuredInfoValueShape::Variant {
                tag: a,
                payload: left,
            },
            StructuredInfoValueShape::Variant {
                tag: b,
                payload: right,
            },
        ) => Ok(a == b && equal_structured(left, right)?),
        _ => Ok(false),
    }
}
