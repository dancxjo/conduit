//! Bounded exact linear-quantity arithmetic shared by reference and Play evaluators.
//! Scalar ratios must fit their existing millionth carrier; no rounding is implicit.
use crate::{BinaryOperator as O, PortableExpressionEvaluationRefusal as R};
use conduit_core::{
    kind_id, ExactNumericWork, ExactRational, KindId, Quantity, QuantityRole, RationalQuantity,
    Scalar, QUANTITY_ENCODED_LEN, SCALAR_INFO_ID,
};

pub(crate) fn is_quantity(kind: &str) -> bool {
    conduit_core::quantity_info_dimension(kind).is_some()
        || conduit_core::parse_quantity_role_info_id(kind).is_some()
        || kind == conduit_core::QUANTITY_INFO_ID
}
fn linear(kind: &str) -> bool {
    if kind == conduit_core::QUANTITY_INFO_ID {
        return true; // The generic carrier checks its concrete role at evaluation.
    }
    if let Some((_, role)) = conduit_core::parse_quantity_role_info_id(kind) {
        role == QuantityRole::Linear
    } else {
        conduit_core::quantity_info_dimension(kind).is_some()
            && kind != conduit_core::TEMPERATURE_INFO_ID
    }
}
fn additive(kind: &str) -> bool {
    linear(kind)
        || conduit_core::parse_quantity_role_info_id(kind)
            .is_some_and(|(_, role)| role == QuantityRole::Delta)
}
pub(crate) fn result_kind(op: O, left: &str, right: &str) -> Option<Result<KindId, ()>> {
    if !is_quantity(left) && !is_quantity(right) {
        return None;
    }
    if !matches!(
        op,
        O::Add | O::Subtract | O::Multiply | O::Divide | O::Remainder
    ) {
        return None;
    }
    Some(match op {
        O::Add | O::Subtract if left == right && additive(left) => Ok(kind_id(left)),
        O::Add | O::Subtract
            if (left == conduit_core::QUANTITY_INFO_ID
                || right == conduit_core::QUANTITY_INFO_ID)
                && additive(left)
                && additive(right) =>
        {
            Ok(kind_id(conduit_core::QUANTITY_INFO_ID))
        }
        O::Divide if left == right && linear(left) && left != conduit_core::QUANTITY_INFO_ID => {
            Ok(kind_id(SCALAR_INFO_ID))
        }
        O::Multiply | O::Divide if linear(left) && linear(right) => {
            Ok(kind_id(conduit_core::QUANTITY_INFO_ID))
        }
        O::Multiply | O::Divide if additive(left) && right == SCALAR_INFO_ID => Ok(kind_id(left)),
        O::Multiply if left == SCALAR_INFO_ID && additive(right) => Ok(kind_id(right)),
        _ => Err(()),
    })
}
pub(crate) struct Encoded {
    bytes: [u8; QUANTITY_ENCODED_LEN],
    length: usize,
}
impl Encoded {
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub(crate) fn to_vec(&self) -> alloc::vec::Vec<u8> {
        self.as_slice().to_vec()
    }
}
fn rational(
    kind: &str,
    bytes: &[u8],
    profile: conduit_core::QuantityValueProfile,
    work: &mut ExactNumericWork,
) -> Result<RationalQuantity, R> {
    let value = profile
        .decode(kind, bytes, work)
        .map_err(|_| R::InvalidProgram)?;
    if value.role() == QuantityRole::Point {
        return Err(R::QuantityProduct(
            conduit_core::RationalQuantityRefusal::InvalidRole,
        ));
    }
    value.rational(work).map_err(|_| R::Arithmetic)
}
fn scalar(bytes: &[u8], work: &mut ExactNumericWork) -> Result<ExactRational, R> {
    let value = Scalar::decode(bytes).map_err(|_| R::InvalidProgram)?;
    ExactRational::from_decimal_parts(i128::from(value.raw_microunits()), -6, work)
        .map_err(|_| R::Arithmetic)
}
/// Storage and coordinate policy are selected before evaluation. No growth or
/// representation negotiation occurs while writing a result.
pub(crate) struct Output<'a> {
    pub(crate) profile: conduit_core::QuantityValueProfile,
    pub(crate) bytes: &'a mut [u8],
}
impl Output<'_> {
    fn scalar(&mut self, value: Scalar) -> Result<usize, R> {
        let encoded = value.encode();
        let target = self
            .bytes
            .get_mut(..encoded.len())
            .ok_or(R::InvalidProgram)?;
        target.copy_from_slice(&encoded);
        Ok(encoded.len())
    }
    fn quantity(
        &mut self,
        coordinate: ExactRational,
        unit: conduit_core::Unit,
        role: QuantityRole,
        work: &mut ExactNumericWork,
    ) -> Result<usize, R> {
        if self.profile == conduit_core::QuantityValueProfile::Exact {
            let value =
                RationalQuantity::new(coordinate, unit, role).map_err(R::QuantityProduct)?;
            return conduit_core::encode_rational_quantity_value(value, self.bytes).map_err(
                |why| match why {
                    conduit_core::QuantityValueRefusal::Rational(why) => R::QuantityProduct(why),
                    _ => R::InvalidProgram,
                },
            );
        }
        let (coefficient, exponent) = coordinate
            .checked_decimal_parts(work)
            .map_err(R::QuantityProjection)?;
        let value = Quantity::from_decimal_role(coefficient, exponent, unit, role)
            .map_err(R::QuantityEncoding)?;
        let target = self
            .bytes
            .get_mut(..QUANTITY_ENCODED_LEN)
            .ok_or(R::InvalidProgram)?;
        target.copy_from_slice(&value.encode());
        Ok(QUANTITY_ENCODED_LEN)
    }
}
pub(crate) fn evaluate(
    op: O,
    left_kind: &str,
    right_kind: &str,
    left: &[u8],
    right: &[u8],
) -> Option<Result<Encoded, R>> {
    if (!is_quantity(left_kind) && !is_quantity(right_kind))
        || !matches!(
            op,
            O::Add | O::Subtract | O::Multiply | O::Divide | O::Remainder
        )
    {
        return None;
    }
    Some((|| {
        let mut encoded = Encoded {
            bytes: [0; QUANTITY_ENCODED_LEN],
            length: 0,
        };
        encoded.length = evaluate_into(
            op,
            left_kind,
            right_kind,
            left,
            right,
            Output {
                profile: conduit_core::QuantityValueProfile::Decimal,
                bytes: &mut encoded.bytes,
            },
        )?;
        Ok(encoded)
    })())
}
/// Keep finite rational scratch out of the common recursive evaluator frame.
#[inline(never)]
pub(crate) fn evaluate_into(
    op: O,
    left_kind: &str,
    right_kind: &str,
    left: &[u8],
    right: &[u8],
    mut output: Output<'_>,
) -> Result<usize, R> {
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    if is_quantity(left_kind) && is_quantity(right_kind) {
        let left = rational(left_kind, left, output.profile, &mut work)?;
        let right = rational(right_kind, right, output.profile, &mut work)?;
        if op == O::Multiply
            || (op == O::Divide
                && (left_kind != right_kind || left_kind == conduit_core::QUANTITY_INFO_ID))
        {
            let result = if op == O::Multiply {
                left.checked_product(right, &mut work)
            } else {
                left.checked_quotient(right, &mut work)
            }
            .map_err(R::QuantityProduct)?;
            return output.quantity(result.coordinate(), result.unit(), result.role(), &mut work);
        }
        let right = right
            .convert_to_unit(left.unit(), &mut work)
            .map_err(|_| R::Arithmetic)?;
        let coordinate = match op {
            O::Add => left
                .checked_add(right, &mut work)
                .map_err(|_| R::Arithmetic)?
                .coordinate(),
            O::Subtract => left
                .checked_subtract(right, &mut work)
                .map_err(|_| R::Arithmetic)?
                .coordinate(),
            O::Divide => {
                let ratio = left
                    .coordinate()
                    .checked_divide(right.coordinate(), &mut work)
                    .map_err(|_| R::Arithmetic)?;
                let micros = ratio
                    .checked_multiply(ExactRational::from_i128(1_000_000), &mut work)
                    .map_err(|_| R::Arithmetic)?
                    .checked_i128()
                    .map_err(|_| R::Arithmetic)?;
                return output.scalar(Scalar::from_raw_microunits(
                    i64::try_from(micros).map_err(|_| R::Arithmetic)?,
                ));
            }
            _ => return Err(R::InvalidProgram),
        };
        return output.quantity(coordinate, left.unit(), left.role(), &mut work);
    }
    let (value, factor, divide) = if is_quantity(left_kind) && right_kind == SCALAR_INFO_ID {
        (
            rational(left_kind, left, output.profile, &mut work)?,
            scalar(right, &mut work)?,
            op == O::Divide,
        )
    } else if left_kind == SCALAR_INFO_ID && is_quantity(right_kind) && op == O::Multiply {
        (
            rational(right_kind, right, output.profile, &mut work)?,
            scalar(left, &mut work)?,
            false,
        )
    } else {
        return Err(R::InvalidProgram);
    };
    if !matches!(op, O::Multiply | O::Divide) {
        return Err(R::InvalidProgram);
    }
    let coordinate = if divide {
        value.coordinate().checked_divide(factor, &mut work)
    } else {
        value.coordinate().checked_multiply(factor, &mut work)
    }
    .map_err(|_| R::Arithmetic)?;
    output.quantity(coordinate, value.unit(), value.role(), &mut work)
}

