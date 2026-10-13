use crate::ScalarComparison;
use core::cmp::Ordering;

impl ScalarComparison {
    /// Decide an exact mathematical relation; unproved symbolic order refuses.
    pub fn evaluate(
        self,
        left: &conduit_core::Scalar,
        right: &conduit_core::Scalar,
    ) -> Result<bool, conduit_core::NumericRefusal> {
        self.evaluate_order(left.compare(right)?)
    }
    pub fn evaluate_order(
        self,
        ordering: Option<Ordering>,
    ) -> Result<bool, conduit_core::NumericRefusal> {
        let ordering = ordering.ok_or(conduit_core::NumericRefusal::IndeterminateDomain)?;
        Ok(match self {
            Self::Less => ordering == Ordering::Less,
            Self::LessOrEqual => ordering != Ordering::Greater,
            Self::Equal => ordering == Ordering::Equal,
            Self::NotEqual => ordering != Ordering::Equal,
            Self::GreaterOrEqual => ordering != Ordering::Less,
            Self::Greater => ordering == Ordering::Greater,
        })
    }
}
