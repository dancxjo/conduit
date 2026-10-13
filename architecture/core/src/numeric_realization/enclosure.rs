//! Denotation enclosure follows the normalized symbolic DAG; execution ordering
//! remains separately bound by the NumericComputation certificate.
use super::*;
use crate::{MathValue, NumericRefusal, SymbolicNodeRef, SymbolicOperation};
use alloc::vec::Vec;
fn dyadic_outward(
    value: &RationalInterval,
    work: &mut NumericRealizationWork,
) -> Result<RationalInterval, NumericRefusal> {
    let lower = Binary64Value::project(value.lower(), Binary64Rounding::TowardNegative, work)?
        .value
        .exact()?
        .ok_or(NumericRefusal::IndeterminateDomain)?;
    let upper = Binary64Value::project(value.upper(), Binary64Rounding::TowardPositive, work)?
        .value
        .exact()?
        .ok_or(NumericRefusal::IndeterminateDomain)?;
    RationalInterval::new(lower, upper)
}
pub fn enclose_math_value(
    value: &MathValue,
    work: &mut NumericRealizationWork,
) -> Result<RationalInterval, NumericRefusal> {
    if value.as_rational().is_some() {
        return RationalInterval::point(value.clone());
    }
    let MathValue::Symbolic(dag) = value else {
        return Err(NumericRefusal::IndeterminateDomain);
    };
    let mut values: Vec<RationalInterval> = Vec::with_capacity(dag.node_count());
    for node in dag.nodes() {
        work.charge(1)?;
        let interval = match node {
            SymbolicNodeRef::Literal(value) => enclose_math_value(value, work)?,
            SymbolicNodeRef::Apply {
                operation,
                operands,
            } => {
                let operand = |index| &values[usize::from(operands[index])];
                match operation {
                    SymbolicOperation::Pi => enclose_pi(12, work)?,
                    SymbolicOperation::Negate => operand(0).negate()?,
                    SymbolicOperation::Add => operand(0).add(operand(1))?,
                    SymbolicOperation::Multiply => operand(0).multiply(operand(1))?,
                    SymbolicOperation::Divide => operand(0).divide(operand(1))?,
                    SymbolicOperation::Root(2) => RationalInterval::new(
                        enclose_sqrt(operand(0).lower(), 80, work)?.lower().clone(),
                        enclose_sqrt(operand(0).upper(), 80, work)?.upper().clone(),
                    )?,
                    SymbolicOperation::Sin | SymbolicOperation::Cos | SymbolicOperation::Tan => {
                        // Symbolic trig operands are turns. pi is captured exact
                        // mathematical meaning; this enclosure is algorithm evidence.
                        let radians = dyadic_outward(
                            &operand(0)
                                .multiply(&enclose_pi(12, work)?)?
                                .multiply(&RationalInterval::point(MathValue::integer(2))?)?,
                            work,
                        )?;
                        if operation == SymbolicOperation::Cos {
                            enclose_sin_cos_radians(&radians, 12, true, work)?
                        } else {
                            let sin = enclose_sin_cos_radians(&radians, 12, false, work)?;
                            if operation == SymbolicOperation::Sin {
                                sin
                            } else {
                                sin.divide(&enclose_sin_cos_radians(&radians, 12, true, work)?)?
                            }
                        }
                    }
                    _ => return Err(NumericRefusal::IndeterminateDomain),
                }
            }
        };
        values.push(interval);
    }
    values.pop().ok_or(NumericRefusal::InvalidNumber)
}

/// A symbolic literal may round only when both certified enclosure endpoints
/// select the same bits, and its exception custody is proved independently.
pub(super) fn project_mathematical_literal(
    value: &MathValue,
    rounding: Binary64Rounding,
    work: &mut NumericRealizationWork,
) -> Result<Binary64Result, NumericRefusal> {
    if value.as_rational().is_some() {
        return Binary64Value::project(value, rounding, work);
    }
    let interval = enclose_math_value(value, work)?;
    let lower = Binary64Value::project(interval.lower(), rounding, work)?;
    let upper = Binary64Value::project(interval.upper(), rounding, work)?;
    if lower.value.bits() != upper.value.bits() {
        return Err(NumericRefusal::IndeterminateDomain);
    }
    let Some(observed) = lower.value.exact()? else {
        if lower.value.is_infinite() && lower.flags.overflow && upper.flags.overflow {
            return Ok(Binary64Result {
                value: lower.value,
                flags: lower.flags.union(upper.flags),
            });
        }
        return Err(NumericRefusal::IndeterminateDomain);
    };
    use core::cmp::Ordering;
    let inexact = observed.compare(interval.lower())? == Some(Ordering::Less)
        || observed.compare(interval.upper())? == Some(Ordering::Greater);
    if !inexact {
        return Err(NumericRefusal::IndeterminateDomain);
    }
    Ok(Binary64Result {
        value: lower.value,
        flags: Binary64Flags {
            inexact: true,
            underflow: lower.value.bits() & 0x7ff0_0000_0000_0000 == 0,
            ..Default::default()
        },
    })
}