/// Physical equality/order can cross coordinate representations. Preserve the
/// existing native decimal comparison, including its wide scientific exponents.
#[inline(never)]
pub(crate) fn compare(left: &[u8], right: &[u8]) -> Result<core::cmp::Ordering, R> {
    if left.first() == Some(&1) && right.first() == Some(&1) {
        return Quantity::decode(left)
            .map_err(|_| R::InvalidProgram)?
            .compare(Quantity::decode(right).map_err(|_| R::InvalidProgram)?)
            .map_err(|_| R::Arithmetic);
    }
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let left =
        conduit_core::QuantityValueView::decode(left, &mut work).map_err(|_| R::InvalidProgram)?;
    let right =
        conduit_core::QuantityValueView::decode(right, &mut work).map_err(|_| R::InvalidProgram)?;
    left.compare(right, &mut work).map_err(|_| R::Arithmetic)
}

/// Stable Host-call details for the finite derived-quantity failure boundary.
/// Values 40..47 are separate from scalar arithmetic and Angle domains.
pub fn quantity_arithmetic_failure_detail(refusal: &R) -> u16 {
    use conduit_core::{ExactNumericRefusal as N, RationalQuantityRefusal as Q};
    match refusal {
        R::QuantityProduct(Q::InvalidRole) => 40,
        R::QuantityProduct(Q::Numeric(N::DivisionByZero)) => 41,
        R::QuantityProduct(Q::Numeric(N::WorkLimit)) => 42,
        R::QuantityProduct(Q::Unit(_)) => 44,
        R::QuantityProduct(_) => 43,
        R::QuantityProjection(N::InexactProjection) => 45,
        R::QuantityEncoding(_) => 46,
        R::QuantityProjection(_) => 47,
        _ => 0,
    }
}

#[cfg(test)]
#[path = "expression_quantity_tests.rs"]
mod tests;
