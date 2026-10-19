//! Numerical input evidence; Source, physical Type and producer custody remain external.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binary64ProjectedInput {
    source: ExactRational,
    target: IeeeF64,
    maximum_error: ExactRational,
}
impl Binary64ProjectedInput {
    /// Verify a finite actual representation against its exact mathematical
    /// source. This proves numerical distance, not a producer's execution or
    /// binding to a particular Source/physical input. It is not statistical
    /// observation uncertainty or permission to discard incoming provenance.
    pub fn new(
        source: ExactRational,
        target: IeeeF64,
        maximum_error: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Binary64AlgorithmRefusal> {
        let value = Self {
            source,
            target,
            maximum_error,
        };
        value.validate(work)?;
        Ok(value)
    }
    pub const fn source(&self) -> ExactRational {
        self.source
    }
    pub const fn target(&self) -> IeeeF64 {
        self.target
    }
    /// The admitted incoming error ceiling, independently rechecked during
    /// execution. The final algorithm also checks its own total error.
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    pub(super) fn validate(
        &self,
        work: &mut ExactNumericWork,
    ) -> Result<ExactRational, Binary64AlgorithmRefusal> {
        if self.maximum_error.is_negative() {
            return Err(Binary64Refusal::NegativeTolerance.into());
        }
        let actual = ExactRational::from_binary64(self.target, work)?;
        error_claim(self.source, actual, self.maximum_error, work)?;
        Ok(actual)
    }
    /// Scratch staging keeps the template's incoming ceiling fixed. Validation
    /// occurs before a result can commit; callers cannot loosen the slot policy.
    pub(super) fn stage(&mut self, input: &Self) {
        self.source = input.source;
        self.target = input.target;
    }
}
