//! Execution policy never changes the retained portable program or its hints.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum ArithmeticPolicy {
    LegacyProven,
    Checked,
}
impl ArithmeticPolicy {
    pub(super) fn permits_wrapping(self, proven: bool) -> bool {
        proven && matches!(self, Self::LegacyProven)
    }
}
impl PreparedPortableExpressionEvaluator {
    /// Check fixed-integer arithmetic even when the original portable program
    /// retains a `proven` hint. This is arithmetic-only preparation: it does not
    /// verify Native declarations, admit their laws or establish Host authority.
    /// Legacy `new` keeps its existing selected execution behavior.
    pub fn new_checked_arithmetic(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        Self::new_profile(
            program,
            conduit_core::QuantityValueProfile::Decimal,
            ArithmeticPolicy::Checked,
            None,
        )
    }
}
