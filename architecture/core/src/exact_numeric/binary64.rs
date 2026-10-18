//! Prepared finite binary64 projection, with a verified absolute error claim.
use super::{ExactNumericRefusal, ExactNumericWork, ExactRational};
use crate::IeeeF64;
use core::cmp::Ordering;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Binary64Rounding {
    NearestTiesEven,
    TowardZero,
    TowardPositive,
    TowardNegative,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Binary64Refusal {
    Numeric(ExactNumericRefusal),
    NegativeTolerance,
    ToleranceExceeded,
}
impl From<ExactNumericRefusal> for Binary64Refusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
/// Exact distance when representable under this profile; otherwise a checked
/// requested bound. Neither variant is statistical measurement uncertainty.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Binary64ErrorClaim {
    ExactDistance(ExactRational),
    CertifiedBound(ExactRational),
}
impl Binary64ErrorClaim {
    pub const fn maximum(self) -> ExactRational {
        match self {
            Self::ExactDistance(v) | Self::CertifiedBound(v) => v,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Binary64Status {
    pub inexact: bool,
    /// Tininess is detected after precision rounding with unbounded exponent
    /// range. The final target can be min-normal and still be tiny-inexact.
    /// An exact subnormal is not tiny-inexact.
    pub tiny_inexact: bool,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PreparedBinary64Projection {
    source: ExactRational,
    target: IeeeF64,
    represented: ExactRational,
    rounding: Binary64Rounding,
    status: Binary64Status,
    error: Binary64ErrorClaim,
    maximum_error: ExactRational,
    spent_limb_work: u64,
    admitted_limb_work: u64,
}
impl PreparedBinary64Projection {
    pub fn prepare(
        source: ExactRational,
        rounding: Binary64Rounding,
        maximum_error: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Binary64Refusal> {
        if maximum_error.is_negative() {
            return Err(Binary64Refusal::NegativeTolerance);
        }
        let before = work.spent();
        let admitted_limb_work = work.remaining();
        let (target, inexact) = source.round_binary64(rounding, work)?;
        let represented = ExactRational::from_binary64(target, work)?;
        let error = error_claim(source, represented, maximum_error, work)?;
        let tiny_inexact = inexact && tininess::after_rounding(source, target, rounding, work)?;
        Ok(Self {
            source,
            target,
            represented,
            rounding,
            status: Binary64Status {
                inexact,
                tiny_inexact,
            },
            error,
            maximum_error,
            spent_limb_work: work.spent() - before,
            admitted_limb_work,
        })
    }
    pub const fn source(&self) -> ExactRational {
        self.source
    }
    pub const fn target(&self) -> IeeeF64 {
        self.target
    }
    pub const fn represented(&self) -> ExactRational {
        self.represented
    }
    pub const fn rounding(&self) -> Binary64Rounding {
        self.rounding
    }
    pub const fn status(&self) -> Binary64Status {
        self.status
    }
    pub const fn error(&self) -> Binary64ErrorClaim {
        self.error
    }
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    pub const fn spent_limb_work(&self) -> u64 {
        self.spent_limb_work
    }
    pub const fn admitted_limb_work(&self) -> u64 {
        self.admitted_limb_work
    }
}
/// Continued-fraction comparisons can prove containment without a cross-product
/// wider than the admitted exact storage, even when the distance cannot fit.
pub(super) fn error_claim(
    source: ExactRational,
    represented: ExactRational,
    tolerance: ExactRational,
    work: &mut ExactNumericWork,
) -> Result<Binary64ErrorClaim, Binary64Refusal> {
    match source.checked_subtract(represented, work) {
        Ok(distance) => {
            let distance = if distance.is_negative() {
                distance.negated()
            } else {
                distance
            };
            if distance.checked_compare(tolerance, work)? == Ordering::Greater {
                return Err(Binary64Refusal::ToleranceExceeded);
            }
            Ok(Binary64ErrorClaim::ExactDistance(distance))
        }
        Err(ExactNumericRefusal::ExtentExceeded) => {
            let lower = represented.checked_subtract(tolerance, work)?;
            let upper = represented.checked_add(tolerance, work)?;
            if source.checked_compare(lower, work)? == Ordering::Less
                || source.checked_compare(upper, work)? == Ordering::Greater
            {
                return Err(Binary64Refusal::ToleranceExceeded);
            }
            Ok(Binary64ErrorClaim::CertifiedBound(tolerance))
        }
        Err(why) => Err(why.into()),
    }
}
mod algorithm;
mod projection;
mod tininess;
pub use algorithm::*;
pub use projection::*;
