//! Bounded binary32 projection, sharing the existing absolute-error evidence.
use super::{
    Binary64ErrorClaim, Binary64Refusal, Binary64Rounding, ExactNumericRefusal, ExactNumericWork,
    ExactRational,
};
use crate::IeeeF32;
mod projection;
pub use projection::*;

/// The rounding policies and exact-distance/certified-bound claims are shared
/// with the established numerical projection contract, independently of format.
pub type Binary32Rounding = Binary64Rounding;
pub type Binary32ErrorClaim = Binary64ErrorClaim;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Binary32Refusal {
    Numeric(ExactNumericRefusal),
    NegativeTolerance,
    ToleranceExceeded,
}
impl From<ExactNumericRefusal> for Binary32Refusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
impl From<Binary64Refusal> for Binary32Refusal {
    fn from(value: Binary64Refusal) -> Self {
        match value {
            Binary64Refusal::Numeric(why) => Self::Numeric(why),
            Binary64Refusal::NegativeTolerance => Self::NegativeTolerance,
            Binary64Refusal::ToleranceExceeded => Self::ToleranceExceeded,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PreparedBinary32Projection {
    source: ExactRational,
    target: IeeeF32,
    represented: ExactRational,
    rounding: Binary32Rounding,
    inexact: bool,
    error: Binary32ErrorClaim,
    maximum_error: ExactRational,
    spent_limb_work: u64,
    admitted_limb_work: u64,
}
impl PreparedBinary32Projection {
    /// Project this actual input under the supplied finite work and error bound.
    /// Failure never substitutes another format or a silently weakened tolerance.
    pub fn prepare(
        source: ExactRational,
        rounding: Binary32Rounding,
        maximum_error: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Binary32Refusal> {
        if maximum_error.is_negative() {
            return Err(Binary32Refusal::NegativeTolerance);
        }
        let before = work.spent();
        let admitted_limb_work = work.remaining();
        let (target, inexact) = source.round_binary32(rounding, work)?;
        let represented = ExactRational::from_binary32(target, work)?;
        let error = super::binary64::error_claim(source, represented, maximum_error, work)?;
        Ok(Self {
            source,
            target,
            represented,
            rounding,
            inexact,
            error,
            maximum_error,
            spent_limb_work: work.spent() - before,
            admitted_limb_work,
        })
    }
    pub const fn source(&self) -> ExactRational {
        self.source
    }
    pub const fn target(&self) -> IeeeF32 {
        self.target
    }
    pub const fn represented(&self) -> ExactRational {
        self.represented
    }
    pub const fn rounding(&self) -> Binary32Rounding {
        self.rounding
    }
    pub const fn inexact(&self) -> bool {
        self.inexact
    }
    pub const fn error(&self) -> Binary32ErrorClaim {
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
