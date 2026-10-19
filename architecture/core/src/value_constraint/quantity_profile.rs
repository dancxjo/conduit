//! Profile-aware admission retains the existing authored contract and range law.
use super::*;
use crate::{ExactNumericWork, Quantity, QuantityValueProfile, QuantityValueView};

/// A selected coordinate profile composes the existing semantic predicate
/// refusal with physical/codec admission. Legacy validator errors are unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuantityProfileContractRefusal {
    Contract(ValueConstraintRefusal),
    Quantity(crate::QuantityValueContractRefusal),
}
impl CheckedValueContract {
    /// Optional coordinate codecs require an explicitly selected wire profile.
    /// The ordinary `validate` entrance keeps its native decimal contract.
    pub fn validate_with_quantity_profile(
        &self,
        canonical: &[u8],
        profile: QuantityValueProfile,
    ) -> Result<(), QuantityProfileContractRefusal> {
        use QuantityProfileContractRefusal as R;
        check_size(self, canonical).map_err(R::Contract)?;
        if quantity_kind(self.value_kind.as_str()) {
            validate_quantity(self.value_kind.as_str(), canonical, profile).map_err(R::Quantity)?;
        } else {
            validate_primitive_info(self.value_kind.as_str(), canonical)
                .map_err(|why| R::Contract(ValueConstraintRefusal::MalformedPrimitive(why)))?;
        }
        validate_constraints(self, canonical).map_err(R::Contract)
    }
}

pub(super) fn validate_contract(
    contract: &CheckedValueContract,
    canonical: &[u8],
) -> Result<(), ValueConstraintRefusal> {
    check_size(contract, canonical)?;
    validate_primitive_info(contract.value_kind.as_str(), canonical)
        .map_err(ValueConstraintRefusal::MalformedPrimitive)?;
    validate_constraints(contract, canonical)
}
fn check_size(
    contract: &CheckedValueContract,
    canonical: &[u8],
) -> Result<(), ValueConstraintRefusal> {
    if canonical.len() > contract.maximum_bytes as usize {
        return Err(ValueConstraintRefusal::Oversize {
            actual: u32::try_from(canonical.len()).unwrap_or(u32::MAX),
            maximum: contract.maximum_bytes,
        });
    }
    Ok(())
}
fn validate_constraints(
    contract: &CheckedValueContract,
    canonical: &[u8],
) -> Result<(), ValueConstraintRefusal> {
    for constraint in &contract.constraints {
        constraint.validate_value(contract.value_kind.as_str(), canonical)?;
    }
    Ok(())
}
fn quantity_kind(kind: &str) -> bool {
    kind == crate::QUANTITY_INFO_ID
        || crate::quantity_info_dimension(kind).is_some()
        || crate::parse_quantity_role_info_id(kind).is_some()
}
#[inline(never)]
fn validate_quantity(
    kind: &str,
    bytes: &[u8],
    profile: QuantityValueProfile,
) -> Result<(), crate::QuantityValueContractRefusal> {
    profile
        .decode(
            kind,
            bytes,
            &mut ExactNumericWork::new(crate::MATHEMATICAL_VALUE_VALIDATION_WORK),
        )
        .map(|_| ())
}

pub(super) fn validate_range(
    canonical: &[u8],
    minimum: Option<&Quantity>,
    maximum: Option<&Quantity>,
    minimum_endpoint: IntervalEndpoint,
    maximum_endpoint: IntervalEndpoint,
) -> Result<(), ValueConstraintRefusal> {
    if canonical.first() == Some(&1) {
        let value =
            Quantity::decode(canonical).map_err(|_| ValueConstraintRefusal::QuantityRange)?;
        let above = minimum.is_none_or(|bound| {
            value
                .compare(*bound)
                .is_ok_and(|order| lower_accepts(order, minimum_endpoint))
        });
        let below = maximum.is_none_or(|bound| {
            value
                .compare(*bound)
                .is_ok_and(|order| upper_accepts(order, maximum_endpoint))
        });
        return (above && below)
            .then_some(())
            .ok_or(ValueConstraintRefusal::QuantityRange);
    }
    exact_range(
        canonical,
        minimum,
        maximum,
        minimum_endpoint,
        maximum_endpoint,
    )
}
#[inline(never)]
fn exact_range(
    canonical: &[u8],
    minimum: Option<&Quantity>,
    maximum: Option<&Quantity>,
    minimum_endpoint: IntervalEndpoint,
    maximum_endpoint: IntervalEndpoint,
) -> Result<(), ValueConstraintRefusal> {
    let mut work = ExactNumericWork::new(3 * crate::MATHEMATICAL_VALUE_VALIDATION_WORK);
    // Outer checked-contract admission still selects the wire profile first.
    // This semantic predicate can compare a rational coordinate in any admitted
    // form; unresolved symbolic ordering remains a named range refusal.
    let value = QuantityValueProfile::Mathematical
        .decode(crate::QUANTITY_INFO_ID, canonical, &mut work)
        .map_err(|_| ValueConstraintRefusal::QuantityRange)?;
    for (bound, endpoint, lower) in [
        (minimum, minimum_endpoint, true),
        (maximum, maximum_endpoint, false),
    ] {
        if let Some(bound) = bound {
            let bytes = bound.encode();
            let bound = QuantityValueView::decode(&bytes, &mut work)
                .map_err(|_| ValueConstraintRefusal::QuantityRange)?;
            let order = value
                .compare(bound, &mut work)
                .map_err(|_| ValueConstraintRefusal::QuantityRange)?;
            if !(if lower {
                lower_accepts(order, endpoint)
            } else {
                upper_accepts(order, endpoint)
            }) {
                return Err(ValueConstraintRefusal::QuantityRange);
            }
        }
    }
    Ok(())
}
