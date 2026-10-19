//! Ownership adapter for the existing interval projection law and obligations.
use super::*;
use core::marker::PhantomData;

#[derive(Debug, Clone, Copy, Default)]
pub struct BorrowedMathematicalEnclosureDomain<'source> {
    source: PhantomData<&'source [u8]>,
}
impl BorrowedMathematicalEnclosureDomain<'_> {
    pub const fn new() -> Self {
        Self {
            source: PhantomData,
        }
    }
}
impl<'source> ProjectionDomain for BorrowedMathematicalEnclosureDomain<'source> {
    type Source = BorrowedMathematicalEnclosure<'source>;
    type Target = MathematicalInterval;
    type Detail = ExactRational;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(super::super::MATHEMATICAL_VALUE_ID)
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
        validate_interval_facts(
            &source.width(),
            &source.target(),
            target,
            facts,
            native,
            scores,
            mechanism,
        )
    }
}
impl<'source> BorrowedMathematicalEnclosure<'source> {
    pub fn facts(&self) -> [ProjectionFact<'_, BorrowedMathematicalEnclosureDomain<'source>>; 2] {
        interval_facts(self.width_ref())
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BorrowedMathematicalEnclosurePolicy {
    pub requirements: MathematicalEnclosureRequirements,
}
impl<'source> ProjectionPolicy<BorrowedMathematicalEnclosureDomain<'source>>
    for BorrowedMathematicalEnclosurePolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/exact-interval-expression-policy@1")
    }
    fn permits(
        &self,
        source: &BorrowedMathematicalEnclosure<'source>,
        target: &MathematicalInterval,
        facts: &[ProjectionFact<'_, BorrowedMathematicalEnclosureDomain<'source>>],
    ) -> bool {
        self.requirements == source.requirements()
            && BorrowedMathematicalEnclosureDomain::new().validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
