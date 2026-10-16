//! Certified principal arctangent in turn coordinates, without host floats.
use super::{pi, MathematicalEnclosureRefusal as R, MathematicalInterval as Interval};
use crate::{ExactNumericWork as Work, ExactRational as Q};
use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArctangentTurnsRequirements {
    pub maximum_width_turns: Q,
    /// Width admitted for each alternating-series enclosure, including π.
    pub maximum_series_width: Q,
    pub maximum_terms: u16,
}

/// Exact input plus a certified enclosure on the principal (-1/4, 1/4)
/// branch. Endpoints are rational turn coordinates, never implicit radians.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedArctangentTurns {
    input: Q,
    target: Interval,
    width_turns: Q,
    requirements: ArctangentTurnsRequirements,
    maximum_used_terms: u16,
    limb_work: u64,
}

impl PreparedArctangentTurns {
    pub fn prepare(
        input: Q,
        requirements: ArctangentTurnsRequirements,
        work: &mut Work,
    ) -> Result<Self, R> {
        if requirements.maximum_width_turns.is_negative()
            || requirements.maximum_series_width.is_negative()
        {
            return Err(R::NegativeTolerance);
        }
        let before = work.spent();
        work.charge(1)?;
        let negative = input.is_negative();
        let magnitude = if negative { input.negated() } else { input };
        let eighth = Q::ratio(1, 8, work)?;
        let quarter = Q::ratio(1, 4, work)?;
        let (mut target, maximum_used_terms) = if magnitude == Q::ZERO {
            (Interval::point(Q::ZERO), 0)
        } else if magnitude == Q::ONE {
            (Interval::point(eighth), 0)
        } else {
            // atan(x) = π/2 - atan(1/x) for x > 1. Then
            // atan(x) = π/4 + atan((x-1)/(x+1)) for 1/2 < x <= 1.
            // The remaining series argument has magnitude at most 1/2.
            let reciprocal = magnitude.checked_compare(Q::ONE, work)? == Ordering::Greater;
            let reduced = if reciprocal {
                Q::ONE.checked_divide(magnitude, work)?
            } else {
                magnitude
            };
            let shifted =
                reduced.checked_compare(Q::ratio(1, 2, work)?, work)? == Ordering::Greater;
            let argument = if shifted {
                reduced
                    .checked_subtract(Q::ONE, work)?
                    .checked_divide(reduced.checked_add(Q::ONE, work)?, work)?
            } else {
                reduced
            };
            let (radians, terms) = pi::enclose_arctangent(
                argument,
                requirements.maximum_series_width,
                requirements.maximum_terms,
                work,
            )?;
            // A coarse requested series width must still prove a positive π
            // interval before reciprocal interval arithmetic is admissible.
            let pi_width = if requirements
                .maximum_series_width
                .checked_compare(eighth, work)?
                == Ordering::Greater
            {
                eighth
            } else {
                requirements.maximum_series_width
            };
            let (pi, pi_terms) = pi::enclose(pi_width, requirements.maximum_terms, work)?;
            if pi.lower().checked_compare(Q::ZERO, work)? != Ordering::Greater {
                return Err(R::InvalidExpression);
            }
            let reciprocal_tau = Interval::bounds(
                Q::ONE.checked_divide(pi.upper().checked_multiply(Q::from_i128(2), work)?, work)?,
                Q::ONE.checked_divide(pi.lower().checked_multiply(Q::from_i128(2), work)?, work)?,
            );
            let mut turns = radians.multiply(reciprocal_tau, work)?;
            if shifted {
                turns = Interval::point(eighth).add(turns, work)?;
            }
            if reciprocal {
                turns = Interval::point(quarter).subtract(turns, work)?;
            }
            (turns, terms.max(pi_terms))
        };
        if negative {
            target = target.negated();
        }
        let width_turns = target.width(work)?;
        if width_turns.checked_compare(requirements.maximum_width_turns, work)? == Ordering::Greater
        {
            return Err(R::ToleranceExceeded);
        }
        Ok(Self {
            input,
            target,
            width_turns,
            requirements,
            maximum_used_terms,
            limb_work: work.spent() - before,
        })
    }
    pub const fn input(&self) -> Q {
        self.input
    }
    pub const fn target(&self) -> Interval {
        self.target
    }
    pub const fn width_turns(&self) -> Q {
        self.width_turns
    }
    pub const fn requirements(&self) -> ArctangentTurnsRequirements {
        self.requirements
    }
    pub const fn maximum_used_terms(&self) -> u16 {
        self.maximum_used_terms
    }
    pub const fn limb_work(&self) -> u64 {
        self.limb_work
    }
}
