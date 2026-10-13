//! The existing generic fidelity law accounts for the algorithm's end-to-end loss.
use super::*;
use crate::projection::*;
const VALUE: &str = "mathematical-denotation";
const REPRESENTATION: &str = "numeric-representation";
const LAW: &str = "finite-binary64-operation-order@1";
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[derive(Debug, Copy, Clone)]
pub struct Binary64AlgorithmDomain;
impl ProjectionDomain for Binary64AlgorithmDomain {
    type Source = PreparedBinary64Algorithm;
    type Target = IeeeF64;
    type Detail = Binary64ErrorClaim;
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
impl PreparedBinary64Algorithm {
    pub fn projection_facts(&self) -> [ProjectionFact<'_, Binary64AlgorithmDomain>; 2] {
        [
            if self.error().maximum() == ExactRational::ZERO {
                ProjectionFact::Preserved {
                    obligation: text(VALUE),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(VALUE),
                    class: ProjectionLoss::Precision,
                    detail: self.error_ref(),
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
pub struct Binary64AlgorithmPolicy {
    pub maximum_error: ExactRational,
    pub rounding: Binary64Rounding,
}
impl ProjectionPolicy<Binary64AlgorithmDomain> for Binary64AlgorithmPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/finite-binary64-policy@1")
    }
    fn permits(
        &self,
        source: &PreparedBinary64Algorithm,
        target: &IeeeF64,
        facts: &[ProjectionFact<'_, Binary64AlgorithmDomain>],
    ) -> bool {
        self.maximum_error == source.maximum_error()
            && self.rounding == source.rounding()
            && Binary64AlgorithmDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
