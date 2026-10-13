//! Principal real square-root enclosure using exact rational bisection.
//! Preparation proves containment; inspection never repeats numerical work.
use super::{ExactNumericRefusal, ExactNumericWork, ExactRational};
use crate::projection::*;
use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquareRootRefusal {
    Numeric(ExactNumericRefusal),
    NegativeRadicand,
    NegativeTolerance,
    RefinementLimit,
}
impl From<ExactNumericRefusal> for SquareRootRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}

/// The principal nonnegative root lies between these exact rational endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SquareRootEnclosure {
    lower: ExactRational,
    upper: ExactRational,
}
impl SquareRootEnclosure {
    pub const fn lower(self) -> ExactRational {
        self.lower
    }
    pub const fn upper(self) -> ExactRational {
        self.upper
    }
}

/// Private custody seals the proof lower² <= radicand <= upper². Width bounds
/// either endpoint's absolute error, without claiming a rational root is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedSquareRoot {
    radicand: ExactRational,
    target: SquareRootEnclosure,
    width: ExactRational,
    maximum_width: ExactRational,
    refinements: u32,
}
impl PreparedSquareRoot {
    /// Both refinement count and limb work are caller-admitted finite bounds.
    /// A zero tolerance succeeds only when bisection proves an exact root.
    pub fn prepare(
        radicand: ExactRational,
        maximum_width: ExactRational,
        maximum_refinements: u32,
        work: &mut ExactNumericWork,
    ) -> Result<Self, SquareRootRefusal> {
        if radicand.is_negative() {
            return Err(SquareRootRefusal::NegativeRadicand);
        }
        if maximum_width.is_negative() {
            return Err(SquareRootRefusal::NegativeTolerance);
        }
        let mut lower = ExactRational::ZERO;
        let mut upper = if radicand.checked_compare(ExactRational::ONE, work)? == Ordering::Less {
            ExactRational::ONE
        } else {
            radicand
        };
        if radicand == ExactRational::ZERO {
            upper = ExactRational::ZERO;
        }
        if radicand == ExactRational::ONE {
            lower = upper;
        }
        let mut refinements = 0;
        loop {
            let width = upper.checked_subtract(lower, work)?;
            if width.checked_compare(maximum_width, work)? != Ordering::Greater {
                return Ok(Self {
                    radicand,
                    target: SquareRootEnclosure { lower, upper },
                    width,
                    maximum_width,
                    refinements,
                });
            }
            if refinements == maximum_refinements {
                return Err(SquareRootRefusal::RefinementLimit);
            }
            let midpoint = lower
                .checked_add(upper, work)?
                .checked_divide(ExactRational::from_i128(2), work)?;
            let square = midpoint.checked_multiply(midpoint, work)?;
            match square.checked_compare(radicand, work)? {
                Ordering::Less => lower = midpoint,
                Ordering::Greater => upper = midpoint,
                Ordering::Equal => {
                    lower = midpoint;
                    upper = midpoint;
                }
            }
            refinements += 1;
        }
    }
    pub const fn radicand(self) -> ExactRational {
        self.radicand
    }
    pub const fn target(self) -> SquareRootEnclosure {
        self.target
    }
    pub const fn width(self) -> ExactRational {
        self.width
    }
    pub const fn maximum_width(self) -> ExactRational {
        self.maximum_width
    }
    pub const fn refinements(self) -> u32 {
        self.refinements
    }
    pub fn facts(&self) -> [ProjectionFact<'_, SquareRootDomain>; 2] {
        [
            if self.width == ExactRational::ZERO {
                ProjectionFact::Preserved {
                    obligation: text(VALUE),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(VALUE),
                    class: ProjectionLoss::Precision,
                    detail: &self.width,
                    native_fact: None,
                }
            },
            ProjectionFact::Transformed {
                obligation: text(REPRESENTATION),
                law: text(LAW),
            },
        ]
    }
}
const VALUE: &str = "exact-denotation";
const REPRESENTATION: &str = "numeric-representation";
const LAW: &str = "numeric/principal-square-root-enclosure@1";
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("reviewed bounded numerical identity")
}
#[derive(Debug, Clone, Copy)]
pub struct SquareRootDomain;
impl ProjectionDomain for SquareRootDomain {
    type Source = PreparedSquareRoot;
    type Target = SquareRootEnclosure;
    type Detail = ExactRational;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("numeric/principal-square-root@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text(LAW)
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if mechanism != ProjectionMechanism::Completed
            || !native.is_empty()
            || !scores.is_empty()
            || target != Some(&source.target)
            || facts.len() != 2
        {
            return false;
        }
        let value = facts.iter().any(|fact| match fact {
            ProjectionFact::Preserved { obligation } => {
                obligation.as_str() == VALUE && source.width == ExactRational::ZERO
            }
            ProjectionFact::Lost {
                obligation,
                class,
                detail,
                native_fact,
            } => {
                obligation.as_str() == VALUE
                    && *class == ProjectionLoss::Precision
                    && **detail == source.width
                    && native_fact.is_none()
                    && source.width != ExactRational::ZERO
            }
            _ => false,
        });
        value
            && facts.iter().any(|fact| {
                matches!(fact,
            ProjectionFact::Transformed { obligation, law }
            if obligation.as_str() == REPRESENTATION && law.as_str() == LAW)
            })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct SquareRootPolicy {
    pub maximum_width: ExactRational,
}
impl ProjectionPolicy<SquareRootDomain> for SquareRootPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/square-root-enclosure-policy@1")
    }
    fn permits(
        &self,
        source: &PreparedSquareRoot,
        target: &SquareRootEnclosure,
        facts: &[ProjectionFact<'_, SquareRootDomain>],
    ) -> bool {
        self.maximum_width == source.maximum_width
            && SquareRootDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
