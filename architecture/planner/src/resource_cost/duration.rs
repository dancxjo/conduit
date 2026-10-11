//! Exact time quantities retain their original Unit coordinates and role.
use super::CostEvidenceRefusal;
use conduit_core::{ExactNumericWork, ExactRational, RationalQuantity, Unit};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CostDuration {
    original: RationalQuantity,
    seconds: ExactRational,
}
impl CostDuration {
    /// Unit conversion reuses the physical family/role/anchor law. A length,
    /// temperature point, or unrelated nominal Time family cannot become a cost.
    /// Normalization is exact and spends the caller's admitted limb budget.
    pub fn admit(
        original: RationalQuantity,
        work: &mut ExactNumericWork,
    ) -> Result<Self, CostEvidenceRefusal> {
        let seconds = original
            .convert_to_unit(
                Unit::resolve("s").expect("reviewed builtin Time Unit"),
                work,
            )?
            .coordinate();
        if seconds.is_negative() {
            return Err(CostEvidenceRefusal::NegativeDuration);
        }
        Ok(Self { original, seconds })
    }
    pub const fn original(self) -> RationalQuantity {
        self.original
    }
    pub const fn seconds(self) -> ExactRational {
        self.seconds
    }
    /// Comparison is over compatible time denotations, never carrier bytes or
    /// throughput magnitudes from different work classes.
    pub fn compare(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<core::cmp::Ordering, CostEvidenceRefusal> {
        Ok(self.seconds.checked_compare(other.seconds, work)?)
    }
}
