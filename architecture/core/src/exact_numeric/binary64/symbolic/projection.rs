//! Generic fidelity admission of a retained symbolic enclosure and float target.
use super::super::*;
use super::PreparedSymbolicBinary64;
use crate::projection::*;
const VALUE: &str = "mathematical-denotation";
const REPRESENTATION: &str = "numeric-representation";
const LAW: &str = "certified-symbolic-binary64@1";
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[derive(Debug, Copy, Clone)]
pub struct SymbolicBinary64Domain;
impl ProjectionDomain for SymbolicBinary64Domain {
    type Source = PreparedSymbolicBinary64;
    type Target = IeeeF64;
    type Detail = ExactRational;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(crate::MATHEMATICAL_VALUE_ID)
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
                obligation.as_str() == VALUE && source.maximum_error() == ExactRational::ZERO
            }
            ProjectionFact::Lost {
                obligation,
                class,
                detail,
                native_fact,
            } => {
                obligation.as_str() == VALUE
                    && *class == ProjectionLoss::Precision
                    && **detail == source.maximum_error()
                    && source.maximum_error() != ExactRational::ZERO
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
impl PreparedSymbolicBinary64 {
    pub fn projection_facts(&self) -> [ProjectionFact<'_, SymbolicBinary64Domain>; 2] {
        [
            if self.maximum_error() == ExactRational::ZERO {
                ProjectionFact::Preserved {
                    obligation: text(VALUE),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(VALUE),
                    class: ProjectionLoss::Precision,
                    detail: &self.maximum_error,
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
pub struct SymbolicBinary64Policy {
    pub maximum_error: ExactRational,
    pub rounding: Binary64Rounding,
}
impl ProjectionPolicy<SymbolicBinary64Domain> for SymbolicBinary64Policy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/symbolic-binary64-policy@1")
    }
    fn permits(
        &self,
        source: &PreparedSymbolicBinary64,
        target: &IeeeF64,
        facts: &[ProjectionFact<'_, SymbolicBinary64Domain>],
    ) -> bool {
        self.maximum_error == source.maximum_error()
            && self.rounding == source.rounding()
            && SymbolicBinary64Domain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
