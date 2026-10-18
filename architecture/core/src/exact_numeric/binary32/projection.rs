//! Single-round binary32 evidence through the existing generic fidelity law.
use super::*;
use crate::projection::*;
const VALUE: &str = "mathematical-denotation";
const REPRESENTATION: &str = "numeric-representation";
const LAW: &str = "finite-binary32-single-round@1";
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[derive(Debug, Copy, Clone)]
pub struct Binary32ProjectionDomain;
impl ProjectionDomain for Binary32ProjectionDomain {
    type Source = PreparedBinary32Projection;
    type Target = IeeeF32;
    type Detail = Binary32ErrorClaim;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(super::super::EXACT_RATIONAL_ID)
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
        if target != Some(&source.target())
            || !native.is_empty()
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
            || facts.len() != 2
        {
            return false;
        }
        let value = facts.iter().any(|fact| match fact {
            ProjectionFact::Preserved { obligation } => {
                obligation.as_str() == VALUE && source.error().maximum() == ExactRational::ZERO
            }
            ProjectionFact::Lost {
                obligation,
                class,
                detail,
                native_fact,
            } => {
                obligation.as_str() == VALUE
                    && *class == ProjectionLoss::Precision
                    && **detail == source.error()
                    && source.error().maximum() != ExactRational::ZERO
                    && native_fact.is_none()
            }
            _ => false,
        });
        value
            && facts.iter().any(|fact| {
                matches!(fact,ProjectionFact::Transformed {obligation,law}
            if obligation.as_str() == REPRESENTATION && law.as_str() == LAW)
            })
    }
}
impl PreparedBinary32Projection {
    pub fn projection_facts(&self) -> [ProjectionFact<'_, Binary32ProjectionDomain>; 2] {
        [
            if self.error().maximum() == ExactRational::ZERO {
                ProjectionFact::Preserved {
                    obligation: text(VALUE),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(VALUE),
                    class: ProjectionLoss::Precision,
                    detail: &self.error,
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
#[derive(Debug, Copy, Clone)]
pub struct Binary32ProjectionPolicy {
    pub maximum_error: ExactRational,
    pub rounding: Binary32Rounding,
}
impl ProjectionPolicy<Binary32ProjectionDomain> for Binary32ProjectionPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/finite-binary32-policy@1")
    }
    fn permits(
        &self,
        source: &PreparedBinary32Projection,
        target: &IeeeF32,
        facts: &[ProjectionFact<'_, Binary32ProjectionDomain>],
    ) -> bool {
        !self.maximum_error.is_negative()
            && source
                .error()
                .maximum()
                .checked_compare(
                    self.maximum_error,
                    &mut ExactNumericWork::new(crate::MATHEMATICAL_VALUE_VALIDATION_WORK),
                )
                .is_ok_and(|order| order != core::cmp::Ordering::Greater)
            && self.rounding == source.rounding()
            && Binary32ProjectionDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
