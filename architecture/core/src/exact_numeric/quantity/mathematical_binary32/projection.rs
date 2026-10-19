//! The established general projection contract, with physical preservation.
use super::*;
use crate::projection::*;
use core::marker::PhantomData;
const VALUE: &str = "mathematical-denotation";
const REPRESENTATION: &str = "numeric-representation";
const PHYSICAL: &str = "physical-coordinate-contract";
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[derive(Debug, Clone, Copy, Default)]
pub struct MathematicalQuantityBinary32Domain<'a>(PhantomData<&'a [u8]>);
impl MathematicalQuantityBinary32Domain<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<'a> BorrowedMathematicalQuantityBinary32<'a> {
    pub fn facts(&self) -> [ProjectionFact<'_, MathematicalQuantityBinary32Domain<'a>>; 3] {
        [
            if self.error.maximum() == ExactRational::ZERO {
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
                law: text(MATHEMATICAL_QUANTITY_BINARY32_LAW),
            },
            ProjectionFact::Preserved {
                obligation: text(PHYSICAL),
            },
        ]
    }
}
impl<'a> ProjectionDomain for MathematicalQuantityBinary32Domain<'a> {
    type Source = BorrowedMathematicalQuantityBinary32<'a>;
    type Target = QuantityBinary32;
    type Detail = Binary32ErrorClaim;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(QUANTITY_INFO_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/quantity-binary32-projection@1")
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
            || facts.len() != 3
            || !native.is_empty()
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
        {
            return false;
        }
        let value = facts.iter().any(|fact| match fact {
            ProjectionFact::Preserved { obligation } => {
                obligation.as_str() == VALUE && source.error.maximum() == ExactRational::ZERO
            }
            ProjectionFact::Lost {
                obligation,
                class,
                detail,
                native_fact,
            } => {
                obligation.as_str() == VALUE
                    && *class == ProjectionLoss::Precision
                    && **detail == source.error
                    && source.error.maximum() != ExactRational::ZERO
                    && native_fact.is_none()
            }
            _ => false,
        });
        value && facts.iter().any(|f| matches!(f, ProjectionFact::Transformed { obligation, law }
            if obligation.as_str() == REPRESENTATION && law.as_str() == MATHEMATICAL_QUANTITY_BINARY32_LAW))
            && facts.iter().any(|f| matches!(f, ProjectionFact::Preserved { obligation } if obligation.as_str() == PHYSICAL))
    }
}
#[derive(Debug, Clone, Copy)]
pub struct MathematicalQuantityBinary32Policy {
    pub maximum_error: RationalQuantity,
    pub rounding: Binary32Rounding,
}
impl<'a> ProjectionPolicy<MathematicalQuantityBinary32Domain<'a>>
    for MathematicalQuantityBinary32Policy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/enclosed-quantity-binary32-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        let mut bytes = [0; 33];
        bytes[..32].copy_from_slice(&self.maximum_error.semantic_digest());
        bytes[32] = match self.rounding {
            Binary32Rounding::NearestTiesEven => 0,
            Binary32Rounding::TowardZero => 1,
            Binary32Rounding::TowardPositive => 2,
            Binary32Rounding::TowardNegative => 3,
        };
        Some(crate::semantic_digest(
            "numeric/enclosed-quantity-binary32-policy-config@1",
            &bytes,
        ))
    }
    fn permits(
        &self,
        source: &BorrowedMathematicalQuantityBinary32<'a>,
        target: &QuantityBinary32,
        facts: &[ProjectionFact<'_, MathematicalQuantityBinary32Domain<'a>>],
    ) -> bool {
        self.maximum_error == source.maximum_error
            && self.rounding == source.midpoint.numeric().rounding()
            && MathematicalQuantityBinary32Domain::new().validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
