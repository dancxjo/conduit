//! An admitted absolute tolerance over retained physical numerical evidence.
use super::*;
use crate::{
    ExactNumericWork, PreparedNearQuantityVerifier, Quantity, QuantityRole, RationalQuantity,
};

/// A finite validation budget, included by callers in mandatory work admission.
pub const NEAR_VALUE_CONSTRAINT_WORK: u64 = 40 * crate::MATHEMATICAL_VALUE_VALIDATION_WORK;

pub(super) fn validate_definition(
    value_kind: &str,
    maximum_bytes: u32,
    schema: &[u8],
    source_kind: &KindId,
    limit: &Quantity,
) -> Result<(), ConstraintDefinitionError> {
    let invalid = || ConstraintDefinitionError::InvalidNearQuantity;
    if maximum_bytes == 0 || maximum_bytes as usize > crate::NEAR_QUANTITY_EVIDENCE_MAX_BYTES {
        return Err(invalid());
    }
    let ty = crate::StructuredInfoType::from_canonical_bytes(schema).map_err(|_| invalid())?;
    if ty.profile().map_err(|_| invalid())?.value_kind().as_str() != value_kind {
        return Err(invalid());
    }
    PreparedNearQuantityVerifier::new(&ty).map_err(|_| invalid())?;
    if limit.role() == QuantityRole::Point {
        return Err(invalid());
    }
    let mut work = ExactNumericWork::new(NEAR_VALUE_CONSTRAINT_WORK);
    let rational = RationalQuantity::from_quantity(*limit, &mut work).map_err(|_| invalid())?;
    if rational.coordinate().is_negative() {
        return Err(invalid());
    }
    if let Some((family, role)) = crate::parse_quantity_role_info_id(source_kind.as_str()) {
        let tolerance_role = if role == QuantityRole::Point {
            QuantityRole::Delta
        } else {
            role
        };
        if family != limit.family().identity() || limit.role() != tolerance_role {
            return Err(invalid());
        }
    } else if let Some(dimension) = crate::quantity_info_dimension(source_kind.as_str()) {
        if dimension != limit.dimension() {
            return Err(invalid());
        }
    } else if source_kind.as_str() != crate::QUANTITY_INFO_ID {
        return Err(invalid());
    }
    Ok(())
}

#[inline(never)]
pub(super) fn validate(
    schema: &[u8],
    source_kind: &KindId,
    limit: &Quantity,
    canonical: &[u8],
) -> Result<(), ValueConstraintRefusal> {
    let mut work = ExactNumericWork::new(NEAR_VALUE_CONSTRAINT_WORK);
    let limit = RationalQuantity::from_quantity(*limit, &mut work).map_err(|_| {
        ValueConstraintRefusal::NearQuantity(crate::NearQuantityEvidenceRefusal::InvalidLimit)
    })?;
    PreparedNearQuantityVerifier::verify_with_schema(
        schema,
        canonical,
        source_kind.as_str(),
        limit,
        None,
        &mut work,
    )
    .map(|_| ())
    .map_err(ValueConstraintRefusal::NearQuantity)
}
