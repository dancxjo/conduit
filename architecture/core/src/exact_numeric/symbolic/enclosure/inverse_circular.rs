//! Principal asin/acos with certified root and monotone arctangent bounds.
use super::{
    ArctangentTurnsRequirements, MathematicalEnclosureRefusal as R,
    MathematicalInterval as Interval, PreparedArctangentTurns,
};
use crate::{
    ExactNumericRefusal, ExactNumericWork as Work, ExactRational as Q, PreparedSquareRoot,
    SquareRootRefusal,
};
use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseCircularFunction {
    Arcsine,
    Arccosine,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseCircularBranch {
    Principal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InverseCircularRequirements {
    pub branch: InverseCircularBranch,
    pub angle: ArctangentTurnsRequirements,
    pub maximum_root_width: Q,
    pub maximum_root_refinements: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseCircularRefusal {
    OutsideUnitDomain,
    UnresolvedPositiveRoot,
    Enclosure(R),
}
impl From<ExactNumericRefusal> for InverseCircularRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Enclosure(R::Numeric(value))
    }
}
impl From<R> for InverseCircularRefusal {
    fn from(value: R) -> Self {
        Self::Enclosure(value)
    }
}
impl From<SquareRootRefusal> for InverseCircularRefusal {
    fn from(value: SquareRootRefusal) -> Self {
        Self::Enclosure(R::Root(value))
    }
}

/// Source x is dimensionless. Endpoints are turn coordinates: asin uses
/// [-1/4, 1/4], acos [0, 1/2]. No machine radian input or output is implied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedInverseCircular {
    function: InverseCircularFunction,
    input: Q,
    target: Interval,
    root: Option<PreparedSquareRoot>,
    atan: Option<(PreparedArctangentTurns, PreparedArctangentTurns)>,
    width_turns: Q,
    requirements: InverseCircularRequirements,
    limb_work: u64,
}
impl PreparedInverseCircular {
    pub fn prepare(
        function: InverseCircularFunction,
        input: Q,
        requirements: InverseCircularRequirements,
        work: &mut Work,
    ) -> Result<Self, InverseCircularRefusal> {
        use InverseCircularRefusal as Refusal;
        if requirements.maximum_root_width.is_negative() {
            return Err(R::NegativeTolerance.into());
        }
        let before = work.spent();
        // Validate the angular profile even when an exact endpoint needs no series.
        PreparedArctangentTurns::prepare(Q::ZERO, requirements.angle, work)?;
        let magnitude = if input.is_negative() {
            input.negated()
        } else {
            input
        };
        if magnitude.checked_compare(Q::ONE, work)? == Ordering::Greater {
            return Err(Refusal::OutsideUnitDomain);
        }
        let quarter = Q::ratio(1, 4, work)?;
        let (asin, root, atan) = if magnitude == Q::ONE {
            (
                Interval::point(if input.is_negative() {
                    quarter.negated()
                } else {
                    quarter
                }),
                None,
                None,
            )
        } else if input == Q::ZERO {
            (Interval::point(Q::ZERO), None, None)
        } else {
            let radicand = Q::ONE.checked_subtract(input.checked_multiply(input, work)?, work)?;
            let root = PreparedSquareRoot::prepare(
                radicand,
                requirements.maximum_root_width,
                requirements.maximum_root_refinements,
                work,
            )?;
            let bounds = root.target();
            if bounds.lower() == Q::ZERO {
                return Err(Refusal::UnresolvedPositiveRoot);
            }
            // Division reverses endpoint order for a negative numerator.
            let (lower_denominator, upper_denominator) = if input.is_negative() {
                (bounds.lower(), bounds.upper())
            } else {
                (bounds.upper(), bounds.lower())
            };
            let lower = PreparedArctangentTurns::prepare(
                input.checked_divide(lower_denominator, work)?,
                requirements.angle,
                work,
            )?;
            let upper = PreparedArctangentTurns::prepare(
                input.checked_divide(upper_denominator, work)?,
                requirements.angle,
                work,
            )?;
            let asin = Interval::bounds(lower.target().lower(), upper.target().upper());
            (asin, Some(root), Some((lower, upper)))
        };
        let target = match function {
            InverseCircularFunction::Arcsine => asin,
            InverseCircularFunction::Arccosine => Interval::point(quarter).subtract(asin, work)?,
        };
        let width_turns = target.width(work)?;
        if width_turns.checked_compare(requirements.angle.maximum_width_turns, work)?
            == Ordering::Greater
        {
            return Err(R::ToleranceExceeded.into());
        }
        Ok(Self {
            function,
            input,
            target,
            root,
            atan,
            width_turns,
            requirements,
            limb_work: work.spent() - before,
        })
    }
    pub const fn function(&self) -> InverseCircularFunction {
        self.function
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
    pub const fn requirements(&self) -> InverseCircularRequirements {
        self.requirements
    }
    pub const fn root(&self) -> Option<&PreparedSquareRoot> {
        self.root.as_ref()
    }
    pub fn arctangent_bounds(
        &self,
    ) -> Option<(&PreparedArctangentTurns, &PreparedArctangentTurns)> {
        self.atan.as_ref().map(|(lower, upper)| (lower, upper))
    }
    pub const fn limb_work(&self) -> u64 {
        self.limb_work
    }
}
