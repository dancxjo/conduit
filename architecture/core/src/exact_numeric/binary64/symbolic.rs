//! Correctly rounded finite projection of an independently certified enclosure.
//! Agreement of rounded endpoints proves rounding of every enclosed value.
mod projection;
use super::{error_claim, Binary64Refusal, Binary64Rounding};
use crate::{ExactNumericWork, ExactRational, IeeeF64, PreparedMathematicalEnclosure};
pub use projection::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolicBinary64Refusal {
    Projection(Binary64Refusal),
    RoundingUndetermined,
}
impl From<Binary64Refusal> for SymbolicBinary64Refusal {
    fn from(value: Binary64Refusal) -> Self {
        Self::Projection(value)
    }
}
/// Retains exact source provenance and its enclosure, rather than relabelling
/// an approximate rational endpoint as the mathematical source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSymbolicBinary64 {
    enclosure: PreparedMathematicalEnclosure,
    target: IeeeF64,
    represented: ExactRational,
    rounding: Binary64Rounding,
    maximum_error: ExactRational,
    admitted_limb_work: u64,
    spent_limb_work: u64,
}
impl PreparedSymbolicBinary64 {
    pub fn prepare(
        enclosure: PreparedMathematicalEnclosure,
        rounding: Binary64Rounding,
        maximum_error: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<Self, SymbolicBinary64Refusal> {
        if maximum_error.is_negative() {
            return Err(Binary64Refusal::NegativeTolerance.into());
        }
        let before = work.spent();
        let admitted_limb_work = work.remaining();
        let numeric = |error| SymbolicBinary64Refusal::Projection(Binary64Refusal::Numeric(error));
        let interval = enclosure.target();
        let (lower, _) = interval
            .lower()
            .round_binary64(rounding, work)
            .map_err(numeric)?;
        let (upper, _) = interval
            .upper()
            .round_binary64(rounding, work)
            .map_err(numeric)?;
        if lower.bits() != upper.bits() {
            return Err(SymbolicBinary64Refusal::RoundingUndetermined);
        }
        let represented = ExactRational::from_binary64(lower, work).map_err(numeric)?;
        // Distance from a fixed point is convex: endpoint bounds prove the
        // requested absolute error throughout the certified interval.
        error_claim(interval.lower(), represented, maximum_error, work)?;
        error_claim(interval.upper(), represented, maximum_error, work)?;
        Ok(Self {
            enclosure,
            target: lower,
            represented,
            rounding,
            maximum_error,
            admitted_limb_work,
            spent_limb_work: work.spent() - before,
        })
    }
    pub fn enclosure(&self) -> &PreparedMathematicalEnclosure {
        &self.enclosure
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
    /// Certified absolute bound, not an exact distance from an irrational.
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    pub const fn admitted_limb_work(&self) -> u64 {
        self.admitted_limb_work
    }
    pub const fn spent_limb_work(&self) -> u64 {
        self.spent_limb_work
    }
}
